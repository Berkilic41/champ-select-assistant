use crate::champion_types::ChampionType;
use crate::team_analysis::TeamComposition;
use crate::types::{ItemData, RuneTree};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

/// A general, archetype-level baseline build. NOT pro/winrate data — a sensible
/// default so every champion (not just the ~17 in `builds_seed.json`) shows a
/// build. Surfaced with a "Genel öneri" label; never presented as proven.
#[derive(Debug, Clone, Deserialize)]
pub struct ArchetypeBuild {
    pub core_items: Vec<u32>,
    pub keystone: u32,
    pub primary_tree: u32,
    /// `[tree_id, rune1_id, rune2_id]`.
    pub secondary_runes: Vec<u32>,
    pub stat_shards: Vec<u32>,
    pub summoner_spells: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct ArchetypeBuildTable {
    builds: HashMap<String, ArchetypeBuild>,
}

/// Embedded at compile time (like the Draft IQ KB). All IDs are real DDragon IDs
/// reused from `builds_seed.json` so they always resolve to icons.
const ARCHETYPE_BUILDS_JSON: &str = include_str!("../resources/draft_iq/archetype_builds.json");

/// General baseline build for a KB archetype string (e.g. "burst_mage"), or
/// `None` for an unknown archetype. Parsed once and cached.
pub fn heuristic_build(archetype: &str) -> Option<ArchetypeBuild> {
    static TABLE: OnceLock<HashMap<String, ArchetypeBuild>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        serde_json::from_str::<ArchetypeBuildTable>(ARCHETYPE_BUILDS_JSON)
            .map(|t| t.builds)
            .unwrap_or_default()
    });
    table.get(&archetype.to_lowercase()).cloned()
}

/// Returns up to 3 situational item IDs based on enemy team composition.
/// Picks items whose tags match the weaknesses exposed by the enemy composition.
pub fn situational_item_ids(enemy: &TeamComposition, all_items: &[ItemData]) -> Vec<u32> {
    let mut tags: Vec<&str> = Vec::new();

    if enemy.is_ap_heavy {
        tags.push("SpellBlock");
    }
    if enemy.is_ad_heavy {
        tags.push("Armor");
    }
    if enemy.tanks >= 2 {
        // Bulk up to survive a tank-heavy frontline
        tags.push("Health");
    }
    if enemy.assassins >= 2 {
        // Bulk up vs burst
        tags.push("Health");
    }

    // If no situational need was identified, return empty — caller can show defaults
    if tags.is_empty() {
        return Vec::new();
    }

    all_items
        .iter()
        .filter(|item| {
            tags.iter()
                .any(|t| item.tags.iter().any(|it| it.as_str() == *t))
        })
        .take(3)
        .map(|i| i.id)
        .collect()
}

/// One defensive/counter itemization advisory vs the enemy composition.
/// `item_ids` is empty for categories without a reliable DDragon tag (anti-heal,
/// tenacity) — those are text-only suggestions until curated item meta exists.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CounterItemHint {
    pub category: String,
    pub reason: String,
    pub item_ids: Vec<u32>,
}

/// Build defensive counter-itemization advice from the enemy composition.
/// `enemy_sustain` / `enemy_hard_cc` are KB-derived (utility_tags "sustain",
/// summed hard-CC count) and passed in by the command layer.
pub fn counter_item_advice(
    enemy: &TeamComposition,
    enemy_sustain: bool,
    enemy_hard_cc: u32,
    all_items: &[ItemData],
    // True when the local pick is a squishy carry (marksman/mage/assassin). Such
    // champions DON'T build pure-tank counter items, so we give them the stat need as
    // text (naming carry-survival items) instead of misleading tank-item icons.
    candidate_squishy_carry: bool,
) -> Vec<CounterItemHint> {
    // Completed (non-component) Summoner's Rift items by tag — the cache is already
    // SR-only. For squishy carries we suppress the icons (the tank-ish tag picks would
    // be wrong for an ADC); the reason text names the right carry items instead.
    let pick = |tag: &str| -> Vec<u32> {
        if candidate_squishy_carry {
            return Vec::new();
        }
        all_items
            .iter()
            .filter(|it| it.is_completed && it.tags.iter().any(|t| t.as_str() == tag))
            .take(2)
            .map(|i| i.id)
            .collect()
    };

    let mut out: Vec<CounterItemHint> = Vec::new();

    if enemy.is_ap_heavy {
        let reason = if candidate_squishy_carry {
            "Düşman AP — Maw/Mercurial/Wit's End gibi carry MR'ı düşün (tank MR alma)"
        } else {
            "Düşman AP ağırlıklı — magic resist al"
        };
        out.push(CounterItemHint {
            category: "MR".to_string(),
            reason: reason.to_string(),
            item_ids: pick("SpellBlock"),
        });
    }
    if enemy.is_ad_heavy {
        let reason = if candidate_squishy_carry {
            "Düşman AD — Guardian Angel / Zhonya gibi carry savunması düşün (tank zırhı alma)"
        } else {
            "Düşman AD ağırlıklı — zırh al"
        };
        out.push(CounterItemHint {
            category: "Armor".to_string(),
            reason: reason.to_string(),
            item_ids: pick("Armor"),
        });
    }
    if enemy.assassins >= 2 || enemy.tanks >= 2 {
        let reason = if enemy.assassins >= 2 {
            if candidate_squishy_carry {
                "Burst tehdidi — Guardian Angel / Sterak gibi carry-savunma (tank can alma)"
            } else {
                "Burst tehdidi — can/dayanıklılık al"
            }
        } else {
            "Tank-ağırlıklı frontline — can + penetrasyon düşün"
        };
        out.push(CounterItemHint {
            category: "Health".to_string(),
            reason: reason.to_string(),
            item_ids: pick("Health"),
        });
    }
    if enemy_sustain {
        out.push(CounterItemHint {
            category: "Anti-heal".to_string(),
            reason: "Düşmanda iyileşme var — anti-heal (Grievous Wounds) düşün".to_string(),
            item_ids: vec![],
        });
    }
    if enemy_hard_cc >= 3 {
        out.push(CounterItemHint {
            category: "Tenacity".to_string(),
            reason: "Yoğun sert CC — tenacity/QSS düşün (Mercury, Cleanse)".to_string(),
            item_ids: vec![],
        });
    }

    out
}

/// Suggests the primary rune tree and keystone for a champion based on its type(s).
/// Returns `(primary_rune_tree_id, keystone_id)`.
/// Falls back to Precision (8000) / 0 when data is missing.
pub fn suggest_rune_tree(champ_types: &[ChampionType], all_trees: &[RuneTree]) -> (u32, u32) {
    let primary_key = match champ_types.first() {
        Some(ChampionType::Tank) | Some(ChampionType::Fighter) => "Resolve",
        Some(ChampionType::Assassin) => "Domination",
        Some(ChampionType::Mage) => "Sorcery",
        _ => "Precision", // Marksman, Support, unknown
    };

    let tree = all_trees.iter().find(|t| t.key == primary_key);
    let tree_id = tree.map(|t| t.id).unwrap_or(8000);
    // First slot = keystone row; pick the first keystone in that row
    let keystone_id = tree
        .and_then(|t| t.slots.first())
        .and_then(|slot| slot.first())
        .map(|r| r.id)
        .unwrap_or(0);

    (tree_id, keystone_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every archetype present in `champions.json` must have a general build so no
    /// recommended champion is left without one.
    #[test]
    fn heuristic_build_covers_all_archetypes() {
        let archetypes = [
            "marksman",
            "vanguard",
            "diver",
            "skirmisher",
            "juggernaut",
            "assassin",
            "enchanter",
            "burst_mage",
            "control_mage",
            "catcher",
            "artillery",
            "specialist",
        ];
        for a in archetypes {
            let b = heuristic_build(a).unwrap_or_else(|| panic!("'{a}' için genel build olmalı"));
            assert!(!b.core_items.is_empty(), "'{a}' core_items dolu olmalı");
            assert!(b.keystone > 0, "'{a}' keystone set olmalı");
            assert_eq!(
                b.summoner_spells.len(),
                2,
                "'{a}' iki summoner spell olmalı"
            );
            assert_eq!(b.stat_shards.len(), 3, "'{a}' üç stat shard olmalı");
        }
    }

    #[test]
    fn heuristic_build_is_case_insensitive() {
        assert!(heuristic_build("Burst_Mage").is_some());
        assert!(heuristic_build("MARKSMAN").is_some());
    }

    #[test]
    fn heuristic_build_unknown_archetype_is_none() {
        assert!(heuristic_build("totally_unknown").is_none());
        assert!(heuristic_build("").is_none());
    }
}
