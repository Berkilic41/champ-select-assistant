// Champ-select analysis cluster — ports of the pure-KB Tauri commands in
// src-tauri/src/commands/champ_select.rs (get_champion_analysis:600,
// get_counter_picks:718, get_draft_verdict:1028, get_team_comp:758,
// get_game_plan:663, get_combo_board:858, get_champion_archetypes:828,
// get_champion_detail:964, get_lane_matchup:1201, get_counter_items:1117,
// get_ban_suggestions:1427). The host only does I/O + assembly; every decision
// runs in core via the WASM JSON API.

import type { DatabaseSync } from "node:sqlite";

import type { Engine } from "../engine";
import { getChampions } from "./champions";
import {
  buildRecommendationsInput,
  orWarnDefault,
  parseSessionArg,
  type EngineCaches,
} from "./recommendations";
import {
  championRatesForPosition,
  championRoleMap,
  masteryTopForPuuid,
  matchupsForPosition,
} from "../repos";

/** Tek şampiyonun tam koçluk analizi (kilitli/hover pick). null = KB'de yok. */
export function getChampionAnalysis(
  engine: Engine,
  db: DatabaseSync,
  sessionJson: unknown,
  championId: number,
  puuid: string,
  caches?: EngineCaches,
): unknown {
  if (!championId) return null; // Rust paritesi: champion_id 0 → None
  const input = buildRecommendationsInput(engine, db, sessionJson, puuid, caches);
  return engine.championAnalysis({ champion_id: championId, ...input });
}

/** Mastery havuzundan lane rakibine counter-pick önerileri. */
export function getCounterPicks(
  engine: Engine,
  db: DatabaseSync,
  sessionJson: unknown,
  puuid: string,
  caches?: EngineCaches,
): unknown[] {
  return engine.counterPicks(
    buildRecommendationsInput(engine, db, sessionJson, puuid, caches),
  );
}

/** Draft'ın tek karar okuması (plan + comp özetleri + lane matchup → verdict). */
export function getDraftVerdict(
  engine: Engine,
  db: DatabaseSync,
  sessionJson: unknown,
  puuid: string,
  caches?: EngineCaches,
): unknown {
  return engine.draftVerdictFull(
    buildRecommendationsInput(engine, db, sessionJson, puuid, caches),
  );
}

/** Session-şekilli KB uçlarının ortak girdisi (team comp / plan / combo / lane). */
function teamContext(
  engine: Engine,
  db: DatabaseSync,
  sessionJson: unknown,
  caches?: EngineCaches,
): Record<string, unknown> {
  return {
    session: parseSessionArg(engine, sessionJson),
    all_champions: getChampions(db),
    role_map: championRoleMap(db),
    // items: yalnız counter_items tüketir; sync sonrası cache'ten gelir.
    items: caches?.items ?? [],
  };
}

export function getTeamComp(
  engine: Engine,
  db: DatabaseSync,
  sessionJson: unknown,
): unknown {
  return engine.teamComp(teamContext(engine, db, sessionJson));
}

export function getGamePlan(
  engine: Engine,
  db: DatabaseSync,
  sessionJson: unknown,
): unknown {
  return engine.gamePlan(teamContext(engine, db, sessionJson));
}

export function getComboBoard(
  engine: Engine,
  db: DatabaseSync,
  sessionJson: unknown,
): unknown[] {
  return engine.comboBoard(teamContext(engine, db, sessionJson));
}

export function getLaneMatchup(
  engine: Engine,
  db: DatabaseSync,
  sessionJson: unknown,
): unknown {
  const ctx = teamContext(engine, db, sessionJson);
  // Ölçülen matchup (champion_matchups) — core dürüst "ölçülen" satırı için
  // my-pos'un matchup'larını okur (faz barları yine KB tahmini kalır).
  const session = ctx.session as {
    queue_id?: number;
    local_player?: { assigned_position?: string };
  };
  const myPos =
    session.queue_id === 450
      ? "aram"
      : (session.local_player?.assigned_position ?? "").toLowerCase();
  const matchups = orWarnDefault(
    () => matchupsForPosition(db, myPos),
    "champion_matchups",
    [],
  );
  return engine.laneMatchup({ ...ctx, matchups });
}

export function getCounterItems(
  engine: Engine,
  db: DatabaseSync,
  sessionJson: unknown,
  caches?: EngineCaches,
): unknown[] {
  return engine.counterItems(teamContext(engine, db, sessionJson, caches));
}

export function getChampionArchetypes(engine: Engine): unknown[] {
  return engine.championArchetypes();
}

export function getChampionDetail(engine: Engine, championId: number): unknown {
  return engine.championDetail(championId);
}

/** Draft simülasyonu (get_draft_simulation paritesi): aday pick'leri mevcut
 *  draft'a uygular. Aday filtreleme + sim kararları tamamen core'da. */
export function getDraftSimulation(
  engine: Engine,
  db: DatabaseSync,
  sessionJson: unknown,
  candidateIds: number[],
): unknown[] {
  return engine.draftSimulation({
    session: sessionJson,
    all_champions: getChampions(db),
    candidate_ids: candidateIds,
  });
}

/** A-vs-B pick karşılaştırması (get_draft_fork paritesi). null = karşılaştırılamaz. */
export function getDraftFork(
  engine: Engine,
  db: DatabaseSync,
  sessionJson: unknown,
  optionAId: number,
  optionBId: number,
): unknown {
  return engine.draftFork({
    session: sessionJson,
    all_champions: getChampions(db),
    option_a_id: optionAId,
    option_b_id: optionBId,
  });
}

/** Aktif ban fazı için en fazla 3 ban önerisi (get_ban_suggestions paritesi). */
export function getBanSuggestions(
  engine: Engine,
  db: DatabaseSync,
  sessionJson: unknown,
  puuid: string,
): unknown[] {
  const session = parseSessionArg(engine, sessionJson);
  const laneKey =
    session.queue_id === 450
      ? "aram"
      : (session.local_player?.assigned_position ?? "").toLowerCase();

  const rateRows = orWarnDefault(
    () => championRatesForPosition(db, laneKey),
    "champion_rates",
    [],
  );
  // Rust paritesi: ban yolu rol-payı filtresi UYGULAMAZ → boş position_samples
  // core'da "filtre yok" demek.
  const metaRates = engine.blendedMetaRates({
    rows: rateRows,
    position_samples: [],
  });

  return engine.banSuggestions({
    session,
    all_champions: getChampions(db),
    meta_rates: metaRates,
    // Top-5 mastery havuzu "bizim havuzu counter'lıyor" sinyalini sürer.
    my_pool: masteryTopForPuuid(db, puuid, 5),
    role_map: championRoleMap(db),
  });
}
