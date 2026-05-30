use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct DamageProfile {
    pub ad: f32,
    pub ap: f32,
    #[serde(rename = "true")]
    pub true_damage: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CcProfile {
    pub has_hard_cc: bool,
    pub hard_cc_count: u8,
    pub primary_cc: Vec<String>,
}

/// Coarse heuristic for champion strength across the three game phases.
/// Values are 0.0–1.0 estimates, not precise mathematical models.
/// See SCHEMA.md for sourcing guidelines.
#[derive(Debug, Clone, Deserialize)]
pub struct PowerCurve {
    pub early: f32,
    pub mid: f32,
    pub late: f32,
}

impl Default for PowerCurve {
    fn default() -> Self {
        Self {
            early: 0.6,
            mid: 0.6,
            late: 0.6,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChampionArchetype {
    pub champion_id: u32,
    pub archetype: String,
    pub damage_profile: DamageProfile,
    pub cc: CcProfile,
    pub mobility: String,
    pub engage_role: String,
    pub peel_capability: String,
    pub blind_safety: f32,
    pub execution_difficulty: u8,
    pub win_condition: String,
    pub ult_type: String,
    pub confidence: String,
    /// Coarse game-phase power curve. Defaults to [0.6, 0.6, 0.6] when absent.
    #[serde(default)]
    pub power_curve: PowerCurve,
    /// Archetypes this champion tends to counter (signal, not hard rule).
    #[serde(default)]
    pub counters_archetypes: Vec<String>,
    /// Team utility contributions (e.g. "engage", "peel", "waveclear").
    #[serde(default)]
    pub utility_tags: Vec<String>,
}

pub fn load_archetypes(raw_json: &str) -> Result<HashMap<String, ChampionArchetype>> {
    let map: HashMap<String, ChampionArchetype> = serde_json::from_str(raw_json)?;
    Ok(map)
}
