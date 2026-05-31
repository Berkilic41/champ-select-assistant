use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use ts_rs::TS;

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

/// Walk the `actions` array and return the local player's 1-indexed position in the
/// global pick sequence (both teams combined, sorted by action `id` within each group).
/// Returns `0` when the pick action has not yet appeared (e.g. still in ban phase).
fn parse_pick_order(v: &serde_json::Value, my_cell_id: i32) -> u8 {
    let mut order: u8 = 0;
    let Some(groups) = v["actions"].as_array() else {
        return 0;
    };
    for group in groups {
        let Some(actions) = group.as_array() else {
            continue;
        };
        let mut picks: Vec<&serde_json::Value> = actions
            .iter()
            .filter(|a| a["type"].as_str() == Some("pick"))
            .collect();
        picks.sort_by_key(|a| a["id"].as_i64().unwrap_or(0));
        for pick in picks {
            order = order.saturating_add(1);
            if pick["actorCellId"].as_i64().map(|c| c as i32) == Some(my_cell_id) {
                return order;
            }
        }
    }
    0
}

/// Parse a raw LCU `/lol-champ-select/v1/session` JSON value into a typed struct.
/// Returns `None` when the JSON doesn't look like a valid session.
pub fn parse_session(v: &serde_json::Value) -> Option<ChampSelectState> {
    let my_cell_id = v["localPlayerCellId"].as_i64()? as i32;

    let parse_team = |arr: &serde_json::Value| -> Vec<TeamSlot> {
        arr.as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|s| TeamSlot {
                        cell_id: s["cellId"].as_i64().unwrap_or(0) as i32,
                        champion_id: s["championId"].as_u64().unwrap_or(0) as u32,
                        intent_champion_id: s["championPickIntent"].as_u64().unwrap_or(0) as u32,
                        assigned_position: s["assignedPosition"].as_str().unwrap_or("").to_string(),
                        is_locked: false,
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let mut my_team = parse_team(&v["myTeam"]);
    let their_team = parse_team(&v["theirTeam"]);

    let my_bans = v["bans"]["myTeamBans"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_u64().map(|n| n as u32))
                .collect()
        })
        .unwrap_or_default();

    let their_bans = v["bans"]["theirTeamBans"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_u64().map(|n| n as u32))
                .collect()
        })
        .unwrap_or_default();

    // Collect cell_ids of completed pick actions
    let mut locked_cells: HashSet<i32> = HashSet::new();
    if let Some(action_groups) = v["actions"].as_array() {
        for group in action_groups {
            if let Some(actions) = group.as_array() {
                for action in actions {
                    if action["type"].as_str() == Some("pick")
                        && action["completed"].as_bool() == Some(true)
                    {
                        if let Some(cell) = action["actorCellId"].as_i64() {
                            locked_cells.insert(cell as i32);
                        }
                    }
                }
            }
        }
    }

    for slot in &mut my_team {
        slot.is_locked = locked_cells.contains(&slot.cell_id);
    }
    let mut their_team = their_team;
    for slot in &mut their_team {
        slot.is_locked = locked_cells.contains(&slot.cell_id);
    }

    let local_player = my_team
        .iter()
        .find(|s| s.cell_id == my_cell_id)
        .cloned()
        .unwrap_or_default();

    let phase = v["timer"]["phase"]
        .as_str()
        .unwrap_or("PLANNING")
        .to_string();
    let time_left_ms = v["timer"]["adjustedTimeLeftInPhase"]
        .as_u64()
        .unwrap_or(30_000);

    // Detect the current action type for the local player (ban/pick/"").
    let action_type = v["actions"]
        .as_array()
        .and_then(|groups| {
            for group in groups {
                if let Some(actions) = group.as_array() {
                    for action in actions {
                        let is_ours =
                            action["actorCellId"].as_i64().map(|c| c as i32) == Some(my_cell_id);
                        let in_progress = action["isInProgress"].as_bool() == Some(true);
                        let not_done = action["completed"].as_bool() != Some(true);
                        if is_ours && in_progress && not_done {
                            return action["type"].as_str().map(String::from);
                        }
                    }
                }
            }
            None
        })
        .unwrap_or_default();

    // Queue id is reported under gameConfig.queueId in /lol-champ-select/v1/session.
    // 0 = unknown (common when the field is absent in custom-game payloads).
    let queue_id = v["gameConfig"]["queueId"].as_u64().unwrap_or(0) as u32;

    let pick_order = parse_pick_order(v, my_cell_id);

    Some(ChampSelectState {
        my_cell_id,
        local_player,
        my_team,
        their_team,
        my_bans,
        their_bans,
        phase,
        time_left_ms,
        action_type,
        queue_id,
        pick_order,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_minimal_session() {
        let v = json!({
            "localPlayerCellId": 2,
            "myTeam": [
                {"cellId": 2, "championId": 99, "championPickIntent": 0,
                 "assignedPosition": "middle"}
            ],
            "theirTeam": [],
            "bans": {"myTeamBans": [], "theirTeamBans": []},
            "actions": [],
            "timer": {"phase": "BAN_PICK", "adjustedTimeLeftInPhase": 27000}
        });
        let state = parse_session(&v).expect("should parse");
        assert_eq!(state.my_cell_id, 2);
        assert_eq!(state.local_player.champion_id, 99);
        assert_eq!(state.phase, "BAN_PICK");
    }

    #[test]
    fn parse_missing_local_cell() {
        // Returns None when localPlayerCellId is absent
        let v = json!({"myTeam": []});
        assert!(parse_session(&v).is_none());
    }

    #[test]
    fn parse_pick_order_ban_phase_returns_zero() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/ban_acting.json")).unwrap();
        let state = parse_session(&v).expect("ban_acting fixture parse edilemedi");
        assert_eq!(
            state.pick_order, 0,
            "Ban fazında pick_order bilinmiyor, 0 dönmeli"
        );
    }

    #[test]
    fn parse_pick_order_pick_acting() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/pick_acting.json")).unwrap();
        let state = parse_session(&v).expect("pick_acting fixture parse edilemedi");
        assert_eq!(
            state.pick_order, 2,
            "pick_acting fixture'da oyuncu 2. sırada seçiyor, pick_order=2 bekleniyor"
        );
    }

    #[test]
    fn parse_pick_order_finalization() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/finalization.json")).unwrap();
        let state = parse_session(&v).expect("finalization fixture parse edilemedi");
        assert_eq!(
            state.pick_order, 2,
            "finalization fixture'da oyuncu 2. sırada seçti, pick_order=2 bekleniyor"
        );
    }

    /// Regression: the live flow is raw LCU → parse_session → emit (serialize) →
    /// frontend → get_recommendations (deserialize). The serialized ChampSelectState
    /// must NOT look like a raw LCU session (no "actions") and must round-trip back
    /// via `from_value`. Guards the get_recommendations parse-branch (Bug: live sent
    /// a serialized state into parse_session → "Geçersiz session JSON").
    #[test]
    fn parsed_state_roundtrips_for_get_recommendations() {
        let raw: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/pick_acting.json")).unwrap();
        let state = parse_session(&raw).expect("raw parse");

        let serialized = serde_json::to_value(&state).expect("serialize state");
        assert!(
            serialized.get("actions").is_none(),
            "serialized ChampSelectState must not carry raw LCU 'actions' (would mis-route to parse_session)"
        );

        let roundtripped: ChampSelectState =
            serde_json::from_value(serialized).expect("serialized state must deserialize back");
        assert_eq!(roundtripped.queue_id, state.queue_id);
        assert_eq!(roundtripped.action_type, state.action_type);
        assert_eq!(roundtripped.my_cell_id, state.my_cell_id);
    }

    #[test]
    fn parse_ban_acting_fixture() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/ban_acting.json")).unwrap();
        let state = parse_session(&v).expect("ban_acting fixture parse edilemedi");
        assert_eq!(state.my_cell_id, 2);
        assert_eq!(
            state.action_type, "ban",
            "Ban fazinda action_type 'ban' olmali"
        );
        assert_eq!(state.phase, "BAN_PICK");
        assert_eq!(state.local_player.assigned_position, "middle");
        assert_eq!(state.my_bans.len(), 1); // Zed banli
    }

    #[test]
    fn parse_pick_acting_fixture() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/pick_acting.json")).unwrap();
        let state = parse_session(&v).expect("pick_acting fixture parse edilemedi");
        assert_eq!(
            state.action_type, "pick",
            "Pick fazinda action_type 'pick' olmali"
        );
        assert_eq!(
            state.local_player.intent_champion_id, 238,
            "Hover'daki champion dogru"
        );
        assert!(
            !state.local_player.is_locked,
            "Pick tamamlanmadi, locked olmamali"
        );
    }

    #[test]
    fn parse_pick_watching_fixture() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/pick_watching.json")).unwrap();
        let state = parse_session(&v).expect("pick_watching fixture parse edilemedi");
        assert_eq!(
            state.action_type, "",
            "Baskasinin sirasi -- action_type bos olmali"
        );
        assert!(
            state.local_player.is_locked,
            "Oyuncu kilitli olmali (completed pick var)"
        );
    }

    #[test]
    fn parse_finalization_fixture() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/finalization.json")).unwrap();
        let state = parse_session(&v).expect("finalization fixture parse edilemedi");
        assert_eq!(state.phase, "FINALIZATION");
        assert_eq!(
            state.local_player.champion_id, 238,
            "Kilit fazinda champion_id dolu olmali"
        );
        assert!(
            state.local_player.is_locked,
            "Kilit fazinda oyuncu kilitli olmali"
        );
        let locked_count = state.my_team.iter().filter(|s| s.is_locked).count();
        assert!(
            locked_count >= 4,
            "En az 4 oyuncu kilitli olmali, got {}",
            locked_count
        );
    }
}
