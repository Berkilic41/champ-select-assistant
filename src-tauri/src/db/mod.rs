pub mod builds_repo;
pub mod champion_meta_repo;
pub mod champion_rates_repo;
pub mod champion_repo;
pub mod connection;
pub mod mastery_repo;
pub mod match_repo;
pub mod matchup_repo;
pub mod summoner_repo;
pub use connection::{open_db, run_migrations};

#[cfg(test)]
mod schema_parity;
#[cfg(test)]
mod tests;

refinery::embed_migrations!("migrations");
