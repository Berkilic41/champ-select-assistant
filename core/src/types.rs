//! Shared pure data DTOs — the structs that cross the I/O ↔ engine boundary.
//!
//! These are produced by the host I/O layer (SQLite repos, Data Dragon fetchers,
//! LCU session parsing, Riot API) and consumed by the pure engine. They live in
//! `core` so the engine modules can reference them without depending on the host's
//! I/O crates (rusqlite / reqwest). The host modules re-export them
//! (`pub use csa_core::types::X;`) so existing paths
//! (`crate::db::champion_repo::ChampionRecord`, …) keep working. Construction stays
//! in the host via struct literals (all fields are `pub`); only the type
//! definitions live here. No I/O, no inherent impls.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A champion's static identity row (DDragon-sourced, cached in SQLite `champions`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChampionRecord {
    pub champion_id: i64,
    pub key: String,
    pub name: String,
    pub title: String,
}

/// A single mastery row for a summoner + champion pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasteryRow {
    pub champion_id: i64,
    #[serde(rename = "mastery_level")]
    pub level: i64,
    #[serde(rename = "mastery_points")]
    pub points: i64,
    pub last_play_time: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct ChampSelectState {
    pub my_cell_id: i32,
    pub local_player: TeamSlot,
    pub my_team: Vec<TeamSlot>,
    pub their_team: Vec<TeamSlot>,
    pub my_bans: Vec<u32>,
    pub their_bans: Vec<u32>,
    pub phase: String,
    pub time_left_ms: u64,
    /// "ban" | "pick" | "" — the type of the local player's currently active action.
    pub action_type: String,
    /// Queue id from `gameData.queue.id`. Common values:
    /// `420` = Ranked Solo/Duo, `440` = Ranked Flex, `450` = ARAM, `0` = unknown.
    /// Used to switch scoring profiles (e.g. ARAM ignores lane matchup / role fit).
    pub queue_id: u32,
    /// 1-indexed position in the global pick sequence across both teams (1 = first pick ever).
    /// `0` = unknown (e.g. still in ban phase, pick action not yet present in session).
    pub pick_order: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct TeamSlot {
    pub cell_id: i32,
    pub champion_id: u32,
    pub intent_champion_id: u32,
    pub assigned_position: String,
    pub is_locked: bool,
}

/// A completed item (DDragon-sourced; only the fields the engine needs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemData {
    pub id: u32,
    pub name: String,
    pub tags: Vec<String>,
    pub total_gold: u32,
    /// true when `into` list is absent or empty (fully completed item)
    pub is_completed: bool,
}

/// A rune tree (DDragon runesReforged-sourced).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuneTree {
    pub id: u32,
    pub key: String,
    pub slots: Vec<Vec<RuneEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuneEntry {
    pub id: u32,
    pub key: String,
}

/// Aggregate per-champion stats for a summoner (Riot Match-V5 derived).
#[derive(Debug, Serialize, Deserialize)]
pub struct ChampionStats {
    pub champion_id: i64,
    pub games: u32,
    pub wins: u32,
    pub kda_avg: f32,
}
