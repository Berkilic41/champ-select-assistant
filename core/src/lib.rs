//! csa-core — the pure, I/O-free recommendation engine.
//!
//! Builds two ways from one source:
//!   * **native rlib** — linked by the current desktop crate (`src-tauri`) and by
//!     `cargo test`, so the engine logic + its unit tests keep running unchanged
//!     while the Electron migration is in flight.
//!   * **WASM** (`wasm-pack build --target nodejs`) — loaded by the Electron main
//!     process and the Cloudflare edge worker.
//!
//! Hard rule: NO filesystem, sockets, DB, or framework deps here. The host gathers
//! data (LCU / SQLite / Riot) and passes it in as JSON; the core computes and
//! returns JSON. This is the shared "decision brain" used everywhere.
//!
//! ## P0 status (scaffold)
//! This is the crate skeleton. The real engine modules (`recommendation/*` —
//! engine, scoring, draft_iq, analyzer, build_advisor, narrative, coach_quality,
//! pool_coach, postgame, ban_advisor, draft_simulator, draft_fork, team_analysis,
//! macro_timers) migrate in next, **byte-preserving** (they contain Turkish
//! coaching text + KB) via file moves — NOT retyped — with their integration
//! tests relocated to the host crate. Until a module lands here, the desktop crate
//! keeps owning it.

pub mod champion_types;
pub mod coach_quality;
pub mod coverage_expansion_policy;
pub mod coverage_ramp;
pub mod data_pipeline_quality;
pub mod draft_brain;
pub mod draft_brain_data;
pub mod draft_fork;
pub mod draft_iq;
pub mod draft_simulator;
pub mod draft_simulator_quality;
pub mod draft_verdict;
pub mod feedback_analytics;
pub mod feedback_observability;
pub mod feedback_signal;
pub mod feedback_sync;
pub mod game_review;
pub mod ingestion_contract;
pub mod macro_timers;
pub mod match_discovery_planner;
pub mod match_fetch_planner;
pub mod match_v5_aggregator;
pub mod match_v5_mapper;
pub mod models;
pub mod pipeline_scheduler_policy;
pub mod pool_coach;
pub mod pool_coach_quality;
pub mod postgame;
pub mod team_analysis;
pub mod types;
// FAZ 3 / Sprint 3A: yerel outcome etiketlerinden kalibre win-probability (saf
// görsel augment; determinist skoru değiştirmez).
pub mod win_calibration;
// FAZ 3 / Sprint 3B: yerel feature→win etiketlerinden öğrenilen ModelPack
// (ridge-to-prior + gate; determinist motor hâlâ oracle).
pub mod model_training;
// FAZ 4 / Sprint 1: pluggable coach narrator seam (grounded FACTS → prose;
// dış LLM adayı aynı audit'ten geçer, yoksa deterministik fallback).
pub mod coach_narrator;
// P0.3d-2: the former "hard cluster" — engine/scoring + advisors. Unblocked by the
// shared DTOs in `types.rs` (ChampionRecord/MasteryRow/ChampSelectState/TeamSlot/
// ItemData/RuneTree/ChampionStats); the whole scoring pipeline is now pure core.
pub mod ban_advisor;
pub mod build_advisor;
pub mod counter_pick;
pub mod engine;
pub mod explanation_audit;
pub mod explanation_grounding;
pub mod pool_builder;
pub mod rate_blend;
pub mod scoring;
// P1.3b-2: raw LCU session JSON → ChampSelectState, shared by both hosts.
pub mod session_parse;
// P1.3b-7: Live Client Data `allgamedata` pure parsers + in-game plan assembly.
pub mod live_client;
// P0.4: the JSON boundary the JS hosts (Electron main / edge worker) call. Also
// where the #[wasm_bindgen] exports live.
pub mod json_api;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// WASM smoke export — proves the core loads and runs inside a JS host (Electron
/// main process / Cloudflare Worker). Real JSON-in/out APIs (`recommendations`,
/// `draft_verdict`, …) are added as engine modules migrate in.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn core_smoke() -> String {
    "csa-core wasm alive".to_string()
}

/// Native counterpart of [`core_smoke`], so the desktop crate / tests can link the
/// same symbol without the wasm-bindgen glue.
#[cfg(not(target_arch = "wasm32"))]
pub fn core_smoke() -> String {
    "csa-core native alive".to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_smoke_links() {
        assert_eq!(super::core_smoke(), "csa-core native alive");
    }
}
