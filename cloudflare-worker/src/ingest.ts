import {
  challengerPuuids,
  recentRankedMatchIds,
  matchDetail,
  patchFromGameVersion,
  LCU_POSITIONS,
  type MatchDto,
} from "./riot";

export interface Env {
  DB: D1Database;
  RIOT_API_KEY: string;
  INGEST_REGIONS: string;
  MAX_MATCHES_PER_RUN: string;
  SEED_PLAYERS_PER_REGION: string;
  MATCHES_PER_PLAYER: string;
  /** Shared secret gating the manual /v1/ingest HTTP trigger (cron needs none). */
  INGEST_SECRET?: string;
}

const num = (v: string | undefined, fallback: number): number => {
  const n = parseInt(v ?? "", 10);
  return Number.isFinite(n) && n > 0 ? n : fallback;
};

/** Fair per-region cap so later regions (e.g. KR, last in the list) are not
 *  starved by earlier ones draining a single global budget. `ceil` guarantees
 *  every region gets at least its share; a global cap still bounds the total. */
export function perRegionBudget(total: number, regionCount: number): number {
  if (regionCount <= 0) return 0;
  return Math.max(1, Math.ceil(total / regionCount));
}

/**
 * One bounded ingestion pass: sample Challenger ranked games across the
 * configured regions, aggregate per-champion/role win/pick + per-champion bans
 * into D1. Bounded by MAX_MATCHES_PER_RUN so it stays within Riot rate limits;
 * the cron accumulates a rolling sample over time.
 */
export async function runIngestion(
  env: Env,
): Promise<{ processed: number; perRegion: Record<string, number> }> {
  const regions = env.INGEST_REGIONS.split(",").map((s) => s.trim()).filter(Boolean);
  const seedN = num(env.SEED_PLAYERS_PER_REGION, 10);
  const perPlayer = num(env.MATCHES_PER_PLAYER, 5);
  const total = num(env.MAX_MATCHES_PER_RUN, 40);
  // Each region draws from its own fair share; a global counter still caps the
  // grand total so we never exceed the Riot budget across all regions combined.
  const regionCap = perRegionBudget(total, regions.length);
  let globalRemaining = total;
  const key = env.RIOT_API_KEY;
  const perRegion: Record<string, number> = {};

  for (const region of regions) {
    if (globalRemaining <= 0) break;
    perRegion[region] = 0;
    let regionRemaining = Math.min(regionCap, globalRemaining);

    let puuids: string[] = [];
    try {
      puuids = await challengerPuuids(region, key, seedN);
    } catch {
      continue; // region/API hiccup — skip, next run retries
    }

    const candidates = new Set<string>();
    for (const puuid of puuids) {
      try {
        (await recentRankedMatchIds(region, puuid, key, perPlayer)).forEach((id) =>
          candidates.add(id),
        );
      } catch {
        /* skip player */
      }
    }

    for (const matchId of candidates) {
      if (regionRemaining <= 0 || globalRemaining <= 0) break;

      let match: MatchDto;
      try {
        match = await matchDetail(region, matchId, key);
      } catch {
        continue;
      }

      // Atomic claim: INSERT OR IGNORE is atomic, so exactly one concurrent run
      // (cron + a manual trigger) gets changes===1 and aggregates; the loser
      // gets changes===0 and skips — no double counting. The claim replaces the
      // old non-atomic SELECT-then-skip + in-batch insert.
      const claim = await env.DB.prepare(
        "INSERT OR IGNORE INTO processed_matches (match_id, processed_at) VALUES (?, ?)",
      )
        .bind(matchId, Date.now())
        .run();
      if (!claim.success || (claim.meta?.changes ?? 0) === 0) continue;

      try {
        await aggregateMatch(env, region, match);
      } catch {
        // Aggregation failed after claiming — release the claim so a later run
        // retries this match instead of silently dropping it.
        await env.DB.prepare("DELETE FROM processed_matches WHERE match_id = ?")
          .bind(matchId)
          .run()
          .catch(() => undefined);
        continue;
      }
      regionRemaining--;
      globalRemaining--;
      perRegion[region]++;
    }
  }

  const processed = Object.values(perRegion).reduce((a, b) => a + b, 0);
  return { processed, perRegion };
}

async function aggregateMatch(
  env: Env,
  region: string,
  match: MatchDto,
): Promise<void> {
  const patch = patchFromGameVersion(match.info.gameVersion);
  const now = Date.now();
  const stmts: D1PreparedStatement[] = [];

  for (const p of match.info.participants) {
    const role = LCU_POSITIONS[p.teamPosition];
    if (!role) continue; // unranked position / remake — skip
    stmts.push(
      env.DB.prepare(
        `INSERT INTO champion_rates (patch, region, champion_id, role, games, wins)
         VALUES (?, ?, ?, ?, 1, ?)
         ON CONFLICT(patch, region, champion_id, role)
         DO UPDATE SET games = games + 1, wins = wins + excluded.wins`,
      ).bind(patch, region, p.championId, role, p.win ? 1 : 0),
    );

    // Lane matchup: the enemy-team participant in the same position. Iterating
    // all 10 participants records both directions (A vs B and B vs A).
    const opp = match.info.participants.find(
      (q) => q.teamId !== p.teamId && q.teamPosition === p.teamPosition,
    );
    if (opp && opp.championId > 0) {
      stmts.push(
        env.DB.prepare(
          `INSERT INTO champion_matchups (patch, region, champion_id, opponent_id, role, games, wins)
           VALUES (?, ?, ?, ?, ?, 1, ?)
           ON CONFLICT(patch, region, champion_id, opponent_id, role)
           DO UPDATE SET games = games + 1, wins = wins + excluded.wins`,
        ).bind(patch, region, p.championId, opp.championId, role, p.win ? 1 : 0),
      );
    }

    // Build aggregation (Phase 3): per-item counters (final slots 0-5; trinket
    // excluded) + whole rune-page/spells combo counters.
    const win = p.win ? 1 : 0;
    const items = [p.item0, p.item1, p.item2, p.item3, p.item4, p.item5]
      .map((it) => Number(it ?? 0))
      .filter((it) => it > 0);
    for (const itemId of new Set(items)) {
      stmts.push(
        env.DB.prepare(
          `INSERT INTO champion_build_items (patch, region, champion_id, role, item_id, games, wins)
           VALUES (?, ?, ?, ?, ?, 1, ?)
           ON CONFLICT(patch, region, champion_id, role, item_id)
           DO UPDATE SET games = games + 1, wins = wins + excluded.wins`,
        ).bind(patch, region, p.championId, role, itemId, win),
      );
    }

    const runeIds = flattenRunePage(p);
    const spells = [Number(p.summoner1Id ?? 0), Number(p.summoner2Id ?? 0)].filter((s) => s > 0);
    if (runeIds.length > 0 || spells.length > 0) {
      stmts.push(
        env.DB.prepare(
          `INSERT INTO champion_build_pages (patch, region, champion_id, role, rune_ids, spells, games, wins)
           VALUES (?, ?, ?, ?, ?, ?, 1, ?)
           ON CONFLICT(patch, region, champion_id, role, rune_ids, spells)
           DO UPDATE SET games = games + 1, wins = wins + excluded.wins`,
        ).bind(patch, region, p.championId, role, JSON.stringify(runeIds), JSON.stringify(spells), win),
      );
    }
  }

  for (const team of match.info.teams) {
    for (const ban of team.bans) {
      if (ban.championId > 0) {
        stmts.push(
          env.DB.prepare(
            `INSERT INTO champion_bans (patch, region, champion_id, bans)
             VALUES (?, ?, ?, 1)
             ON CONFLICT(patch, region, champion_id)
             DO UPDATE SET bans = bans + 1`,
          ).bind(patch, region, ban.championId),
        );
      }
    }
  }

  stmts.push(
    env.DB.prepare(
      `INSERT INTO ingest_meta (patch, region, total_games, updated_at)
       VALUES (?, ?, 1, ?)
       ON CONFLICT(patch, region)
       DO UPDATE SET total_games = total_games + 1, updated_at = excluded.updated_at`,
    ).bind(patch, region, now),
  );
  // NOTE: processed_matches is claimed atomically in runIngestion BEFORE this
  // call (INSERT OR IGNORE + changes check), so it is intentionally not inserted
  // here — doing both would re-introduce the double-count race.

  await env.DB.batch(stmts);
}

/** Rune page → flat id array [primaryStyle, keystone, p2, p3, p4, subStyle, s1, s2].
 *  Missing/malformed perks (old fixtures, remakes) → []. */
function flattenRunePage(p: {
  perks?: { styles?: { style?: number; selections?: { perk?: number }[] }[] };
}): number[] {
  const styles = p.perks?.styles;
  if (!Array.isArray(styles) || styles.length === 0) return [];
  const out: number[] = [];
  for (const s of styles) {
    const style = Number(s?.style ?? 0);
    if (style > 0) out.push(style);
    for (const sel of s?.selections ?? []) {
      const perk = Number(sel?.perk ?? 0);
      if (perk > 0) out.push(perk);
    }
  }
  return out;
}

export interface RateRow {
  champion_id: number;
  role: string;
  games: number;
  win_rate: number;
  pick_rate: number;
  ban_rate: number;
}

/** Read aggregated rates for a (patch, region). Patch defaults to the latest seen. */
export async function readRates(
  env: Env,
  region: string,
  patch?: string,
): Promise<{ patch: string; region: string; total_games: number; rates: RateRow[] }> {
  const resolvedPatch =
    patch ??
    (
      await env.DB.prepare(
        // En taze patch'i ingest RECENCY'sine göre seç (updated_at, epoch ms).
        // 'ORDER BY patch DESC' leksik sıralardı → "16.9" > "16.10" (byte '9'>'1')
        // ve Riot 16.10/16.11 çıksa bile bayat "16.9"u 'latest' sanardı.
        "SELECT patch FROM ingest_meta WHERE region = ? ORDER BY updated_at DESC LIMIT 1",
      )
        .bind(region)
        .first<{ patch: string }>()
    )?.patch ??
    "";

  const meta = await env.DB.prepare(
    "SELECT total_games FROM ingest_meta WHERE patch = ? AND region = ?",
  )
    .bind(resolvedPatch, region)
    .first<{ total_games: number }>();
  const total = meta?.total_games ?? 0;
  if (total === 0) {
    return { patch: resolvedPatch, region, total_games: 0, rates: [] };
  }

  const rows = await env.DB.prepare(
    "SELECT champion_id, role, games, wins FROM champion_rates WHERE patch = ? AND region = ?",
  )
    .bind(resolvedPatch, region)
    .all<{ champion_id: number; role: string; games: number; wins: number }>();

  const bans = await env.DB.prepare(
    "SELECT champion_id, bans FROM champion_bans WHERE patch = ? AND region = ?",
  )
    .bind(resolvedPatch, region)
    .all<{ champion_id: number; bans: number }>();
  const banMap = new Map<number, number>();
  for (const b of bans.results) banMap.set(b.champion_id, b.bans);

  const rates: RateRow[] = rows.results.map((r) => ({
    champion_id: r.champion_id,
    role: r.role,
    games: r.games,
    win_rate: r.games > 0 ? r.wins / r.games : 0,
    pick_rate: r.games / total,
    ban_rate: (banMap.get(r.champion_id) ?? 0) / total,
  }));

  return { patch: resolvedPatch, region, total_games: total, rates };
}

export interface MatchupRow {
  champion_id: number;
  opponent_id: number;
  role: string;
  games: number;
  wins: number;
  win_rate: number;
}

/** Read aggregated lane matchups for a (patch, region). Patch defaults to the
 *  latest seen (same resolution as readRates). Both directions are stored, so
 *  consumers can index by champion_id directly. */
export async function readMatchups(
  env: Env,
  region: string,
  patch?: string,
): Promise<{ patch: string; region: string; matchups: MatchupRow[] }> {
  const resolvedPatch =
    patch ??
    (
      await env.DB.prepare(
        // En taze patch'i ingest RECENCY'sine göre seç (updated_at, epoch ms).
        // 'ORDER BY patch DESC' leksik sıralardı → "16.9" > "16.10" (byte '9'>'1')
        // ve Riot 16.10/16.11 çıksa bile bayat "16.9"u 'latest' sanardı.
        "SELECT patch FROM ingest_meta WHERE region = ? ORDER BY updated_at DESC LIMIT 1",
      )
        .bind(region)
        .first<{ patch: string }>()
    )?.patch ??
    "";

  const rows = await env.DB.prepare(
    `SELECT champion_id, opponent_id, role, games, wins
     FROM champion_matchups WHERE patch = ? AND region = ?`,
  )
    .bind(resolvedPatch, region)
    .all<{ champion_id: number; opponent_id: number; role: string; games: number; wins: number }>();

  const matchups: MatchupRow[] = rows.results.map((r) => ({
    champion_id: r.champion_id,
    opponent_id: r.opponent_id,
    role: r.role,
    games: r.games,
    wins: r.wins,
    win_rate: r.games > 0 ? r.wins / r.games : 0,
  }));

  return { patch: resolvedPatch, region, matchups };
}

export interface BuildRow {
  champion_id: number;
  role: string;
  item_ids: number[]; // top items by games (most popular first, max 6)
  rune_ids: number[]; // most popular rune page (flat id array)
  summoner_spells: number[]; // spells of that page
  games: number; // games behind the rune page (the build's sample)
  wins: number;
  win_rate: number;
}

/** Read the most popular build per (champion, role) for a (patch, region):
 *  top-6 items by game count + the most played rune page/spells combo. The
 *  served games/win_rate are the rune page's own counters (honest sample). */
export async function readBuilds(
  env: Env,
  region: string,
  patch?: string,
): Promise<{ patch: string; region: string; builds: BuildRow[] }> {
  const resolvedPatch =
    patch ??
    (
      await env.DB.prepare(
        // En taze patch'i ingest RECENCY'sine göre seç (updated_at, epoch ms).
        // 'ORDER BY patch DESC' leksik sıralardı → "16.9" > "16.10" (byte '9'>'1')
        // ve Riot 16.10/16.11 çıksa bile bayat "16.9"u 'latest' sanardı.
        "SELECT patch FROM ingest_meta WHERE region = ? ORDER BY updated_at DESC LIMIT 1",
      )
        .bind(region)
        .first<{ patch: string }>()
    )?.patch ??
    "";

  const items = await env.DB.prepare(
    `SELECT champion_id, role, item_id, games
     FROM champion_build_items WHERE patch = ? AND region = ?
     ORDER BY games DESC, item_id ASC`,
  )
    .bind(resolvedPatch, region)
    .all<{ champion_id: number; role: string; item_id: number; games: number }>();

  const pages = await env.DB.prepare(
    `SELECT champion_id, role, rune_ids, spells, games, wins
     FROM champion_build_pages WHERE patch = ? AND region = ?
     ORDER BY games DESC`,
  )
    .bind(resolvedPatch, region)
    .all<{
      champion_id: number;
      role: string;
      rune_ids: string;
      spells: string;
      games: number;
      wins: number;
    }>();

  // Compose per (champion, role): rows are pre-sorted by games DESC, so the
  // first page seen per key is the most played one and items append in order.
  const itemMap = new Map<string, number[]>();
  for (const r of items.results) {
    const key = `${r.champion_id}:${r.role}`;
    const list = itemMap.get(key) ?? [];
    if (list.length < 6) {
      list.push(r.item_id);
      itemMap.set(key, list);
    }
  }

  const parseIds = (s: string): number[] => {
    try {
      const v = JSON.parse(s);
      return Array.isArray(v) ? v.map(Number).filter((n) => n > 0) : [];
    } catch {
      return [];
    }
  };

  const builds = new Map<string, BuildRow>();
  for (const pg of pages.results) {
    const key = `${pg.champion_id}:${pg.role}`;
    if (builds.has(key)) continue; // games DESC → ilk görülen en popüler sayfa
    builds.set(key, {
      champion_id: pg.champion_id,
      role: pg.role,
      item_ids: itemMap.get(key) ?? [],
      rune_ids: parseIds(pg.rune_ids),
      summoner_spells: parseIds(pg.spells),
      games: pg.games,
      wins: pg.wins,
      win_rate: pg.games > 0 ? pg.wins / pg.games : 0,
    });
  }

  return { patch: resolvedPatch, region, builds: [...builds.values()] };
}
