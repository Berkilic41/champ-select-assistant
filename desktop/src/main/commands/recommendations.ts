// Port of get_recommendations (src-tauri/src/commands/champ_select.rs:506 +
// load_scoring_inputs:247). The host ONLY does I/O + assembly: every computation
// (session parsing, rate blending + role-share filter, feedback aggregation, ARAM
// weight preset, scoring itself, build enrichment, missing_signals, pro_presence
// and the DraftBrain pack upgrade) runs in core via the WASM JSON API — copying
// any of that into TS would violate the single-source rule. The host feeds the
// raw rows (builds, pro champion_rates, draft_brain_packs payloads) and core does
// the Rust command layer's full post-processing.

import type { DatabaseSync } from "node:sqlite";

import type { Engine } from "../engine";
import { getChampions } from "./champions";
import { recentMatches, toCoreMatch } from "./game-review";
import { getChampionPreferences } from "./preferences";
import { getSettings } from "./settings";
import {
  buildsForPosition,
  championRatesForPosition,
  championRoleMap,
  feedbackRows,
  masteryTopForPuuid,
  matchPlayerStats,
  matchupsForPosition,
  packPayload,
  positionSampleTotals,
  proPresenceRows,
} from "../repos";

/** The slice of ChampSelectState the host itself needs (full type: ts-rs gen). */
interface SessionLike {
  queue_id: number;
  local_player: { assigned_position: string };
}

/**
 * parse_session_arg parity: raw LCU sessions carry an `actions` array and go
 * through core's parse_session; the frontend round-trips the already-parsed
 * snake_case state. Invalid raw payloads throw "Geçersiz session JSON" (core).
 */
export function parseSessionArg(engine: Engine, sessionJson: unknown): SessionLike {
  const isRaw =
    typeof sessionJson === "object" &&
    sessionJson !== null &&
    "actions" in (sessionJson as Record<string, unknown>);
  if (isRaw) return engine.parseSession<SessionLike>(sessionJson);
  return sessionJson as SessionLike;
}

/** DB query'sini Rust'taki or_warn_default gibi sarmala: logla, boşla devam et. */
export function orWarnDefault<T>(read: () => T, what: string, fallback: T): T {
  try {
    return read();
  } catch (err) {
    console.warn(`${what} okunamadı, boş kullanılıyor: ${(err as Error).message}`);
    return fallback;
  }
}

/**
 * load_scoring_inputs parity: gather everything the engine's JSON API needs from
 * the DB + core helpers. Shared by get_recommendations, get_champion_analysis,
 * get_counter_picks and get_draft_verdict (one loading path, no divergence).
 */
/** DDragon bellek-içi cache dilimi (items/rune_trees) — boş = henüz sync yok. */
export interface EngineCaches {
  items: unknown[];
  runeTrees: unknown[];
}

export function buildRecommendationsInput(
  engine: Engine,
  db: DatabaseSync,
  sessionJson: unknown,
  puuid: string,
  caches?: EngineCaches,
): Record<string, unknown> {
  const session = parseSessionArg(engine, sessionJson);

  // ARAM (450) is laneless — synthetic "aram" key; otherwise assigned position.
  const myPos =
    session.queue_id === 450
      ? "aram"
      : (session.local_player?.assigned_position ?? "").toLowerCase();

  const mastery = masteryTopForPuuid(db, puuid, 40);
  // C3 kuyruk ayrımı: comfort winrate sinyali draft'ın kuyruğundan beslenir —
  // ARAM draft'ı yalnız ARAM maçlarını, SR draft'ı yalnız SR maçlarını görür.
  const stats = matchPlayerStats(db, puuid, session.queue_id === 450 ? "aram" : "sr");
  const allChampions = getChampions(db);
  const roleMap = championRoleMap(db);

  // Blend per-source rates + role-share filter — in core (single source).
  const rateRows = orWarnDefault(
    () => championRatesForPosition(db, myPos),
    "champion_rates",
    [],
  );
  const sampleTotals = orWarnDefault(
    () => positionSampleTotals(db),
    "champion_rates_totals",
    [],
  );
  const metaRates = engine.blendedMetaRates({
    rows: rateRows,
    position_samples: sampleTotals,
  });

  const matchups = orWarnDefault(
    () => matchupsForPosition(db, myPos),
    "champion_matchups",
    [],
  );

  // Feedback aggregation — in core.
  const feedbackSignals = engine.feedbackSignals(feedbackRows(db));

  // Brawl preset (ARAM=450, Arena/Hexakill=1700) comes FROM core; otherwise user
  // settings + the fixed, non-configurable 0.05 risk weight (Rust parity).
  const isBrawl = session.queue_id === 450 || session.queue_id === 1700;
  let weights: unknown;
  if (isBrawl) {
    weights = engine.aramWeights();
  } else {
    const settings = getSettings(db);
    weights = {
      comfort: settings.weight_comfort,
      matchup: settings.weight_matchup,
      team_counter: settings.weight_team_counter,
      synergy: settings.weight_synergy,
      meta: settings.weight_meta,
      role_fit: settings.weight_role_fit,
      risk: 0.05,
    };
  }

  // items/rune_trees: bellek-içi DDragon cache'inden (sync_ddragon_champions
  // doldurur). Soğuk cache Rust'taki gibi warn'layıp boşla devam eder.
  const items = caches?.items ?? [];
  const runeTrees = caches?.runeTrees ?? [];
  if (items.length === 0) {
    console.warn("Item cache boş — önce sync_ddragon_champions çağırın");
  }

  // Post-processing rows (champ_select.rs parity): curated builds for the lane,
  // pro-presence rates and the cached DraftBrain pack payloads. Core consumes
  // these in recommendations/analysis enrichment; absent rows degrade honestly
  // (general heuristic build, no pro badge, local rules/seed packs).
  // D1: form sinyali için ham maç satırları — draft'ın queue-grubuyla aynı
  // (C3 kuralı: ARAM↔SR çapraz kirlenme yok). Core rol filtresini kendi yapar.
  const recentRows = orWarnDefault(
    () =>
      recentMatches(db, puuid, 60).filter(
        (r) => (Number(r.queue_id) === 450) === (session.queue_id === 450),
      ),
    "recent_matches",
    [],
  );

  const builds = orWarnDefault(() => buildsForPosition(db, myPos), "builds", []);
  const proRows = orWarnDefault(() => proPresenceRows(db), "pro_presence", []);
  const modelPackPayload = orWarnDefault(() => packPayload(db, "model_pack"), "model_pack", null);
  const dataPackPayload = orWarnDefault(() => packPayload(db, "data_pack"), "data_pack", null);
  const prefs = orWarnDefault(
    () => getChampionPreferences(db, puuid),
    "user_preferences",
    { never: [], learning: [] },
  );

  return {
    session,
    weights,
    all_champions: allChampions,
    mastery,
    stats,
    role_map: roleMap,
    meta_rates: metaRates,
    matchups,
    feedback_signals: feedbackSignals,
    items,
    rune_trees: runeTrees,
    // power_curves: core derives them from its own KB archetypes when omitted.
    builds,
    pro_rows: proRows,
    model_pack_payload: modelPackPayload,
    data_pack_payload: dataPackPayload,
    recent_matches: recentRows.map(toCoreMatch),
    // D3: kullanıcı tercihleri — veto hard-filtre + learning boost core'da.
    vetoed_champions: prefs.never,
    learning_champions: prefs.learning,
  };
}

export function getRecommendations(
  engine: Engine,
  db: DatabaseSync,
  sessionJson: unknown,
  puuid: string,
  caches?: EngineCaches,
): unknown[] {
  // Latency budget: champ-select event → recommendation < 500ms (perf target).
  const started = Date.now();
  const recs = engine.recommendations(
    buildRecommendationsInput(engine, db, sessionJson, puuid, caches),
  );

  const elapsedMs = Date.now() - started;
  if (elapsedMs > 500) {
    console.warn(
      `get_recommendations ${recs.length} öneri / ${elapsedMs} ms — <500ms hedefi aşıldı`,
    );
  } else {
    console.info(`get_recommendations ${recs.length} öneri / ${elapsedMs} ms`);
  }
  return recs;
}
