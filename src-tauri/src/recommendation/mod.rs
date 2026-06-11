// Façade over `csa-core`: every engine module is re-exported so historical
// `crate::recommendation::X::…` paths keep working. Several re-exports are only
// consumed by the #[cfg(test)] host test modules below, which the non-test build
// reports as "unused" — that's expected, not dead code.
#![allow(unused_imports)]

// P0.3d-2: the entire scoring pipeline (engine/scoring + ban/build/counter/pool/
// rate_blend advisors + explanation audit/grounding) now lives in `csa-core`.
// Re-exported so every `crate::recommendation::X::…` consumer keeps working.
pub use csa_core::ban_advisor;
pub use csa_core::build_advisor;
pub use csa_core::champion_types;
// coach_quality now lives in the shared `csa-core` crate (pure engine → WASM).
// Re-exported so existing `crate::recommendation::coach_quality::…` paths keep working.
pub use csa_core::coach_quality;
#[cfg(test)]
mod coach_quality_tests;
pub use csa_core::counter_pick;
pub use csa_core::coverage_expansion_policy;
pub use csa_core::coverage_ramp;
pub use csa_core::data_pipeline_quality;
pub use csa_core::draft_brain;
pub use csa_core::draft_brain_data;
pub use csa_core::draft_fork;
// draft_iq subtree (archetype/combos/analyzer/narrative/game_plan + KnowledgeBase,
// KB champions.json/combos.json embedded via include_str! from core/resources/) now
// lives in `csa-core`. Re-exported so `crate::recommendation::draft_iq::…` (18
// consumers) keep working unchanged.
pub use csa_core::draft_iq;
pub use csa_core::draft_simulator;
pub use csa_core::draft_simulator_quality;
pub use csa_core::draft_verdict;
pub use csa_core::engine;
pub use csa_core::explanation_audit;
pub use csa_core::explanation_grounding;
pub use csa_core::feedback_analytics;
pub use csa_core::feedback_observability;
pub use csa_core::feedback_signal;
pub use csa_core::feedback_sync;
pub use csa_core::ingestion_contract;
// macro_timers now lives in the shared `csa-core` crate (pure objective-timer
// engine → WASM). Re-exported so `crate::recommendation::macro_timers::…` paths
// (commands/overlay.rs, riot/live_client.rs) keep working unchanged.
pub use csa_core::macro_timers;
pub use csa_core::match_discovery_planner;
pub use csa_core::match_fetch_planner;
pub use csa_core::match_v5_aggregator;
pub use csa_core::match_v5_mapper;
#[cfg(test)]
mod match_v5_mapper_drift;
// models (output DTOs: Recommendation/Tier/ComboHint/DraftPlan/ScoreBreakdownItem…)
// now lives in `csa-core` (pure, ts-rs). Re-exported so `crate::recommendation::
// models::…` paths (5 consumers) keep working unchanged.
pub use csa_core::models;
pub use csa_core::pipeline_scheduler_policy;
pub use csa_core::pool_builder;
pub use csa_core::pool_coach;
pub use csa_core::pool_coach_quality;
pub use csa_core::postgame;
pub use csa_core::rate_blend;
pub use csa_core::scoring;
pub use csa_core::scouting;
#[cfg(test)]
mod snapshot_tests;
pub use csa_core::team_analysis;
#[cfg(test)]
mod tests;

pub use csa_core::ban_advisor::BanSuggestion;
pub use csa_core::counter_pick::{compute_counter_picks, CounterPickHint};
pub use csa_core::draft_verdict::{compute_draft_verdict, DraftVerdict};
pub use csa_core::engine::{analyze_champion, compute_recommendations};
pub use csa_core::models::Recommendation;
pub use csa_core::pool_builder::{suggest_pool, PoolSuggestion};
// MetaRate + MatchupEntry re-exported for command-layer callers building ScoringContext.
#[allow(unused_imports)]
pub use csa_core::scoring::{MatchupEntry, MetaRate, ScoringContext, ScoringWeights};
