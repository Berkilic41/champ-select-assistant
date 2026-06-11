// Pool / scouting / feedback insight command ports: get_pool_suggestions
// (champ_select.rs:1283), get_champion_pool_plan (pool_coach.rs:109),
// get_lobby_scouting (scouting.rs:47), get_feedback_observability +
// get_feedback_analytics (data_quality.rs:2368/2410). Host = DB rows + blended
// meta; every decision (pool filter, comfort, scouting, sentiment) runs in core.

import type { DatabaseSync } from "node:sqlite";

import type { Engine } from "../engine";
import { getChampions } from "./champions";
import { orWarnDefault } from "./recommendations";
import {
  championRatesForPosition,
  masteryTopForPuuid,
  matchPlayerStats,
  positionSampleTotals,
} from "../repos";

/** Role için blend + rol-payı filtreli meta (her iki pool komutu da filtreli). */
function blendedMetaForRole(
  engine: Engine,
  db: DatabaseSync,
  role: string,
): unknown[] {
  return engine.blendedMetaRates({
    rows: orWarnDefault(() => championRatesForPosition(db, role), "champion_rates", []),
    position_samples: orWarnDefault(
      () => positionSampleTotals(db),
      "champion_rates_totals",
      [],
    ),
  });
}

/** Role için öğrenilecek şampiyonlar (mastery top-60 hariç tutulur). */
export function getPoolSuggestions(
  engine: Engine,
  db: DatabaseSync,
  role: string,
  puuid: string,
): unknown[] {
  return engine.poolSuggestions({
    role,
    mastery: masteryTopForPuuid(db, puuid, 60),
    all_champions: getChampions(db),
    meta_rates: blendedMetaForRole(engine, db, role),
  });
}

/** Havuz geliştirme planı (mastery top-100 + maç istatistikleri). */
export function getChampionPoolPlan(
  engine: Engine,
  db: DatabaseSync,
  role: string,
  puuid: string,
): unknown {
  return engine.championPoolPlan({
    role,
    mastery: masteryTopForPuuid(db, puuid, 100),
    stats: matchPlayerStats(db, puuid),
    all_champions: getChampions(db),
    meta_rates: blendedMetaForRole(engine, db, role),
  });
}

/** Lobi keşif raporu — pools renderer'dan gelir (get_enemy_champion_pools). */
export function getLobbyScouting(
  engine: Engine,
  db: DatabaseSync,
  pools: unknown[],
): unknown {
  return engine.lobbyScouting({
    pools: Array.isArray(pools) ? pools : [],
    all_champions: getChampions(db),
  });
}

/** Feedback gözlemlenebilirlik özeti (yalnız yerel DB). */
export function getFeedbackObservability(engine: Engine, db: DatabaseSync): unknown {
  const rows = db
    .prepare("SELECT champion_id, feedback, synced_at FROM recommendation_feedback")
    .all() as unknown as {
    champion_id: number;
    feedback: string;
    synced_at: number | null;
  }[];
  return engine.feedbackObservability(
    rows
      .filter((r) => Number(r.champion_id) >= 0)
      .map((r) => ({
        champion_id: Number(r.champion_id),
        verdict: String(r.feedback),
        synced: r.synced_at !== null,
      })),
  );
}

/** Feedback analitiği (pencere default 7 gün; saat host'tan — core saat okumaz). */
export function getFeedbackAnalytics(
  engine: Engine,
  db: DatabaseSync,
  windowDays?: number,
): unknown {
  const events = db
    .prepare(
      "SELECT champion_id, champion_key, feedback, created_at FROM recommendation_feedback",
    )
    .all() as unknown as {
    champion_id: number;
    champion_key: string;
    feedback: string;
    created_at: number;
  }[];
  return engine.feedbackAnalytics({
    events: events.map((r) => ({
      champion_id: Math.max(Number(r.champion_id), 0),
      champion_key: String(r.champion_key),
      verdict: String(r.feedback),
      created_at: Number(r.created_at),
    })),
    now_secs: Math.floor(Date.now() / 1000),
    window_days: windowDays ?? 7,
  });
}
