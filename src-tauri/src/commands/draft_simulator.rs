use crate::db::champion_repo;
use crate::errors::AppError;
use crate::lcu::session::TeamSlot;
use crate::lcu::{parse_session, ChampSelectState};
use crate::recommendation::draft_fork::{compare_fork, DraftFork};
use crate::recommendation::draft_iq::archetype::{ChampionArchetype, DamageProfile};
use crate::recommendation::draft_iq::DraftKnowledgeBase;
use crate::recommendation::draft_simulator::{
    simulate, DamageType, DraftSimInput, DraftSimMove, DraftSimResult, DraftSimState, SimChampion,
};
use crate::AppState;
use std::collections::{HashMap, HashSet};
use tauri::State;

fn parse_session_arg(session_json: serde_json::Value) -> Result<ChampSelectState, AppError> {
    if session_json.get("actions").is_some() {
        parse_session(&session_json)
            .ok_or_else(|| AppError::Other("Geçersiz session JSON".to_string()))
    } else {
        Ok(serde_json::from_value(session_json)?)
    }
}

fn damage_type(profile: &DamageProfile) -> DamageType {
    if profile.true_damage >= 0.45
        && profile.true_damage >= profile.ad
        && profile.true_damage >= profile.ap
    {
        DamageType::True
    } else if profile.ad >= 0.35 && profile.ap >= 0.35 {
        DamageType::Mixed
    } else if profile.ap > profile.ad {
        DamageType::Ap
    } else {
        DamageType::Ad
    }
}

fn combo_partner_ids(
    key: &str,
    key_to_id: &HashMap<String, u32>,
    kb: &DraftKnowledgeBase,
) -> Vec<u32> {
    let mut ids: Vec<u32> = kb
        .combos
        .all_pairs()
        .iter()
        .filter_map(|pair| {
            let partner = if pair.a.eq_ignore_ascii_case(key) {
                Some(pair.b.as_str())
            } else if pair.b.eq_ignore_ascii_case(key) {
                Some(pair.a.as_str())
            } else {
                None
            }?;
            key_to_id.get(&partner.to_lowercase()).copied()
        })
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn to_sim_champion(
    champion_id: u32,
    id_to_key: &HashMap<u32, String>,
    key_to_id: &HashMap<String, u32>,
    kb: &DraftKnowledgeBase,
) -> Option<SimChampion> {
    let key = id_to_key.get(&champion_id)?;
    let arch: &ChampionArchetype = kb.get_archetype(key)?;
    Some(SimChampion {
        champion_id,
        champion_key: key.clone(),
        archetype: arch.archetype.clone(),
        damage: damage_type(&arch.damage_profile),
        combo_partner_ids: combo_partner_ids(key, key_to_id, kb),
    })
}

fn slot_pick(slot: &TeamSlot, include_intent: bool) -> Option<u32> {
    if slot.champion_id > 0 {
        Some(slot.champion_id)
    } else if include_intent && slot.intent_champion_id > 0 {
        Some(slot.intent_champion_id)
    } else {
        None
    }
}

fn build_sim_state(
    session: &ChampSelectState,
    id_to_key: &HashMap<u32, String>,
    key_to_id: &HashMap<String, u32>,
    kb: &DraftKnowledgeBase,
) -> DraftSimState {
    let my_team = session
        .my_team
        .iter()
        .filter_map(|slot| {
            let is_local_unlocked = slot.cell_id == session.my_cell_id && !slot.is_locked;
            let champion_id = slot_pick(slot, !is_local_unlocked)?;
            to_sim_champion(champion_id, id_to_key, key_to_id, kb)
        })
        .collect();

    let enemy_team = session
        .their_team
        .iter()
        .filter_map(|slot| {
            let champion_id = slot_pick(slot, true)?;
            to_sim_champion(champion_id, id_to_key, key_to_id, kb)
        })
        .collect();

    DraftSimState {
        my_team,
        enemy_team,
        blind: session.queue_id == 430 || session.local_player.assigned_position.is_empty(),
        first_pick: session.pick_order <= 1,
    }
}

fn unavailable_champion_ids(session: &ChampSelectState) -> HashSet<u32> {
    session
        .my_team
        .iter()
        .filter_map(|slot| {
            let is_local_unlocked = slot.cell_id == session.my_cell_id && !slot.is_locked;
            slot_pick(slot, !is_local_unlocked)
        })
        .chain(
            session
                .their_team
                .iter()
                .filter_map(|slot| slot_pick(slot, true)),
        )
        .chain(session.my_bans.iter().copied())
        .chain(session.their_bans.iter().copied())
        .filter(|id| *id > 0)
        .collect()
}

fn candidate_move(
    champion_id: u32,
    session: &ChampSelectState,
    id_to_key: &HashMap<u32, String>,
    key_to_id: &HashMap<String, u32>,
    kb: &DraftKnowledgeBase,
) -> Option<DraftSimMove> {
    to_sim_champion(champion_id, id_to_key, key_to_id, kb).map(|champion| DraftSimMove {
        champion,
        position: Some(session.local_player.assigned_position.clone())
            .filter(|pos| !pos.is_empty()),
    })
}

async fn sim_resolution_maps(
    state: &State<'_, AppState>,
) -> Result<(HashMap<u32, String>, HashMap<String, u32>), AppError> {
    let champions = {
        let db = state.db.lock().await;
        champion_repo::list_all(&db)?
    };

    let id_to_key: HashMap<u32, String> = champions
        .into_iter()
        .filter_map(|champ| {
            u32::try_from(champ.champion_id)
                .ok()
                .map(|id| (id, champ.key))
        })
        .collect();
    let key_to_id: HashMap<String, u32> = id_to_key
        .iter()
        .map(|(id, key)| (key.to_lowercase(), *id))
        .collect();
    Ok((id_to_key, key_to_id))
}

#[tauri::command]
pub async fn get_draft_simulation(
    session_json: serde_json::Value,
    candidate_ids: Vec<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<DraftSimResult>, AppError> {
    let session = parse_session_arg(session_json)?;
    if candidate_ids.is_empty() {
        return Ok(Vec::new());
    }

    let (id_to_key, key_to_id) = sim_resolution_maps(&state).await?;
    let kb = state.draft_iq.clone();
    let sim_state = build_sim_state(&session, &id_to_key, &key_to_id, &kb);

    let unavailable = unavailable_champion_ids(&session);

    let mut seen = HashSet::new();
    let candidate_moves: Vec<DraftSimMove> = candidate_ids
        .into_iter()
        .filter(|id| *id > 0 && !unavailable.contains(id) && seen.insert(*id))
        .filter_map(|id| candidate_move(id, &session, &id_to_key, &key_to_id, &kb))
        .collect();

    let input = DraftSimInput {
        state: sim_state,
        candidate_moves,
    };
    Ok(simulate(&input))
}

#[tauri::command]
pub async fn get_draft_fork(
    session_json: serde_json::Value,
    option_a_id: u32,
    option_b_id: u32,
    state: State<'_, AppState>,
) -> Result<Option<DraftFork>, AppError> {
    if option_a_id == 0 || option_b_id == 0 || option_a_id == option_b_id {
        return Ok(None);
    }

    let session = parse_session_arg(session_json)?;
    let unavailable = unavailable_champion_ids(&session);
    if unavailable.contains(&option_a_id) || unavailable.contains(&option_b_id) {
        return Ok(None);
    }

    let (id_to_key, key_to_id) = sim_resolution_maps(&state).await?;
    let kb = state.draft_iq.clone();
    let sim_state = build_sim_state(&session, &id_to_key, &key_to_id, &kb);
    let Some(option_a) = candidate_move(option_a_id, &session, &id_to_key, &key_to_id, &kb) else {
        return Ok(None);
    };
    let Some(option_b) = candidate_move(option_b_id, &session, &id_to_key, &key_to_id, &kb) else {
        return Ok(None);
    };

    Ok(Some(compare_fork(&sim_state, &option_a, &option_b)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_serialized_state_without_raw_actions() {
        let state = ChampSelectState {
            my_cell_id: 1,
            local_player: TeamSlot {
                cell_id: 1,
                champion_id: 0,
                intent_champion_id: 0,
                assigned_position: "middle".to_string(),
                is_locked: false,
            },
            my_team: vec![],
            their_team: vec![],
            my_bans: vec![],
            their_bans: vec![],
            phase: "BAN_PICK".to_string(),
            time_left_ms: 10_000,
            action_type: "pick".to_string(),
            queue_id: 420,
            pick_order: 2,
        };

        let parsed = parse_session_arg(serde_json::to_value(state).unwrap()).unwrap();
        assert_eq!(parsed.action_type, "pick");
        assert_eq!(parsed.pick_order, 2);
    }

    #[test]
    fn slot_pick_excludes_intent_when_requested() {
        let slot = TeamSlot {
            cell_id: 1,
            champion_id: 0,
            intent_champion_id: 99,
            assigned_position: "middle".to_string(),
            is_locked: false,
        };
        assert_eq!(slot_pick(&slot, false), None);
        assert_eq!(slot_pick(&slot, true), Some(99));
    }

    #[test]
    fn damage_mapping_detects_mixed_damage() {
        let profile = DamageProfile {
            ad: 0.45,
            ap: 0.45,
            true_damage: 0.1,
        };
        assert_eq!(damage_type(&profile), DamageType::Mixed);
    }

    #[test]
    fn raw_lcu_session_still_routes_to_parser() {
        let raw = json!({
            "localPlayerCellId": 2,
            "myTeam": [
                {"cellId": 2, "championId": 0, "championPickIntent": 0, "assignedPosition": "middle"}
            ],
            "theirTeam": [],
            "bans": {"myTeamBans": [], "theirTeamBans": []},
            "actions": [],
            "timer": {"phase": "BAN_PICK", "adjustedTimeLeftInPhase": 27000},
            "gameConfig": {"queueId": 420}
        });
        let parsed = parse_session_arg(raw).unwrap();
        assert_eq!(parsed.my_cell_id, 2);
        assert_eq!(parsed.queue_id, 420);
    }
}
