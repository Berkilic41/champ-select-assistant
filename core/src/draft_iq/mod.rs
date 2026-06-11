pub mod analyzer;
pub mod archetype;
pub mod combos;
pub mod game_plan;
pub mod narrative;

#[cfg(test)]
mod tests;

use archetype::{load_archetypes, ChampionArchetype};
use combos::{load_combos, ComboDirectory};
use std::collections::HashMap;

// Paths are relative to this file (src/recommendation/draft_iq/mod.rs).
// ../../../resources/ = src-tauri/resources/
const CHAMPIONS_JSON: &str = include_str!("../../resources/draft_iq/champions.json");
const COMBOS_JSON: &str = include_str!("../../resources/draft_iq/combos.json");

#[derive(Debug)]
pub struct DraftKnowledgeBase {
    pub archetypes: HashMap<String, ChampionArchetype>,
    pub combos: ComboDirectory,
}

impl DraftKnowledgeBase {
    pub fn load() -> anyhow::Result<Self> {
        let archetypes = load_archetypes(CHAMPIONS_JSON)?;
        let combos = load_combos(COMBOS_JSON)?;
        Ok(Self { archetypes, combos })
    }

    pub fn get_archetype(&self, champion_key: &str) -> Option<&ChampionArchetype> {
        self.archetypes.get(champion_key)
    }
}
