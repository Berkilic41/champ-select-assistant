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
}

const num = (v: string | undefined, fallback: number): number => {
  const n = parseInt(v ?? "", 10);
  return Number.isFinite(n) && n > 0 ? n : fallback;
};

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
  let remaining = num(env.MAX_MATCHES_PER_RUN, 40);
  const key = env.RIOT_API_KEY;
  const perRegion: Record<string, number> = {};

  for (const region of regions) {
    if (remaining <= 0) break;
    perRegion[region] = 0;

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
      if (remaining <= 0) break;
      const seen = await env.DB.prepare(
        "SELECT 1 FROM processed_matches WHERE match_id = ?",
      )
        .bind(matchId)
        .first();
      if (seen) continue;

      let match: MatchDto;
      try {
        match = await matchDetail(region, matchId, key);
      } catch {
        continue;
      }
      await aggregateMatch(env, region, matchId, match);
      remaining--;
      perRegion[region]++;
    }
  }

  const processed = Object.values(perRegion).reduce((a, b) => a + b, 0);
  return { processed, perRegion };
}

async function aggregateMatch(
  env: Env,
  region: string,
  matchId: string,
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
  stmts.push(
    env.DB.prepare(
      "INSERT OR IGNORE INTO processed_matches (match_id, processed_at) VALUES (?, ?)",
    ).bind(matchId, now),
  );

  await env.DB.batch(stmts);
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
        "SELECT patch FROM ingest_meta WHERE region = ? ORDER BY patch DESC LIMIT 1",
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
