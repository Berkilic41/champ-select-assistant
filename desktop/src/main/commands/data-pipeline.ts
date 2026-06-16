// sync_data_pipeline — port of src-tauri/src/commands/data_quality.rs
// `sync_data_pipeline_inner` + meta_sync.rs seed imports + meta/meraki.rs.
// Orchestration only: every decision (cache promotion, quality reports) runs in
// core via the WASM JSON API; this file does fetches + SQL + fetch logs.
//
// DÜRÜST FARK (bilinçli, görünür): Match-V5 ingestion alt sistemi (discovery
// crawl + fetch planner + aggregator zinciri) Electron host'a HENÜZ taşınmadı.
// Bu adım fetch-log'a "skipped/not_migrated" yazar ve özet `errors` listesinde
// açıkça raporlanır — sessiz sahte başarı YOK. Kendi diliminde gelecek.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import {
  syncDdragonChampions,
  type DdragonCaches,
  type FetchJson,
} from "../ddragon";
import type { Engine } from "../engine";
import { seedFile } from "../paths";
import type { LcuService } from "./lcu";
import { syncMatchV5Ingestion, type MatchV5IngestionOutcome } from "../match-v5";
import { runtimeClientFromEnv, type RiotClient } from "../riot/client";
import { getPipelineQualityReport } from "./data-quality";

const PACK_TTL_SECS = 24 * 60 * 60;
const MERAKI_URL =
  "https://cdn.merakianalytics.com/riot/lol/resources/latest/en-US/championrates.json";

const nowSecs = () => Math.floor(Date.now() / 1000);

/** Varsayılan JSON fetch (Meraki — 15s timeout, Rust HTTP_TIMEOUT_SECS). */
export const defaultJsonFetch: FetchJson = async (url) => {
  const res = await fetch(url, { signal: AbortSignal.timeout(15_000) });
  if (!res.ok) throw new Error(`HTTP ${res.status}: ${url}`);
  return res.json();
};

// ── Fetch log (source_fetch_log — Rust record_fetch_log birebir) ──────────────

export function recordFetchLog(
  db: DatabaseSync,
  source: string,
  status: string,
  decision: string,
  message: string,
  startedAt: number,
  finishedAt: number,
): void {
  try {
    db.prepare(
      `INSERT INTO source_fetch_log
         (source, status, decision, message, started_at, finished_at, duration_ms)
       VALUES (?, ?, ?, ?, ?, ?, ?)`,
    ).run(
      source,
      status,
      decision,
      message,
      startedAt,
      finishedAt,
      Math.max(0, finishedAt - startedAt) * 1000,
    );
  } catch (err) {
    console.warn(`source_fetch_log yazılamadı (${source}):`, err);
  }
}

// ── Seed imports (meta_sync.rs import_builds_seed / import_matchups_seed) ─────

interface BuildSeedEntry {
  champion_id: number;
  position: string;
  patch_version: string;
  item_ids: number[];
  rune_ids: number[];
  win_rate: number;
  source: string;
  opponent_archetype?: string | null;
  skill_order?: string | null;
  summoner_spells?: number[] | null;
  secondary_runes?: number[] | null;
  stat_shards?: number[] | null;
}

export function importBuildsSeed(db: DatabaseSync): number {
  const seedPath = seedFile("builds_seed.json");
  const entries = JSON.parse(readFileSync(seedPath, "utf8")) as BuildSeedEntry[];
  const upsert = db.prepare(
    `INSERT INTO builds
       (champion_id, position, patch_version, item_ids, rune_ids, win_rate,
        pick_rate, source, opponent_archetype, skill_order, summoner_spells,
        secondary_runes, stat_shards, region, games, confidence, cached_at)
     VALUES (?, ?, ?, ?, ?, ?, 0.0, ?, ?, ?, ?, ?, ?, 'global', 0, 'low', ?)
     ON CONFLICT(champion_id, position, patch_version, source,
                 COALESCE(opponent_archetype, '')) DO UPDATE SET
       item_ids = excluded.item_ids, rune_ids = excluded.rune_ids,
       win_rate = excluded.win_rate, pick_rate = excluded.pick_rate,
       opponent_archetype = excluded.opponent_archetype,
       skill_order = excluded.skill_order,
       summoner_spells = excluded.summoner_spells,
       secondary_runes = excluded.secondary_runes,
       stat_shards = excluded.stat_shards, region = excluded.region,
       games = excluded.games, confidence = excluded.confidence,
       cached_at = excluded.cached_at`,
  );
  const now = nowSecs();
  for (const e of entries) {
    upsert.run(
      e.champion_id,
      e.position,
      e.patch_version,
      JSON.stringify(e.item_ids ?? []),
      JSON.stringify(e.rune_ids ?? []),
      e.win_rate,
      e.source,
      e.opponent_archetype ?? null,
      e.skill_order ?? null,
      e.summoner_spells ? JSON.stringify(e.summoner_spells) : null,
      e.secondary_runes ? JSON.stringify(e.secondary_runes) : null,
      e.stat_shards ? JSON.stringify(e.stat_shards) : null,
      now,
    );
  }
  return entries.length;
}

interface MatchupSeedEntry {
  champion_id: number;
  opponent_id: number;
  position: string;
  games: number;
  wins: number;
  win_rate: number;
  patch_version: string;
  source: string;
}

export function importMatchupsSeed(db: DatabaseSync): number {
  const seedPath = seedFile(join("meta", "matchup_seed.json"));
  const entries = JSON.parse(readFileSync(seedPath, "utf8")) as MatchupSeedEntry[];
  const upsert = db.prepare(
    `INSERT INTO champion_matchups
       (champion_id, opponent_id, position, games, wins, win_rate,
        source, patch_version, region, confidence, sample_size, cached_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'global', 'low', ?, ?)
     ON CONFLICT(champion_id, opponent_id, position, source) DO UPDATE SET
       games = excluded.games, wins = excluded.wins,
       win_rate = excluded.win_rate, patch_version = excluded.patch_version,
       region = excluded.region, confidence = excluded.confidence,
       sample_size = excluded.sample_size, cached_at = excluded.cached_at`,
  );
  const now = nowSecs();
  for (const e of entries) {
    upsert.run(
      e.champion_id,
      e.opponent_id,
      e.position,
      e.games,
      e.wins,
      e.win_rate,
      e.source,
      e.patch_version,
      e.games,
      now,
    );
  }
  return entries.length;
}

/** Bir seed import'unu ATOMİK koş: champions tablosu seed'lerin referansladığı tüm
 *  şampiyonları henüz içermiyorsa (tam DDragon sync'inden önce) FK ihlali tek bir
 *  satır kalmadan geri sarılır — kısmi import + yanlış coverage-ramp ölçümü olmaz.
 *  FK/IO hatasında re-throw eder; çağıran (scheduler) best-effort sarar. */
function importSeedAtomic(db: DatabaseSync, fn: (db: DatabaseSync) => number): number {
  db.exec("BEGIN");
  try {
    const n = fn(db);
    db.exec("COMMIT");
    return n;
  } catch (err) {
    db.exec("ROLLBACK");
    throw err;
  }
}

/** Cold-start priming: bundled offline seed'leri YALNIZ ilgili tablo boşken içe
 *  aktar. İdempotent upsert + boş-tablo guard'ı warm DB'de gereksiz işi atlar.
 *  Tam DDragon sync'inden HEMEN sonra çağrılmalı (FK için champions dolu olmalı);
 *  champions eksikse atomik import temizce geri sarılır ve hata fırlatılır. */
export function primeColdStartSeeds(db: DatabaseSync): { builds: number; matchups: number } {
  const buildsEmpty = db.prepare("SELECT 1 FROM builds LIMIT 1").get() === undefined;
  const matchupsEmpty =
    db.prepare("SELECT 1 FROM champion_matchups LIMIT 1").get() === undefined;
  return {
    builds: buildsEmpty ? importSeedAtomic(db, importBuildsSeed) : 0,
    matchups: matchupsEmpty ? importSeedAtomic(db, importMatchupsSeed) : 0,
  };
}

// ── Meraki rates (meta/meraki.rs portu — kaynak ölü olabilir, hata dürüst) ────

interface MerakiPositionStats {
  playRate?: number;
  winRate?: number;
  banRate?: number;
  count?: number | null;
}
interface MerakiResponse {
  patch: string;
  data: Record<string, Record<string, MerakiPositionStats>>;
}

function normalizePosition(pos: string): string | null {
  switch (pos.toUpperCase()) {
    case "TOP":
      return "top";
    case "JUNGLE":
      return "jungle";
    case "MIDDLE":
      return "middle";
    case "BOTTOM":
      return "bottom";
    case "SUPPORT":
      return "utility"; // Meraki SUPPORT der, LCU utility
    default:
      return null;
  }
}

function confidenceFromCount(count: number | null | undefined): string {
  if (count === null || count === undefined) return "low";
  if (count < 200) return "low";
  if (count <= 2000) return "medium";
  return "high";
}

export async function syncMerakiRates(
  db: DatabaseSync,
  fetchJson: FetchJson,
): Promise<number> {
  const champions = db
    .prepare("SELECT champion_id, key FROM champions")
    .all() as unknown as { champion_id: number; key: string }[];
  const meraki = (await fetchJson(MERAKI_URL)) as MerakiResponse;

  const upsert = db.prepare(
    `INSERT INTO champion_rates (
       champion_id, position, win_rate, pick_rate, ban_rate,
       sample_size, patch, source, confidence, region, cached_at
     ) VALUES (?, ?, ?, ?, ?, ?, ?, 'meraki', ?, 'global', ?)
     ON CONFLICT(champion_id, position, source) DO UPDATE SET
       win_rate = excluded.win_rate, pick_rate = excluded.pick_rate,
       ban_rate = excluded.ban_rate, sample_size = excluded.sample_size,
       patch = excluded.patch, confidence = excluded.confidence,
       region = excluded.region, cached_at = excluded.cached_at`,
  );
  let count = 0;
  const now = nowSecs();
  for (const [champKey, positions] of Object.entries(meraki.data ?? {})) {
    // Yeni exportlar sayısal id ("266"), eskiler DDragon key ("Aatrox") kullanır.
    const numeric = Number(champKey);
    const record = Number.isFinite(numeric)
      ? champions.find((c) => Number(c.champion_id) === numeric)
      : champions.find((c) => c.key === champKey);
    if (!record) continue;
    for (const [rawPos, stats] of Object.entries(positions ?? {})) {
      const position = normalizePosition(rawPos);
      if (!position) continue;
      const winRate = stats.winRate ?? 0;
      if (winRate <= 0) continue; // {playRate:0} stub'ları meta skorunu zehirler
      upsert.run(
        record.champion_id,
        position,
        winRate,
        stats.playRate ?? 0,
        stats.banRate ?? 0,
        stats.count ?? 0,
        meraki.patch,
        confidenceFromCount(stats.count),
        now,
      );
      count += 1;
    }
  }
  return count;
}

// ── Champ-select guard'ı ──────────────────────────────────────────────────────

/** Champ-select sırasında ağ refresh'i YASAK (öneri gecikmesi ağ beklemez). */
export async function isChampSelectActive(lcu: LcuService): Promise<boolean> {
  const client = lcu.currentClient();
  if (!client) return false;
  try {
    const phase = await client.getJson<unknown>("/lol-gameflow/v1/gameflow-phase");
    return phase === "ChampSelect";
  } catch {
    return false;
  }
}

// ── Orkestratör ───────────────────────────────────────────────────────────────

export interface DataPipelineRefreshSummary {
  before_status: string;
  after_status: string;
  actions: string[];
  ddragon_champions: number;
  meraki_rates: number;
  builds_imported: number;
  matchups_imported: number;
  match_v5_matches: number;
  match_v5_rates: number;
  match_v5_matchups: number;
  match_v5_builds: number;
  match_v5_errors: number;
  data_pack_cached: boolean;
  cache_action: string;
  cache_promoted: boolean;
  errors: string[];
}

function packExists(db: DatabaseSync): boolean {
  return (
    db
      .prepare("SELECT 1 FROM draft_brain_packs WHERE kind = 'data_pack' LIMIT 1")
      .get() !== undefined
  );
}

export interface CachedPackRow {
  source: string | null;
  expires_at: number | null;
  payload_json: string | null;
}

export function cachedPackRow(db: DatabaseSync): CachedPackRow | null {
  const row = db
    .prepare(
      "SELECT source, expires_at, payload_json FROM draft_brain_packs WHERE kind = 'data_pack'",
    )
    .get() as unknown as
    | { source: string | null; expires_at: number | null; payload_json: string | null }
    | undefined;
  return row ?? null;
}

function coverageCounts(db: DatabaseSync): Record<string, number> {
  const count = (sql: string) => {
    const row = db.prepare(sql).get() as unknown as Record<string, unknown>;
    return Math.max(0, Number(Object.values(row ?? {})[0] ?? 0));
  };
  return {
    total_champions: count("SELECT COUNT(*) FROM champions"),
    champion_rates_count: count("SELECT COUNT(*) FROM champion_rates"),
    matchup_count: count("SELECT COUNT(*) FROM champion_matchups"),
    build_count: count("SELECT COUNT(*) FROM builds"),
    build_champions: count("SELECT COUNT(DISTINCT champion_id) FROM builds"),
    meta_role_champions: count(
      "SELECT COUNT(DISTINCT champion_id) FROM champion_rates",
    ),
  };
}

/** PipelineQualityReport.actions → benzersiz action anahtarları (Rust action_keys). */
function actionKeys(report: { actions?: { action?: string }[] }): string[] {
  const keys: string[] = [];
  for (const action of report.actions ?? []) {
    if (typeof action.action === "string" && !keys.includes(action.action)) {
      keys.push(action.action);
    }
  }
  return keys;
}

export interface CachePromotionResult {
  dataPackCached: boolean;
  cacheAction: string;
  cachePromoted: boolean;
  /** Hata mesajı (fetch-log'a 'failed' yazıldı); başarı/skip'te undefined. */
  error?: string;
}

/** Rust `promote_data_pack_if_quality_allows` paritesi: `local_builder` adayını
 *  REFRESH ÖNCESİ okunan son-iyi cache'le (`currentPack`) kıyasla; karar + yeni
 *  pack core'da. Manuel sync VE scheduler tick'i aynı yolu kullanır. */
export function promoteDataPack(
  engine: Engine,
  db: DatabaseSync,
  caches: DdragonCaches,
  currentPack: CachedPackRow | null,
): CachePromotionResult {
  const cacheStarted = nowSecs();
  const buildChampionsKnown = (() => {
    const row = db
      .prepare(
        "SELECT COUNT(DISTINCT champion_id) AS c FROM builds WHERE source != 'unknown'",
      )
      .get() as unknown as { c: number };
    return Math.max(0, Number(row?.c ?? 0));
  })();
  let dataPackCached = packExists(db);
  let cacheAction = "keep_current";
  let cachePromoted = false;
  try {
    const decision = engine.cachePromotion<{
      promoted: boolean;
      action: string;
      reason: string;
      pack_version?: string;
      pack_payload_json?: string;
    }>({
      counts: coverageCounts(db),
      data_pack: currentPack,
      build_champions_known: buildChampionsKnown,
      patch: caches.version || null,
      region: null,
      now_secs: nowSecs(),
    });
    cacheAction = decision.action;
    cachePromoted = decision.promoted;
    if (decision.promoted && decision.pack_payload_json) {
      const now = nowSecs();
      db.prepare(
        `INSERT INTO draft_brain_packs
           (kind, version, payload_json, source, fetched_at, expires_at)
         VALUES ('data_pack', ?, ?, 'local_builder', ?, ?)
         ON CONFLICT(kind) DO UPDATE SET
           version = excluded.version, payload_json = excluded.payload_json,
           source = excluded.source, fetched_at = excluded.fetched_at,
           expires_at = excluded.expires_at`,
      ).run(
        decision.pack_version ?? "local-builder",
        decision.pack_payload_json,
        now,
        now + PACK_TTL_SECS,
      );
      dataPackCached = true;
      recordFetchLog(
        db,
        "data_pack_cache",
        "success",
        decision.action,
        decision.reason,
        cacheStarted,
        nowSecs(),
      );
    } else {
      recordFetchLog(
        db,
        "data_pack_cache",
        "skipped",
        decision.action,
        decision.reason,
        cacheStarted,
        nowSecs(),
      );
      dataPackCached = packExists(db);
    }
    return { dataPackCached, cacheAction, cachePromoted };
  } catch (err) {
    recordFetchLog(
      db,
      "data_pack_cache",
      "failed",
      cacheAction,
      (err as Error).message,
      cacheStarted,
      nowSecs(),
    );
    return {
      dataPackCached,
      cacheAction,
      cachePromoted,
      error: (err as Error).message,
    };
  }
}

let refreshInFlight: Promise<unknown> | undefined;

/** Rust data_pipeline_refresh_lock paritesi: aynı anda tek refresh — manuel
 *  sync_data_pipeline VE arka plan scheduler tick'i aynı kilidi paylaşır. */
export function withRefreshLock<T>(fn: () => Promise<T>): Promise<T> {
  const run = (refreshInFlight ?? Promise.resolve()).then(fn, fn);
  refreshInFlight = run;
  return run;
}

/** Manuel production-pipeline refresh (kullanıcı tetikler; eşzamanlı çağrı tekil). */
export function syncDataPipeline(
  engine: Engine,
  db: DatabaseSync,
  lcu: LcuService,
  caches: DdragonCaches,
  ddragonFetch?: FetchJson,
  merakiFetch?: FetchJson,
  // undefined = env'den çöz; null = key yok (testler gerçek API'ye ASLA çıkmaz).
  riotClient?: RiotClient | null,
): Promise<DataPipelineRefreshSummary> {
  return withRefreshLock(() =>
    syncDataPipelineInner(engine, db, lcu, caches, ddragonFetch, merakiFetch, riotClient),
  );
}

/** Tek bir pipeline kaynağını koş: başarıda success fetch-log + sonucu döndür;
 *  hatada failed fetch-log + `errors`'a `source: mesaj` ekle + undefined döndür.
 *  syncDataPipelineInner'daki beş yakın-aynı try/catch/recordFetchLog bloğunu
 *  DRY'lar — kaynağa özgü tek fark `fn` (sync/async) ve `message(result)`. */
async function runSource<T>(
  db: DatabaseSync,
  source: string,
  errors: string[],
  fn: () => T | Promise<T>,
  message: (result: T) => string,
): Promise<T | undefined> {
  const started = nowSecs();
  try {
    const result = await fn();
    recordFetchLog(db, source, "success", "refresh", message(result), started, nowSecs());
    return result;
  } catch (err) {
    recordFetchLog(db, source, "failed", "refresh", (err as Error).message, started, nowSecs());
    errors.push(`${source}: ${(err as Error).message}`);
    return undefined;
  }
}

async function syncDataPipelineInner(
  engine: Engine,
  db: DatabaseSync,
  lcu: LcuService,
  caches: DdragonCaches,
  ddragonFetch?: FetchJson,
  merakiFetch?: FetchJson,
  riotClient?: RiotClient | null,
): Promise<DataPipelineRefreshSummary> {
  const before = getPipelineQualityReport(engine, db, caches) as {
    status: string;
    actions: { action: string }[];
  };

  if (await isChampSelectActive(lcu)) {
    const now = nowSecs();
    for (const source of ["ddragon", "meraki", "match_v5"]) {
      recordFetchLog(
        db,
        source,
        "skipped",
        "skip_champ_select",
        "Champ-select aktif; manual pipeline refresh ertelendi.",
        now,
        now,
      );
    }
    return {
      before_status: before.status,
      after_status: before.status,
      actions: ["skip_champ_select"],
      ddragon_champions: 0,
      meraki_rates: 0,
      builds_imported: 0,
      matchups_imported: 0,
      match_v5_matches: 0,
      match_v5_rates: 0,
      match_v5_matchups: 0,
      match_v5_builds: 0,
      match_v5_errors: 0,
      data_pack_cached: packExists(db),
      cache_action: "keep_current",
      cache_promoted: false,
      errors: ["champ_select_active"],
    };
  }

  // Promotion kıyası REFRESH ÖNCESİ son-iyi cache'e karşı (Rust current_good).
  const currentPack = cachedPackRow(db);
  const errors: string[] = [];

  const ddragonChampions =
    (await runSource(
      db,
      "ddragon",
      errors,
      () => syncDdragonChampions(db, caches, ddragonFetch),
      (n) => `${n} champions`,
    )) ?? 0;

  const merakiRates =
    (await runSource(
      db,
      "meraki",
      errors,
      () => syncMerakiRates(db, merakiFetch ?? defaultJsonFetch),
      (n) => `${n} rates`,
    )) ?? 0;

  const buildsImported =
    (await runSource(
      db,
      "build_seed",
      errors,
      () => importBuildsSeed(db),
      (n) => `${n} builds`,
    )) ?? 0;

  const matchupsImported =
    (await runSource(
      db,
      "matchup_seed",
      errors,
      () => importMatchupsSeed(db),
      (n) => `${n} matchups`,
    )) ?? 0;

  // Match-V5 ingestion (key yoksa Rust gibi sessiz 0'lı başarı).
  const matchV5 =
    (await runSource<MatchV5IngestionOutcome>(
      db,
      "match_v5",
      errors,
      () => {
        const riot = riotClient === undefined ? runtimeClientFromEnv() : riotClient;
        return syncMatchV5Ingestion(engine, db, caches, riot);
      },
      (m) =>
        `${m.fetched_matches} matches, ${m.rates} rates, ` +
        `${m.matchups} matchups, ${m.builds} builds, ${m.detail_errors} errors`,
    )) ?? { fetched_matches: 0, detail_errors: 0, rates: 0, matchups: 0, builds: 0 };

  // Cache promotion — karar + yeni pack core'da.
  const promotion = promoteDataPack(engine, db, caches, currentPack);
  if (promotion.error) errors.push(`data_pack_cache: ${promotion.error}`);
  const dataPackCached = promotion.dataPackCached;
  const cacheAction = promotion.cacheAction;
  const cachePromoted = promotion.cachePromoted;

  const after = getPipelineQualityReport(engine, db, caches) as {
    status: string;
  };

  return {
    before_status: before.status,
    after_status: after.status,
    actions: actionKeys(before),
    ddragon_champions: ddragonChampions,
    meraki_rates: merakiRates,
    builds_imported: buildsImported,
    matchups_imported: matchupsImported,
    match_v5_matches: matchV5.fetched_matches,
    match_v5_rates: matchV5.rates,
    match_v5_matchups: matchV5.matchups,
    match_v5_builds: matchV5.builds,
    match_v5_errors: matchV5.detail_errors,
    data_pack_cached: dataPackCached,
    cache_action: cacheAction,
    cache_promoted: cachePromoted,
    errors,
  };
}
