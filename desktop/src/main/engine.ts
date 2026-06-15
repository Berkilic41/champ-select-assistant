// core.wasm bridge — loads the wasm-pack (`--target nodejs`) output of `csa-core`
// and exposes the JSON API as typed-ish functions. This is the ONLY place the
// host touches the engine; everything below it is pure Rust/WASM.
//
// Input/output schemas live in `core/src/json_api.rs` (DTOs) and the ts-rs
// generated types under `src/types/generated/`.

import { corePkgPath } from "./paths";

/** The wasm-pack module surface we rely on (see core/src/json_api.rs). */
export interface CoreModule {
  core_smoke(): string;
  recommendations_json(input: string): string;
  ban_suggestions_json(input: string): string;
  draft_verdict_json(input: string): string;
  pool_coach_json(input: string): string;
  performance_report_json(input: string): string;
  macro_state_json(input: string): string;
  parse_session_json(input: string): string;
  blended_meta_rates_json(input: string): string;
  feedback_signals_json(input: string): string;
  aram_weights_json(): string;
  win_prob_estimates_json(input: string): string;
  train_model_pack_json(input: string): string;
  champion_analysis_json(input: string): string;
  counter_picks_json(input: string): string;
  draft_verdict_full_json(input: string): string;
  team_comp_json(input: string): string;
  game_plan_json(input: string): string;
  combo_board_json(input: string): string;
  champion_archetypes_json(): string;
  champion_detail_json(championId: number): string;
  lane_matchup_json(input: string): string;
  counter_items_json(input: string): string;
  pool_suggestions_json(input: string): string;
  champion_pool_plan_json(input: string): string;
  game_review_json(input: string): string;
  trend_report_json(input: string): string;
  session_read_json(input: string): string;
  feedback_observability_json(input: string): string;
  feedback_analytics_json(input: string): string;
  draft_simulation_json(input: string): string;
  draft_fork_json(input: string): string;
  ingame_plan_json(input: string): string;
  macro_state_from_allgamedata_json(input: string): string;
  feedback_flush_plan_json(input: string): string;
  feedback_flush_resolve_json(input: string): string;
  data_source_registry_json(input: string): string;
  pipeline_quality_report_json(input: string): string;
  data_trajectory_json(input: string): string;
  cache_promotion_json(input: string): string;
  match_fetch_plan_json(input: string): string;
  match_discovery_plan_json(input: string): string;
  match_v5_ingest_json(input: string): string;
  pipeline_refresh_plan_json(input: string): string;
  coverage_ramp_json(input: string): string;
}

/** Dev: the sibling `core/pkg` build; packaged: resources/core-pkg. */
export function defaultCorePkgPath(): string {
  return corePkgPath();
}

export class Engine {
  private constructor(private readonly core: CoreModule) {}

  /** Load the WASM module synchronously (wasm-pack nodejs target is CJS). */
  static load(pkgPath: string = defaultCorePkgPath()): Engine {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const core = require(pkgPath) as CoreModule;
    const smoke = core.core_smoke();
    if (smoke !== "csa-core wasm alive") {
      throw new Error(`csa-core WASM smoke beklenmedik: ${smoke}`);
    }
    return new Engine(core);
  }

  /** Wrap a JSON-string API as object-in / object-out. */
  private call<TOut>(fn: (input: string) => string, input: unknown): TOut {
    return JSON.parse(fn.call(this.core, JSON.stringify(input))) as TOut;
  }

  recommendations<TOut = unknown[]>(input: unknown): TOut {
    return this.call(this.core.recommendations_json, input);
  }

  banSuggestions<TOut = unknown[]>(input: unknown): TOut {
    return this.call(this.core.ban_suggestions_json, input);
  }

  draftVerdict<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.draft_verdict_json, input);
  }

  poolCoach<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.pool_coach_json, input);
  }

  performanceReport<TOut = unknown>(matches: unknown[]): TOut {
    return this.call(this.core.performance_report_json, matches);
  }

  macroState<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.macro_state_json, input);
  }

  /** Raw LCU session payload → ChampSelectState. Throws "Geçersiz session JSON". */
  parseSession<TOut = unknown>(rawSession: unknown): TOut {
    return this.call(this.core.parse_session_json, rawSession);
  }

  /** champion_rates rows + cross-lane sample totals → blended, role-share-filtered
   *  MetaRateEntry array (RecommendationsInput.meta_rates shape). */
  blendedMetaRates<TOut = unknown[]>(input: unknown): TOut {
    return this.call(this.core.blended_meta_rates_json, input);
  }

  /** Raw recommendation_feedback rows → aggregated FeedbackSignal array. */
  feedbackSignals<TOut = unknown[]>(rows: unknown[]): TOut {
    return this.call(this.core.feedback_signals_json, rows);
  }

  /** Brawl-mode (ARAM/Arena) ScoringWeights preset — values stay in core. */
  aramWeights<TOut = unknown>(): TOut {
    return JSON.parse(this.core.aram_weights_json()) as TOut;
  }

  /** {examples:[{score,won}], scores:[f32]} → WinProbReport (FAZ 3 / Sprint 3A:
   *  kalibre kazanma olasılığı; min-sample altında available=false). */
  winProbEstimates<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.win_prob_estimates_json, input);
  }

  /** {examples:[{features,won}], prior?} → ModelPack | null (FAZ 3 / Sprint 3B:
   *  öğrenilen ağırlıklar; min-sample/dejenere veride null → rules fallback). */
  trainModelPack<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.train_model_pack_json, input);
  }

  /** RecommendationsInput + champion_id → Recommendation | null. */
  championAnalysis<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.champion_analysis_json, input);
  }

  /** RecommendationsInput → CounterPickHint[] (pool = mastery list). */
  counterPicks<TOut = unknown[]>(input: unknown): TOut {
    return this.call(this.core.counter_picks_json, input);
  }

  /** RecommendationsInput → DraftVerdict (plan + comps + lane matchup in core). */
  draftVerdictFull<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.draft_verdict_full_json, input);
  }

  /** {session, all_champions, role_map} → TeamCompBoard. */
  teamComp<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.team_comp_json, input);
  }

  /** {session, all_champions} → GamePlan. */
  gamePlan<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.game_plan_json, input);
  }

  /** {session, all_champions} → ComboBoardEntry[]. */
  comboBoard<TOut = unknown[]>(input: unknown): TOut {
    return this.call(this.core.combo_board_json, input);
  }

  /** KB archetype badges: [{champion_id, archetype}]. */
  championArchetypes<TOut = unknown[]>(): TOut {
    return JSON.parse(this.core.champion_archetypes_json()) as TOut;
  }

  /** KB profile for one champion → ChampionDetail | null. */
  championDetail<TOut = unknown>(championId: number): TOut {
    return JSON.parse(this.core.champion_detail_json(championId)) as TOut;
  }

  /** {session, all_champions, role_map} → LaneMatchup | null. */
  laneMatchup<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.lane_matchup_json, input);
  }

  /** {session, all_champions, role_map, items} → CounterItemHint[]. */
  counterItems<TOut = unknown[]>(input: unknown): TOut {
    return this.call(this.core.counter_items_json, input);
  }

  /** {role, mastery, all_champions, meta_rates} → PoolSuggestion[]. */
  poolSuggestions<TOut = unknown[]>(input: unknown): TOut {
    return this.call(this.core.pool_suggestions_json, input);
  }

  /** {role, mastery, stats, all_champions, meta_rates} → ChampionPoolPlan. */
  championPoolPlan<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.champion_pool_plan_json, input);
  }

  /** {match, history, prev_goal?} → GameReview (C1+C2 koç döngüsü). */
  gameReview<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.game_review_json, input);
  }

  /** {matches} → TrendReport (C4; host rol+queue-grubu filtreler). */
  trendReport<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.trend_report_json, input);
  }

  /** {matches} → SessionRead (F1 tilt koruması; host seans filtreler). */
  sessionRead<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.session_read_json, input);
  }

  /** [{champion_id, verdict, synced}] → {counters, status}. */
  feedbackObservability<TOut = unknown>(rows: unknown[]): TOut {
    return this.call(this.core.feedback_observability_json, rows);
  }

  /** {events, now_secs, window_days?} → FeedbackAnalytics. */
  feedbackAnalytics<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.feedback_analytics_json, input);
  }

  /** {session, all_champions, candidate_ids} → DraftSimResult[]. */
  draftSimulation<TOut = unknown[]>(input: unknown): TOut {
    return this.call(this.core.draft_simulation_json, input);
  }

  /** {session, all_champions, option_a_id, option_b_id} → DraftFork | null. */
  draftFork<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.draft_fork_json, input);
  }

  /** {allgamedata, all_champions} → IngamePlan | null. */
  ingamePlan<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.ingame_plan_json, input);
  }

  /** Raw Live Client `allgamedata` → MacroState (host {live,state} sarar). */
  macroStateFromAllgamedata<TOut = unknown>(raw: unknown): TOut {
    return this.call(this.core.macro_state_from_allgamedata_json, raw);
  }

  /** {rows, now_secs} → satır başına send/wait/skip kararı + dedup anahtarı. */
  feedbackFlushPlan<TOut = unknown[]>(input: unknown): TOut {
    return this.call(this.core.feedback_flush_plan_json, input);
  }

  /** {retry_count, next_retry_at, ok, error, now_secs} → yeni V013 sync state. */
  feedbackFlushResolve<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.feedback_flush_resolve_json, input);
  }

  /** {counts, data_pack, now_secs} → DataSourceRegistryReport. */
  dataSourceRegistry<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.data_source_registry_json, input);
  }

  /** {counts, data_pack, source_rows, patches, pack_exists} → PipelineQualityReport. */
  pipelineQualityReport<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.pipeline_quality_report_json, input);
  }

  /** {quality_status, ramp?, riot/summoner bayrakları} → DataTrajectoryView. */
  dataTrajectory<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.data_trajectory_json, input);
  }

  /** {counts, data_pack, build_champions_known, patch} → promote kararı + yeni pack. */
  cachePromotion<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.cache_promotion_json, input);
  }

  /** {candidates, fetched_records, frontiers, budget} → MatchFetchPlan. */
  matchFetchPlan<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.match_fetch_plan_json, input);
  }

  /** {seeds, crawled_players, candidate_matches, known_matches} → MatchDiscoveryPlan. */
  matchDiscoveryPlan<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.match_discovery_plan_json, input);
  }

  /** {details(raw), ids, region} → canonical satırlar + parsed/failed + seed'ler. */
  matchV5Ingest<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.match_v5_ingest_json, input);
  }

  /** Scheduler tick planı: {logs, request_timestamps, bayraklar} →
   *  {champ_select_active, rate_limit, fetch_logs, plan} (kararlar core'da). */
  pipelineRefreshPlan<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.pipeline_refresh_plan_json, input);
  }

  /** {before, after, champ_select_active, crawl_budget} → CoverageRampReport. */
  coverageRamp<TOut = unknown>(input: unknown): TOut {
    return this.call(this.core.coverage_ramp_json, input);
  }
}
