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

/** match_repo::player_stats — per-champion aggregates, top-20 by games.
 *
 *  `queueFilter` (C3, kuyruk ayrımı): 'aram' = yalnız ARAM (450), 'sr' = ARAM
 *  HARİÇ tüm Sihirdar Vadisi kuyrukları. Verilmezse eski davranış (tümü) —
 *  görüntüleme yolu (get_player_stats) değişmeden kalır; öneri motorunun
 *  comfort winrate sinyali ise draft'ın kuyruğuna göre filtrelenir (ARAM
 *  maçları SR konforunu, SR maçları ARAM konforunu kirletmez). */
export function matchPlayerStats(
  db: DatabaseSync,
  puuid: string,
  queueFilter?: "sr" | "aram",
): ChampionStats[] {
  const queueClause =
    queueFilter === "aram"
      ? "AND queue_id = 450"
      : queueFilter === "sr"
        ? "AND queue_id != 450"
        : "";
  const rows = db
    .prepare(
      `SELECT champion_id,
              COUNT(*) AS games,
              SUM(win) AS wins,
              AVG(CAST(kills + assists AS REAL) / MAX(deaths, 1)) AS kda_avg
       FROM matches
       WHERE puuid = ? ${queueClause}
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

/** champion_id → DDragon key ("LeeSin"); champions satırı yoksa null. */
export function championKeyById(db: DatabaseSync, championId: number): string | null {
  const row = db
    .prepare("SELECT key FROM champions WHERE champion_id = ?")
    .get(championId) as { key?: string } | undefined;
  return typeof row?.key === "string" ? row.key : null;
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

/** core json_api::BuildRowEntry — JSON-string columns stay raw, core parses them. */
export interface BuildRowOut {
  champion_id: number;
  item_ids: string;
  rune_ids: string;
  opponent_archetype: string | null;
  skill_order: string | null;
  summoner_spells: string | null;
  secondary_runes: string | null;
  stat_shards: string | null;
}

/**
 * builds_repo read for the enrichment path: ALL rows for one lane, sorted
 * `cached_at DESC` so core's first-match == Rust's `ORDER BY … LIMIT 1`
 * (matchup-specific and default rows alike).
 */
export function buildsForPosition(db: DatabaseSync, position: string): BuildRowOut[] {
  const rows = db
    .prepare(
      `SELECT champion_id, item_ids, rune_ids, opponent_archetype, skill_order,
              summoner_spells, secondary_runes, stat_shards
       FROM builds
       WHERE position = ?
       ORDER BY cached_at DESC`,
    )
    .all(position) as unknown as BuildRowOut[];
  return rows.map((r) => ({
    champion_id: Number(r.champion_id),
    item_ids: String(r.item_ids),
    rune_ids: String(r.rune_ids),
    opponent_archetype: r.opponent_archetype === null ? null : String(r.opponent_archetype),
    skill_order: r.skill_order === null ? null : String(r.skill_order),
    summoner_spells: r.summoner_spells === null ? null : String(r.summoner_spells),
    secondary_runes: r.secondary_runes === null ? null : String(r.secondary_runes),
    stat_shards: r.stat_shards === null ? null : String(r.stat_shards),
  }));
}

/** core json_api::ProPresenceRow — champion_rates rows at the synthetic "pro" position. */
export interface ProPresenceRowOut {
  champion_id: number;
  pick_rate: number;
  ban_rate: number;
  source: string;
}

export function proPresenceRows(db: DatabaseSync): ProPresenceRowOut[] {
  const rows = db
    .prepare(
      `SELECT champion_id, pick_rate, ban_rate, source
       FROM champion_rates
       WHERE position = 'pro'`,
    )
    .all() as unknown as ProPresenceRowOut[];
  return rows.map((r) => ({
    champion_id: Number(r.champion_id),
    pick_rate: Number(r.pick_rate),
    ban_rate: Number(r.ban_rate),
    source: String(r.source),
  }));
}

/** draft_brain pack_payload parity — raw payload_json for one pack kind, or null. */
export function packPayload(db: DatabaseSync, kind: string): string | null {
  const row = db
    .prepare("SELECT payload_json FROM draft_brain_packs WHERE kind = ?")
    .get(kind) as { payload_json?: string } | undefined;
  return row?.payload_json != null ? String(row.payload_json) : null;
}

/** D2: ranked_stats okuma — oyuncunun soloQ/flex rank'i (görüntü bağlamı). */
export interface RankedStatRow {
  queue: string;
  tier: string;
  division: string;
  league_points: number;
  wins: number;
  losses: number;
  is_provisional: boolean;
  updated_at: number;
}

export function rankedStats(db: DatabaseSync, puuid: string): RankedStatRow[] {
  const rows = db
    .prepare(
      `SELECT queue, tier, division, league_points, wins, losses, is_provisional, updated_at
       FROM ranked_stats WHERE puuid = ?`,
    )
    .all(puuid) as unknown as Record<string, unknown>[];
  return rows.map((r) => ({
    queue: String(r.queue),
    tier: String(r.tier),
    division: String(r.division ?? ""),
    league_points: Number(r.league_points),
    wins: Number(r.wins),
    losses: Number(r.losses),
    is_provisional: Number(r.is_provisional) === 1,
    updated_at: Number(r.updated_at),
  }));
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
