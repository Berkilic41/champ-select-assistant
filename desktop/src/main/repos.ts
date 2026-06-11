// SQLite read queries for the recommendation flow — TS port of the read paths in
// src-tauri/src/db/{mastery,match,champion_meta,champion_rates,matchup}_repo.rs
// (same SQL, same shapes). Pure reads; all aggregation/blending stays in core.

import type { DatabaseSync } from "node:sqlite";

/** core types.rs MasteryRow serde shape (mastery_level / mastery_points). */
export interface MasteryRow {
  champion_id: number;
  mastery_level: number;
  mastery_points: number;
  last_play_time: number | null;
}

/** core types.rs ChampionStats. */
export interface ChampionStats {
  champion_id: number;
  games: number;
  wins: number;
  kda_avg: number;
}

/** core rate_blend::SourceRate (the fields blend_rates consumes). */
export interface SourceRateRow {
  champion_id: number;
  position: string;
  win_rate: number;
  ban_rate: number;
  sample_size: number;
}

/** core json_api::PositionSampleEntry. */
export interface PositionSampleRow {
  champion_id: number;
  position: string;
  sample_size: number;
}

export interface MatchupRow {
  champion_id: number;
  opponent_id: number;
  games: number;
  win_rate: number;
}

/** core feedback_signal::FeedbackInput. */
export interface FeedbackRow {
  champion_id: number;
  verdict: string;
}

/** mastery_repo::top_for_puuid — top-N by points. */
export function masteryTopForPuuid(
  db: DatabaseSync,
  puuid: string,
  limit: number,
): MasteryRow[] {
  const rows = db
    .prepare(
      `SELECT champion_id, mastery_level, mastery_points, last_play_time
       FROM mastery
       WHERE puuid = ?
       ORDER BY mastery_points DESC
       LIMIT ?`,
    )
    .all(puuid, limit) as unknown as MasteryRow[];
  return rows.map((r) => ({
    champion_id: Number(r.champion_id),
    mastery_level: Number(r.mastery_level),
    mastery_points: Number(r.mastery_points),
    last_play_time: r.last_play_time === null ? null : Number(r.last_play_time),
  }));
}

/** match_repo::player_stats — per-champion aggregates, top-20 by games. */
export function matchPlayerStats(db: DatabaseSync, puuid: string): ChampionStats[] {
  const rows = db
    .prepare(
      `SELECT champion_id,
              COUNT(*) AS games,
              SUM(win) AS wins,
              AVG(CAST(kills + assists AS REAL) / MAX(deaths, 1)) AS kda_avg
       FROM matches
       WHERE puuid = ?
       GROUP BY champion_id
       ORDER BY games DESC
       LIMIT 20`,
    )
    .all(puuid) as unknown as ChampionStats[];
  return rows.map((r) => ({
    champion_id: Number(r.champion_id),
    games: Number(r.games),
    wins: Number(r.wins),
    kda_avg: Number(r.kda_avg),
  }));
}

/** champion_meta_repo::get_all + parse_roles — champion_id → CDragon roles. */
export function championRoleMap(db: DatabaseSync): Record<number, string[]> {
  const rows = db
    .prepare("SELECT champion_id, roles FROM champion_meta")
    .all() as unknown as { champion_id: number; roles: string }[];
  const map: Record<number, string[]> = {};
  for (const row of rows) {
    let roles: string[] = [];
    try {
      const parsed: unknown = JSON.parse(row.roles);
      if (Array.isArray(parsed)) roles = parsed.filter((x) => typeof x === "string");
    } catch {
      // parse_roles parity: invalid JSON → empty list, not a failure.
    }
    map[Number(row.champion_id)] = roles;
  }
  return map;
}

/** champion_rates_repo::get_all_for_position — raw per-source rows for one lane. */
export function championRatesForPosition(
  db: DatabaseSync,
  position: string,
): SourceRateRow[] {
  const rows = db
    .prepare(
      `SELECT champion_id, position, win_rate, ban_rate, sample_size
       FROM champion_rates
       WHERE position = ?
       ORDER BY champion_id, source`,
    )
    .all(position) as unknown as SourceRateRow[];
  return rows.map((r) => ({
    champion_id: Number(r.champion_id),
    position: r.position,
    win_rate: Number(r.win_rate),
    ban_rate: Number(r.ban_rate),
    sample_size: Number(r.sample_size),
  }));
}

/** champion_rates_repo::position_sample_totals — cross-lane share denominators. */
export function positionSampleTotals(db: DatabaseSync): PositionSampleRow[] {
  const rows = db
    .prepare(
      `SELECT champion_id, position, CAST(SUM(sample_size) AS INTEGER) AS sample_size
       FROM champion_rates
       GROUP BY champion_id, position`,
    )
    .all() as unknown as PositionSampleRow[];
  return rows.map((r) => ({
    champion_id: Number(r.champion_id),
    position: r.position,
    sample_size: Number(r.sample_size),
  }));
}

/**
 * matchup_repo::fetch_all_for_position + the command layer's dedup: multiple
 * sources can carry a row per (champion, opponent) pair — keep the MOST-SAMPLED
 * row per pair (never blend win-rates across patches; champ_select.rs parity).
 * Output is the RecommendationsInput.matchups entry shape.
 */
export function matchupsForPosition(db: DatabaseSync, position: string): MatchupRow[] {
  const rows = db
    .prepare(
      `SELECT champion_id, opponent_id, games, win_rate
       FROM champion_matchups
       WHERE position = ?`,
    )
    .all(position) as unknown as MatchupRow[];
  const best = new Map<string, MatchupRow>();
  for (const r of rows) {
    const entry: MatchupRow = {
      champion_id: Number(r.champion_id),
      opponent_id: Number(r.opponent_id),
      games: Number(r.games),
      win_rate: Number(r.win_rate),
    };
    const key = `${entry.champion_id}:${entry.opponent_id}`;
    const prev = best.get(key);
    if (!prev || entry.games > prev.games) best.set(key, entry);
  }
  return [...best.values()];
}

/** read_feedback_rows — raw rows; negative ids skipped (u32 try_from parity). */
export function feedbackRows(db: DatabaseSync): FeedbackRow[] {
  const rows = db
    .prepare("SELECT champion_id, feedback FROM recommendation_feedback")
    .all() as unknown as { champion_id: number; feedback: string }[];
  return rows
    .filter((r) => Number(r.champion_id) >= 0)
    .map((r) => ({ champion_id: Number(r.champion_id), verdict: String(r.feedback) }));
}
