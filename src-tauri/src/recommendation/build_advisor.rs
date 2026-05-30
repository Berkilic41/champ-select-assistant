use crate::ddragon::cdragon::{ItemData, RuneTree};
use crate::recommendation::champion_types::ChampionType;
use crate::recommendation::team_analysis::TeamComposition;

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
