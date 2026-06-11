//! Raw LCU `/lol-champ-select/v1/session` JSON → typed [`ChampSelectState`].
//!
//! Pure JSON transform (no I/O) moved from the host's `lcu/session.rs` so both
//! hosts (Rust/Tauri and Electron via WASM `parse_session_json`) parse sessions
//! with the SAME logic. The host re-exports it
//! (`pub use csa_core::session_parse::parse_session;`) so existing paths keep
//! working; its fixture tests stay host-side (they include_str! LCU fixtures).

use std::collections::HashSet;

use crate::types::{ChampSelectState, TeamSlot};

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
