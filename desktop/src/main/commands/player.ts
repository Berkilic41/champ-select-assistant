// Player/postgame command ports: get_player_stats + get_masteries +
// get_active_summoner_puuid (riot.rs:226-252), get_mastery_progress +
// get_performance_report (postgame.rs:34-105), submit_recommendation_feedback
// (draft_brain.rs:285). DB-only reads/writes; the trends builder runs in core.

import type { DatabaseSync } from "node:sqlite";

import type { Engine } from "../engine";
import { masteryTopForPuuid, matchPlayerStats } from "../repos";

/** Kaç son maç trend okumasını besler (postgame.rs RECENT_LIMIT). */
const RECENT_LIMIT = 20;

/** core draft_brain::DRAFT_BRAIN_RULES_VERSION (core/src/draft_brain.rs:9) — tek
 *  sabit; core JSON API'sinden okunmuyor diye burada eşlenir, sürüm değişirse
 *  core ile birlikte güncellenmeli. */
const DRAFT_BRAIN_RULES_VERSION = "draft-brain-rules-v2";

export function getPlayerStats(db: DatabaseSync, puuid: string): unknown[] {
  return matchPlayerStats(db, puuid);
}

export function getMasteries(db: DatabaseSync, puuid: string): unknown[] {
  return masteryTopForPuuid(db, puuid, 20);
}

/** En son senkronize edilen sihirdarın puuid'i (yoksa null). */
export function getActiveSummonerPuuid(db: DatabaseSync): string | null {
  const row = db
    .prepare("SELECT puuid FROM summoners ORDER BY cached_at DESC LIMIT 1")
    .get() as unknown as { puuid: string } | undefined;
  return row?.puuid ?? null;
}

function championKeyMap(db: DatabaseSync): Map<number, string> {
  const rows = db
    .prepare("SELECT champion_id, key FROM champions")
    .all() as unknown as { champion_id: number; key: string }[];
  return new Map(rows.map((r) => [Number(r.champion_id), r.key]));
}

export interface MasteryProgressEntry {
  champion_id: number;
  champion_key: string;
  points_gained: number;
  current_points: number;
  current_level: number;
}

/** Son `days` günde mastery puanı kazanılan ilk 5 şampiyon (≥2 snapshot ister). */
export function getMasteryProgress(
  db: DatabaseSync,
  puuid: string,
  days: number,
): MasteryProgressEntry[] {
  const since = Math.floor(Date.now() / 1000) - Math.max(days, 1) * 86_400;
  const keys = championKeyMap(db);
  const rows = db
    .prepare(
      `SELECT champion_id,
              MAX(mastery_points) - MIN(mastery_points) AS gained,
              MAX(mastery_points) AS current_points,
              MAX(mastery_level)  AS current_level
       FROM mastery_snapshots
       WHERE puuid = ? AND snapshot_at >= ?
       GROUP BY champion_id
       HAVING gained > 0
       ORDER BY gained DESC
       LIMIT 5`,
    )
    .all(puuid, since) as unknown as {
    champion_id: number;
    gained: number;
    current_points: number;
    current_level: number;
  }[];
  return rows.map((r) => ({
    champion_id: Number(r.champion_id),
    champion_key: keys.get(Number(r.champion_id)) ?? "",
    points_gained: Number(r.gained),
    current_points: Number(r.current_points),
    current_level: Number(r.current_level),
  }));
}

/** Son maçlardan performans-trend raporu — satırlar host'tan, rapor core'dan. */
export function getPerformanceReport(
  engine: Engine,
  db: DatabaseSync,
  puuid: string,
): unknown {
  const keys = championKeyMap(db);
  // Most-recent-first so streak/form are correct in the pure builder.
  const rows = db
    .prepare(
      `SELECT champion_id, position, win, kills, deaths, assists, played_at, duration_secs,
              cs, cs_at_10, deaths_pre_14, vision_score
       FROM matches
       WHERE puuid = ?
       ORDER BY played_at DESC
       LIMIT ?`,
    )
    .all(puuid, RECENT_LIMIT) as unknown as {
    champion_id: number;
    position: string | null;
    win: number;
    kills: number;
    deaths: number;
    assists: number;
    played_at: number;
    duration_secs: number;
    cs: number | null;
    cs_at_10: number | null;
    deaths_pre_14: number | null;
    vision_score: number | null;
  }[];
  const optU32 = (v: number | null): number | null =>
    v === null ? null : Math.max(Number(v), 0);
  const matches = rows.map((r) => ({
    champion_id: Number(r.champion_id),
    champion_key: keys.get(Number(r.champion_id)) ?? "",
    position: r.position ?? "",
    win: Number(r.win) !== 0,
    kills: Math.max(Number(r.kills), 0),
    deaths: Math.max(Number(r.deaths), 0),
    assists: Math.max(Number(r.assists), 0),
    played_at: Number(r.played_at),
    duration_secs: Math.max(Number(r.duration_secs), 0),
    cs: optU32(r.cs),
    cs_at_10: optU32(r.cs_at_10),
    deaths_pre_14: optU32(r.deaths_pre_14),
    vision_score: optU32(r.vision_score),
  }));
  return engine.performanceReport(matches);
}

export interface RecommendationFeedbackInput {
  champion_id: number;
  champion_key: string;
  feedback: string;
  session_hash?: string | null;
  model_version?: string | null;
  score?: number | null;
  payload?: unknown;
}

export interface RecommendationFeedbackAck {
  stored: boolean;
  synced: boolean;
  feedback_id: number;
}

/** Öneri geri bildirimini yerel tabloya yaz (sync ayrı komutta — synced:false). */
export function submitRecommendationFeedback(
  db: DatabaseSync,
  input: RecommendationFeedbackInput,
): RecommendationFeedbackAck {
  const payloadJson = JSON.stringify(input.payload ?? {});
  const modelVersion = input.model_version ?? DRAFT_BRAIN_RULES_VERSION;
  const result = db
    .prepare(
      `INSERT INTO recommendation_feedback
         (champion_id, champion_key, feedback, session_hash, model_version,
          score, payload_json, synced_at, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?)`,
    )
    .run(
      input.champion_id,
      input.champion_key,
      input.feedback,
      input.session_hash ?? null,
      modelVersion,
      input.score ?? 0,
      payloadJson,
      Math.floor(Date.now() / 1000),
    );
  return {
    stored: true,
    synced: false,
    feedback_id: Number(result.lastInsertRowid),
  };
}
