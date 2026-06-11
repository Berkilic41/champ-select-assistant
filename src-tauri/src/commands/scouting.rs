//! Lobby scouting command (Faz 3). Resolves the pre-fetched enemy champion-pool
//! summaries to KB archetypes and runs the pure scouting builder. Takes the pools
//! as input (the frontend already fetches them via `get_enemy_champion_pools`),
//! so this never re-implements the LCU history fetch and touches no Codex file.

use crate::commands::champ_select::EnemyPoolSummary;
use crate::db::champion_repo;
use crate::errors::AppError;
use crate::recommendation::draft_iq::DraftKnowledgeBase;
use crate::recommendation::scouting::{build_scouting_report, ScoutPoolInput, ScoutingReport};
use crate::AppState;
use std::collections::HashMap;
use tauri::State;

/// Pure conversion: already-fetched enemy pools + a champion-id→key map + the KB
/// → scouting report. No LCU/network/IO here — the pools come in pre-fetched, so
/// scouting never re-runs the (rate-limited) LCU history lookup.
fn scout_from_pools(
    pools: &[EnemyPoolSummary],
    champ_keys: &HashMap<u32, String>,
    kb: &DraftKnowledgeBase,
) -> ScoutingReport {
    let inputs: Vec<ScoutPoolInput> = pools
        .iter()
        .map(|p| {
            let archetype = champ_keys
                .get(&p.top_champion_id)
                .and_then(|key| kb.get_archetype(key))
                .map(|a| a.archetype.clone());
            ScoutPoolInput {
                cell_id: p.cell_id,
                champion_id: p.top_champion_id,
                champion_key: p.top_champion_key.clone(),
                play_rate: p.play_rate,
                game_count: p.game_count,
                archetype,
            }
        })
        .collect();
    build_scouting_report(&inputs)
}

/// Build a lobby scouting read (enemy playstyle profiles + explained ban targets)
/// from already-fetched enemy pool summaries. One short DB read for the champion
/// key map, then pure analysis; no LCU/network here.
#[tauri::command]
pub async fn get_lobby_scouting(
    pools: Vec<EnemyPoolSummary>,
    state: State<'_, AppState>,
) -> Result<ScoutingReport, AppError> {
    let kb = state.draft_iq.clone();

    let champ_keys: HashMap<u32, String> = {
        let db = state.db.lock().await;
        champion_repo::list_all(&db)?
            .into_iter()
            .map(|c| (c.champion_id as u32, c.key))
            .collect()
    };

    Ok(scout_from_pools(&pools, &champ_keys, &kb))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(cell: i32, id: u32, key: &str, play: f32, games: u32) -> EnemyPoolSummary {
        EnemyPoolSummary {
            cell_id: cell,
            top_champion_id: id,
            top_champion_key: key.to_string(),
            play_rate: play,
            game_count: games,
        }
    }

    #[test]
    fn maps_pools_to_report_with_kb_archetype() {
        let kb = DraftKnowledgeBase::load().expect("KB load");
        let mut keys = HashMap::new();
        keys.insert(238u32, "Zed".to_string()); // Zed → assassin in the KB
        let pools = vec![summary(5, 238, "Zed", 0.70, 12)];

        let report = scout_from_pools(&pools, &keys, &kb);
        assert_eq!(report.enemies.len(), 1);
        assert_eq!(report.enemies[0].top_champion_id, 238);
        assert_eq!(
            report.enemies[0].threat, "yüksek",
            "OTP assassin → high threat"
        );
        assert!(report.ban_targets.iter().any(|b| b.champion_id == 238));
    }

    #[test]
    fn missing_champion_key_yields_neutral_profile() {
        let kb = DraftKnowledgeBase::load().expect("KB load");
        let keys: HashMap<u32, String> = HashMap::new(); // no id→key mapping
        let pools = vec![summary(5, 238, "Zed", 0.70, 12)];

        let report = scout_from_pools(&pools, &keys, &kb);
        // No key → no archetype resolved → neutral profile, not auto-banned.
        assert_eq!(report.enemies[0].threat, "orta");
        assert!(report.enemies[0]
            .playstyle_tags
            .iter()
            .any(|t| t == "dengeli profil"));
        assert!(report.ban_targets.is_empty());
    }

    #[test]
    fn empty_pools_yield_partial_empty_report() {
        let kb = DraftKnowledgeBase::load().expect("KB load");
        let keys = HashMap::new();
        let report = scout_from_pools(&[], &keys, &kb);
        assert!(report.enemies.is_empty());
        assert!(report.ban_targets.is_empty());
        assert!(report.partial);
    }
}
