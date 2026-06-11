// Match-V5 ingestion — port of the network/SQL half of
// src-tauri/src/commands/data_quality.rs `sync_match_v5_ingestion` (+ its SQL
// helpers) and riot/rate_limiter.rs `RollingBudget`. Every DECISION (fetch plan,
// discovery plan, detail→canonical aggregation) runs in core via the WASM JSON
// API; this file does Riot calls + SQLite.
//
// Privacy (Rust parity): raw PUUIDs are used ONLY in-memory for crawl calls;
// the DB stores the FNV-1a-64 hash, never the raw PUUID. `hashPuuid` here must
// stay byte-identical with core's FNV (cross-checked by a parity test against
// `match_v5_ingest_json`'s participant hashes).

import type { DatabaseSync } from "node:sqlite";

import type { DdragonCaches } from "./ddragon";
import type { Engine } from "./engine";
import { ensureChampion, summonerRegion, upsertSummoner } from "./repos-write";
import {
  getByRiotId,
  listMatchIds,
  getMatchDetail,
  routingForRegion,
  type RiotClient,
} from "./riot/client";

const MATCH_V5_RANKED_QUEUE = 420;
const MATCH_V5_CANDIDATE_COUNT = 20;
const MATCH_V5_FETCH_BATCH_LIMIT = 50;
const MATCH_V5_ROLE_TARGET_SAMPLES = 1000;
const MATCH_V5_ROLES = ["top", "jungle", "middle", "bottom", "utility"];
const MATCH_V5_SOURCE = "riot_match_v5";
const MATCH_DISCOVERY_CRAWL_BUDGET = 15;
const MATCH_DISCOVERY_MAX_BREADTH = 15;
const MATCH_DISCOVERY_PER_PLAYER_MATCH_CAP = 5;
const MATCH_DISCOVERY_MATCH_LIST_COUNT = 10;

const nowSecs = () => Math.floor(Date.now() / 1000);

/** riot/rate_limiter.rs RollingBudget paritesi: 120s pencerede ≤85 istek —
 *  Riot'un 100 req/2dk tavanının güvenli altı (429 = key iptali riski). */
export class RollingBudget {
  private hits: number[] = [];
  constructor(
    private readonly windowMs = 120_000,
    private readonly max = 85,
  ) {}
  tryAcquire(now: number = Date.now()): boolean {
    while (this.hits.length > 0 && now - this.hits[0] >= this.windowMs) {
      this.hits.shift();
    }
    if (this.hits.length >= this.max) return false;
    this.hits.push(now);
    return true;
  }
}

/** Süreç-genel Riot bütçesi (tek key → tek tavan). */
let globalBudget: RollingBudget | undefined;
export function riotBudget(): RollingBudget {
  return (globalBudget ??= new RollingBudget());
}

const FNV_OFFSET = 0xcbf29ce484222325n;
const FNV_PRIME = 0x00000100000001b3n;
const U64_MASK = 0xffffffffffffffffn;

/** FNV-1a 64 hex — core fnv1a64_hex ile byte-aynı (parite testi var). */
export function hashPuuid(puuid: string): string {
  let hash = FNV_OFFSET;
  for (const byte of Buffer.from(puuid, "utf8")) {
    hash ^= BigInt(byte);
    hash = (hash * FNV_PRIME) & U64_MASK;
  }
  return hash.toString(16).padStart(16, "0");
}

/** LCU'nun UUID-biçimli (Riot dışı) puuid'i — Match-V5'te kullanılamaz. */
export function isLcuUuidPuuid(puuid: string): boolean {
  return /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/.test(
    puuid,
  );
}

/** normalize_patch paritesi ("16.10.1" → "16.10"). */
export function normalizePatch(gameVersion: string): string {
  const [major, minor] = gameVersion.split(".");
  return major && minor ? `${major}.${minor}` : "unknown";
}

// ── SQL helpers (data_quality.rs birebir) ─────────────────────────────────────

interface CandidateRow {
  match_id: string;
  region: string;
  patch: string;
  queue_id: number;
  role_hint: string | null;
  discovered_at: number;
}

function buildMatchFetchCandidates(
  ids: string[],
  region: string,
  patch: string,
  now: number,
): CandidateRow[] {
  return ids.map((id, idx) => ({
    match_id: id,
    region,
    patch,
    queue_id: MATCH_V5_RANKED_QUEUE,
    role_hint: null,
    discovered_at: now - idx,
  }));
}

function recordMatchFetchCandidates(
  db: DatabaseSync,
  candidates: CandidateRow[],
  now: number,
): void {
  const stmt = db.prepare(
    `INSERT INTO match_v5_fetch_history
       (match_id, region, patch, queue_id, status, discovered_at, updated_at)
     VALUES (?, ?, ?, ?, 'discovered', ?, ?)
     ON CONFLICT(match_id) DO UPDATE SET
       region = excluded.region,
       patch = CASE
         WHEN match_v5_fetch_history.status IN ('fetched', 'parsed', 'processed')
         THEN match_v5_fetch_history.patch ELSE excluded.patch END,
       queue_id = CASE
         WHEN match_v5_fetch_history.queue_id = 0 THEN excluded.queue_id
         ELSE match_v5_fetch_history.queue_id END,
       discovered_at = match_v5_fetch_history.discovered_at,
       updated_at = excluded.updated_at`,
  );
  for (const c of candidates) {
    if (!c.match_id.trim()) continue;
    stmt.run(c.match_id, c.region, c.patch, c.queue_id, c.discovered_at, now);
  }
}

function readMatchFetchRecords(
  db: DatabaseSync,
  ids: string[],
): Record<string, unknown>[] {
  const stmt = db.prepare(
    `SELECT match_id, region, patch, status,
            COALESCE(fetched_at, processed_at, discovered_at, updated_at) AS fetched_at
     FROM match_v5_fetch_history WHERE match_id = ?`,
  );
  const records: Record<string, unknown>[] = [];
  for (const id of ids) {
    const row = stmt.get(id) as unknown as Record<string, unknown> | undefined;
    if (row) records.push({ ...row, fetched_at: Number(row.fetched_at ?? 0) });
  }
  return records;
}

function readPendingDiscoveredMatchIds(
  db: DatabaseSync,
  region: string,
  limit: number,
): string[] {
  return (
    db
      .prepare(
        `SELECT match_id FROM match_v5_fetch_history
         WHERE region = ? AND status IN ('discovered', 'failed')
         ORDER BY discovered_at DESC, match_id ASC LIMIT ?`,
      )
      .all(region, limit) as unknown as { match_id: string }[]
  ).map((r) => r.match_id);
}

function markMatchFetchFailed(
  db: DatabaseSync,
  matchId: string,
  region: string,
  patch: string,
  error: string,
  now: number,
): void {
  db.prepare(
    `INSERT INTO match_v5_fetch_history
       (match_id, region, patch, queue_id, status, discovered_at, fetched_at, error, updated_at)
     VALUES (?, ?, ?, ?, 'failed', ?, ?, ?, ?)
     ON CONFLICT(match_id) DO UPDATE SET
       region = excluded.region, patch = excluded.patch,
       queue_id = excluded.queue_id, status = 'failed',
       fetched_at = excluded.fetched_at, error = excluded.error,
       updated_at = excluded.updated_at`,
  ).run(matchId, region, patch, MATCH_V5_RANKED_QUEUE, now, now, error, now);
}

function markMatchFetchProcessed(
  db: DatabaseSync,
  matchId: string,
  region: string,
  patch: string,
  queueId: number,
  now: number,
): void {
  db.prepare(
    `INSERT INTO match_v5_fetch_history
       (match_id, region, patch, queue_id, status, discovered_at, fetched_at, processed_at, updated_at)
     VALUES (?, ?, ?, ?, 'processed', ?, ?, ?, ?)
     ON CONFLICT(match_id) DO UPDATE SET
       region = excluded.region, patch = excluded.patch,
       queue_id = excluded.queue_id, status = 'processed',
       fetched_at = COALESCE(match_v5_fetch_history.fetched_at, excluded.fetched_at),
       processed_at = excluded.processed_at, error = NULL,
       updated_at = excluded.updated_at`,
  ).run(matchId, region, patch, queueId, now, now, now, now);
}

interface DiscoverySeedRow {
  puuid_hash: string;
  region: string;
  source: string;
  seen_at: number;
  contribution_count: number;
}

function upsertDiscoverySeeds(
  db: DatabaseSync,
  seeds: DiscoverySeedRow[],
  now: number,
): void {
  const stmt = db.prepare(
    `INSERT INTO match_discovery_players
       (puuid_hash, region, source, contribution_count, first_seen_at, last_seen_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?)
     ON CONFLICT(puuid_hash) DO UPDATE SET
       region = excluded.region, source = excluded.source,
       contribution_count = match_discovery_players.contribution_count
         + excluded.contribution_count,
       last_seen_at = MAX(match_discovery_players.last_seen_at, excluded.last_seen_at),
       updated_at = excluded.updated_at`,
  );
  for (const seed of seeds) {
    if (!seed.puuid_hash.trim()) continue;
    stmt.run(
      seed.puuid_hash,
      seed.region,
      seed.source,
      seed.contribution_count,
      seed.seen_at,
      seed.seen_at,
      now,
    );
  }
}

function markDiscoveryPlayerCrawled(
  db: DatabaseSync,
  puuidHash: string,
  region: string,
  now: number,
): void {
  if (!puuidHash.trim()) return;
  db.prepare(
    `INSERT INTO match_discovery_players
       (puuid_hash, region, source, contribution_count, first_seen_at, last_seen_at,
        last_crawled_at, crawl_count, updated_at)
     VALUES (?, ?, 'manual_seed', 0, ?, ?, ?, 1, ?)
     ON CONFLICT(puuid_hash) DO UPDATE SET
       region = excluded.region,
       last_crawled_at = excluded.last_crawled_at,
       crawl_count = match_discovery_players.crawl_count + 1,
       updated_at = excluded.updated_at`,
  ).run(puuidHash, region, now, now, now, now);
}

function readCrawledPlayerRecords(
  db: DatabaseSync,
  region: string,
): Record<string, unknown>[] {
  return (
    db
      .prepare(
        `SELECT puuid_hash, region, last_crawled_at, crawl_count
         FROM match_discovery_players
         WHERE region = ? AND last_crawled_at IS NOT NULL`,
      )
      .all(region) as unknown as Record<string, unknown>[]
  ).map((r) => ({
    ...r,
    last_crawled_at: Number(r.last_crawled_at ?? 0),
    crawl_count: Math.max(0, Number(r.crawl_count ?? 0)),
  }));
}

function readKnownMatchRecords(
  db: DatabaseSync,
  ids: string[],
): Record<string, unknown>[] {
  const stmt = db.prepare(
    "SELECT match_id, region, status FROM match_v5_fetch_history WHERE match_id = ?",
  );
  const records: Record<string, unknown>[] = [];
  for (const id of ids) {
    const row = stmt.get(id) as unknown as Record<string, unknown> | undefined;
    if (row) records.push(row);
  }
  return records;
}

/** role_samples + frontier kurulumu (build_coverage_expansion_frontiers). */
function buildFrontiers(
  db: DatabaseSync,
  region: string,
  patch: string,
): Record<string, unknown>[] {
  const sample = (sql: string, role: string): number => {
    const row = db.prepare(sql).get(MATCH_V5_SOURCE, region, patch, role) as unknown as
      Record<string, unknown>;
    return Math.max(0, Number(Object.values(row ?? {})[0] ?? 0));
  };
  return MATCH_V5_ROLES.map((role) => {
    const rate = sample(
      `SELECT COALESCE(SUM(sample_size), 0) FROM champion_rates
       WHERE source = ? AND region = ? AND patch = ? AND position = ?`,
      role,
    );
    const matchup = sample(
      `SELECT COALESCE(SUM(sample_size), SUM(games), 0) FROM champion_matchups
       WHERE source = ? AND region = ? AND patch_version = ? AND position = ?`,
      role,
    );
    const build = sample(
      `SELECT COALESCE(SUM(games), 0) FROM builds
       WHERE source = ? AND region = ? AND patch_version = ? AND position = ?`,
      role,
    );
    return {
      region,
      patch,
      role,
      champion_id: null,
      current_samples: Math.min(rate, matchup, build),
      target_samples: MATCH_V5_ROLE_TARGET_SAMPLES,
    };
  });
}

export interface CanonicalRowSetOut {
  rates: Record<string, unknown>[];
  matchups: Record<string, unknown>[];
  builds: Record<string, unknown>[];
}

/** upsert_canonical_rows paritesi (champion FK garantisi dahil). Match-V5
 *  ingestion'ı VE aggregate kaynaklar (u.gg / Leaguepedia) aynı yolu kullanır. */
export function upsertCanonicalRows(db: DatabaseSync, rowSet: CanonicalRowSetOut): void {
  const champIds = new Set<number>();
  for (const r of rowSet.rates) champIds.add(Number(r.champion_id));
  for (const m of rowSet.matchups) {
    champIds.add(Number(m.champion_id));
    champIds.add(Number(m.opponent_id));
  }
  for (const b of rowSet.builds) champIds.add(Number(b.champion_id));
  for (const id of champIds) if (id > 0) ensureChampion(db, id);

  const now = nowSecs();
  const rateStmt = db.prepare(
    `INSERT INTO champion_rates (
       champion_id, position, win_rate, pick_rate, ban_rate,
       sample_size, patch, source, confidence, region, cached_at
     ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
     ON CONFLICT(champion_id, position, source) DO UPDATE SET
       win_rate = excluded.win_rate, pick_rate = excluded.pick_rate,
       ban_rate = excluded.ban_rate, sample_size = excluded.sample_size,
       patch = excluded.patch, confidence = excluded.confidence,
       region = excluded.region, cached_at = excluded.cached_at`,
  );
  for (const r of rowSet.rates) {
    rateStmt.run(
      Number(r.champion_id),
      String(r.position),
      Number(r.win_rate),
      Number(r.pick_rate),
      Number(r.ban_rate),
      Number(r.sample_size),
      String(r.patch),
      String(r.source),
      String(r.confidence),
      String(r.region),
      now,
    );
  }

  const matchupStmt = db.prepare(
    `INSERT INTO champion_matchups
       (champion_id, opponent_id, position, games, wins, win_rate,
        source, patch_version, region, confidence, sample_size, cached_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
     ON CONFLICT(champion_id, opponent_id, position, source) DO UPDATE SET
       games = excluded.games, wins = excluded.wins,
       win_rate = excluded.win_rate, patch_version = excluded.patch_version,
       region = excluded.region, confidence = excluded.confidence,
       sample_size = excluded.sample_size, cached_at = excluded.cached_at`,
  );
  for (const m of rowSet.matchups) {
    matchupStmt.run(
      Number(m.champion_id),
      Number(m.opponent_id),
      String(m.position),
      Number(m.games),
      Number(m.wins),
      Number(m.win_rate),
      String(m.source),
      String(m.patch),
      String(m.region),
      String(m.confidence),
      Number(m.sample_size),
      now,
    );
  }

  const buildStmt = db.prepare(
    `INSERT INTO builds
       (champion_id, position, patch_version, item_ids, rune_ids, win_rate,
        pick_rate, source, opponent_archetype, skill_order, summoner_spells,
        secondary_runes, stat_shards, region, games, confidence, cached_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, ?, NULL, NULL, ?, ?, ?, ?)
     ON CONFLICT(champion_id, position, patch_version, source,
                 COALESCE(opponent_archetype, '')) DO UPDATE SET
       item_ids = excluded.item_ids, rune_ids = excluded.rune_ids,
       win_rate = excluded.win_rate, pick_rate = excluded.pick_rate,
       summoner_spells = excluded.summoner_spells, region = excluded.region,
       games = excluded.games, confidence = excluded.confidence,
       cached_at = excluded.cached_at`,
  );
  for (const b of rowSet.builds) {
    const itemIds = (b.item_ids ?? []) as number[];
    const runeIds = (b.rune_ids ?? []) as number[];
    if (itemIds.length === 0 && runeIds.length === 0) continue;
    buildStmt.run(
      Number(b.champion_id),
      String(b.position),
      String(b.patch),
      JSON.stringify(itemIds),
      JSON.stringify(runeIds),
      Number(b.win_rate),
      Number(b.pick_rate),
      String(b.source),
      JSON.stringify((b.summoner_spells ?? []) as number[]),
      String(b.region),
      Number(b.games),
      String(b.confidence),
      now,
    );
  }
}

// ── Ingestion orkestratörü ────────────────────────────────────────────────────

export interface MatchV5IngestionOutcome {
  fetched_matches: number;
  detail_errors: number;
  rates: number;
  matchups: number;
  builds: number;
}

const EMPTY_OUTCOME: MatchV5IngestionOutcome = {
  fetched_matches: 0,
  detail_errors: 0,
  rates: 0,
  matchups: 0,
  builds: 0,
};

/**
 * sync_match_v5_ingestion paritesi. `riot` null → key yok → sessiz boş sonuç
 * (Rust davranışı). Tüm Riot çağrıları rolling 2-dk bütçeye tabidir.
 */
export async function syncMatchV5Ingestion(
  engine: Engine,
  db: DatabaseSync,
  caches: DdragonCaches,
  riot: RiotClient | null,
  budget: RollingBudget = riotBudget(),
): Promise<MatchV5IngestionOutcome> {
  if (!riot) return { ...EMPTY_OUTCOME };

  const active = db
    .prepare("SELECT puuid FROM summoners ORDER BY cached_at DESC LIMIT 1")
    .get() as unknown as { puuid?: string } | undefined;
  let puuid = active?.puuid ?? "";
  if (!puuid) return { ...EMPTY_OUTCOME };

  let region = summonerRegion(db, puuid);
  const summonerRow = db
    .prepare("SELECT game_name, tag_line FROM summoners WHERE puuid = ?")
    .get(puuid) as unknown as { game_name?: string; tag_line?: string };
  const gameName = summonerRow?.game_name ?? "";
  const tagLine = summonerRow?.tag_line ?? "";

  // LCU UUID'leri Match-V5'e gönderilemez — account-v1 ile gerçek puuid'e çöz.
  if (isLcuUuidPuuid(puuid)) {
    if (!gameName.trim() || !tagLine.trim()) {
      throw new Error(
        "Match-V5 aktif oyuncu PUUID'si LCU UUID formatında; Riot ID yok, account-v1 ile çözülemiyor",
      );
    }
    const resolved = await getByRiotId(riot, gameName, tagLine, region);
    if (isLcuUuidPuuid(resolved.puuid)) {
      throw new Error(
        "Riot account-v1 LCU UUID formatında PUUID döndürdü; Match-V5 çağrısı güvenli değil",
      );
    }
    puuid = resolved.puuid;
    region = resolved.region;
    upsertSummoner(db, resolved);
  }

  const routing = routingForRegion(region);
  const ids = await listMatchIds(
    riot,
    puuid,
    routing,
    "ranked",
    MATCH_V5_RANKED_QUEUE,
    MATCH_V5_CANDIDATE_COUNT,
  );

  const currentPatch = normalizePatch(caches.version ?? "");
  const now = nowSecs();

  // Aktif oyuncu seed'i + crawl işareti (diske yalnız hash).
  const activeHash = hashPuuid(puuid);
  upsertDiscoverySeeds(
    db,
    [
      {
        puuid_hash: activeHash,
        region,
        source: "active_player",
        seen_at: now,
        contribution_count: ids.length,
      },
    ],
    now,
  );
  markDiscoveryPlayerCrawled(db, activeHash, region, now);

  // Bekleyen keşfedilmiş maçları aday listesine kat (dedup).
  const seen = new Set(ids);
  for (const id of readPendingDiscoveredMatchIds(db, region, MATCH_V5_CANDIDATE_COUNT)) {
    if (!seen.has(id)) {
      seen.add(id);
      ids.push(id);
    }
  }

  const candidates = buildMatchFetchCandidates(ids, region, currentPatch, now);
  recordMatchFetchCandidates(db, candidates, now);
  const plan = engine.matchFetchPlan<{ to_fetch: string[] }>({
    now_secs: now,
    champ_select_active: false,
    rate_budget: MATCH_V5_FETCH_BATCH_LIMIT,
    batch_limit: MATCH_V5_FETCH_BATCH_LIMIT,
    candidates,
    fetched_records: readMatchFetchRecords(db, ids),
    frontiers: buildFrontiers(db, region, currentPatch),
  });

  // Detayları çek — rolling Riot bütçesi dolunca tick erken biter.
  const details: unknown[] = [];
  const detailIds: string[] = [];
  let detailErrors = 0;
  const failed: [string, string][] = [];
  for (const id of plan.to_fetch) {
    if (!budget.tryAcquire()) break;
    try {
      details.push(await getMatchDetail(riot, id, routing));
      detailIds.push(id);
    } catch {
      detailErrors += 1;
      failed.push([id, "detail_fetch_failed"]);
    }
  }

  // Parse + aggregate + canonical satırlar + katılımcı seed'leri (core).
  const ingest = engine.matchV5Ingest<{
    row_set: CanonicalRowSetOut & { region: string };
    parsed: { match_id: string; patch: string; queue_id: number }[];
    failed: string[];
    match_count: number;
    participants: { puuid: string; hash: string }[];
  }>({ details, ids: detailIds, region });
  detailErrors += ingest.failed.length;
  for (const id of ingest.failed) failed.push([id, "parse_failed"]);

  for (const [id, error] of failed) {
    markMatchFetchFailed(db, id, region, currentPatch, error, nowSecs());
  }
  upsertCanonicalRows(db, ingest.row_set);
  const processedAt = nowSecs();
  for (const meta of ingest.parsed) {
    markMatchFetchProcessed(db, meta.match_id, region, meta.patch, meta.queue_id, processedAt);
  }

  // Keşif: katılımcı seed'leri → crawl planı → list_ids → yeni adaylar.
  const participantSeeds = ingest.participants.filter((p) => p.hash !== activeHash);
  if (participantSeeds.length > 0) {
    const seeds: DiscoverySeedRow[] = participantSeeds.map((p) => ({
      puuid_hash: p.hash,
      region,
      source: "match_participant",
      seen_at: nowSecs(),
      contribution_count: 1,
    }));
    upsertDiscoverySeeds(db, seeds, nowSecs());
    const crawlPlan = engine.matchDiscoveryPlan<{ to_crawl: string[] }>({
      now_secs: nowSecs(),
      champ_select_active: false,
      crawl_budget: MATCH_DISCOVERY_CRAWL_BUDGET,
      max_breadth: MATCH_DISCOVERY_MAX_BREADTH,
      per_player_match_cap: MATCH_DISCOVERY_PER_PLAYER_MATCH_CAP,
      seeds,
      crawled_players: readCrawledPlayerRecords(db, region),
      candidate_matches: [],
      known_matches: [],
    });

    const rawByHash = new Map(participantSeeds.map((p) => [p.hash, p.puuid]));
    const discovered: Record<string, unknown>[] = [];
    for (const hash of crawlPlan.to_crawl) {
      const rawPuuid = rawByHash.get(hash);
      if (!rawPuuid) continue;
      if (!budget.tryAcquire()) break;
      try {
        const matchIds = await listMatchIds(
          riot,
          rawPuuid,
          routing,
          "ranked",
          MATCH_V5_RANKED_QUEUE,
          MATCH_DISCOVERY_MATCH_LIST_COUNT,
        );
        const crawledAt = nowSecs();
        markDiscoveryPlayerCrawled(db, hash, region, crawledAt);
        matchIds.forEach((id, idx) =>
          discovered.push({
            match_id: id,
            region,
            source_puuid_hash: hash,
            discovered_at: crawledAt - idx,
          }),
        );
      } catch {
        detailErrors += 1;
      }
    }

    if (discovered.length > 0) {
      const discoveredIds = discovered.map((d) => String(d.match_id));
      const intake = engine.matchDiscoveryPlan<{ new_match_ids: string[] }>({
        now_secs: nowSecs(),
        champ_select_active: false,
        crawl_budget: 0,
        max_breadth: 0,
        per_player_match_cap: MATCH_DISCOVERY_PER_PLAYER_MATCH_CAP,
        seeds: [],
        crawled_players: [],
        candidate_matches: discovered,
        known_matches: readKnownMatchRecords(db, discoveredIds),
      });
      recordMatchFetchCandidates(
        db,
        buildMatchFetchCandidates(intake.new_match_ids, region, currentPatch, nowSecs()),
        nowSecs(),
      );
    }
  }

  return {
    fetched_matches: ingest.match_count,
    detail_errors: detailErrors,
    rates: ingest.row_set.rates.length,
    matchups: ingest.row_set.matchups.length,
    builds: ingest.row_set.builds.length,
  };
}
