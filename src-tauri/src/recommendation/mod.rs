pub mod ban_advisor;
pub mod build_advisor;
pub mod champion_types;
pub mod draft_iq;
pub mod engine;
pub mod models;
pub mod scoring;
#[cfg(test)]
mod snapshot_tests;
pub mod team_analysis;
#[cfg(test)]
mod tests;

pub use ban_advisor::BanSuggestion;
pub use engine::compute_recommendations;
pub use models::Recommendation;
// MetaRate + MatchupEntry re-exported for command-layer callers building ScoringContext.
#[allow(unused_imports)]
pub use scoring::{MatchupEntry, MetaRate, ScoringContext, ScoringWeights};
