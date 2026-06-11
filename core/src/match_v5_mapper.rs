//! Match-V5 JSON mapper.
//!
//! This is pure glue between Riot's raw Match-V5 detail shape and the
//! simplified `MatchV5` model consumed by the aggregation engine. It stays out
//! of the command layer so raw JSON schema-drift tests can run without network,
//! DB, or Tauri state.

use crate::match_v5_aggregator::{MatchV5, MatchV5Participant};
use serde_json::Value;

pub fn normalize_patch(game_version: &str) -> String {
    let mut parts = game_version.split('.');
    match (parts.next(), parts.next()) {
        (Some(major), Some(minor)) if !major.is_empty() && !minor.is_empty() => {
            format!("{major}.{minor}")
        }
        _ => "unknown".to_string(),
    }
}

pub fn parse_rune_ids(participant: &Value) -> Vec<u32> {
    participant["perks"]["styles"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|style| style["selections"].as_array().into_iter().flatten())
        .filter_map(|selection| selection["perk"].as_u64().map(|id| id as u32))
        .collect()
}

pub fn parse_items(participant: &Value) -> Vec<u32> {
    (0..=6)
        .map(|idx| {
            participant
                .get(format!("item{idx}"))
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32
        })
        .collect()
}

/// Champion ids banned across both teams (`info.teams[].bans[].championId`).
/// Riot uses `-1` for an empty ban slot — filtered out.
pub fn parse_bans(info: &Value) -> Vec<u32> {
    info["teams"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|team| team["bans"].as_array().into_iter().flatten())
        .filter_map(|b| b["championId"].as_i64())
        .filter(|&id| id > 0)
        .map(|id| id as u32)
        .collect()
}

pub fn match_v5_from_detail(detail: &Value, fallback_id: &str) -> Option<MatchV5> {
    let info = detail.get("info")?;
    let match_id = detail["metadata"]["matchId"]
        .as_str()
        .unwrap_or(fallback_id)
        .to_string();
    let queue_id = info["queueId"].as_u64().unwrap_or(0) as u32;
    let patch = normalize_patch(info["gameVersion"].as_str().unwrap_or("unknown"));
    let participants = info["participants"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| MatchV5Participant {
            participant_id: p["participantId"].as_u64().unwrap_or(0) as u32,
            champion_id: p["championId"].as_u64().unwrap_or(0) as u32,
            team_id: p["teamId"].as_u64().unwrap_or(0) as u32,
            team_position: p["teamPosition"].as_str().unwrap_or("").to_string(),
            win: p["win"].as_bool().unwrap_or(false),
            kills: p["kills"].as_u64().unwrap_or(0) as u32,
            deaths: p["deaths"].as_u64().unwrap_or(0) as u32,
            assists: p["assists"].as_u64().unwrap_or(0) as u32,
            items: parse_items(p),
            runes: parse_rune_ids(p),
            summoner_spells: vec![
                p["summoner1Id"].as_u64().unwrap_or(0) as u32,
                p["summoner2Id"].as_u64().unwrap_or(0) as u32,
            ],
        })
        .collect();

    Some(MatchV5 {
        match_id,
        queue_id,
        patch,
        participants,
        bans: parse_bans(info),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn patch_normalization_keeps_major_minor_only() {
        assert_eq!(normalize_patch("16.10.1"), "16.10");
        assert_eq!(normalize_patch("16.10.123.456"), "16.10");
        assert_eq!(normalize_patch("unknown"), "unknown");
        assert_eq!(normalize_patch(""), "unknown");
    }

    #[test]
    fn raw_detail_maps_to_simplified_match_v5() {
        let detail = json!({
            "metadata": { "matchId": "EUW1_1" },
            "info": {
                "queueId": 420,
                "gameVersion": "16.10.1",
                "participants": [{
                    "participantId": 1,
                    "championId": 266,
                    "teamId": 100,
                    "teamPosition": "TOP",
                    "win": true,
                    "kills": 3,
                    "deaths": 1,
                    "assists": 4,
                    "item0": 1055,
                    "item1": 3047,
                    "item2": 0,
                    "item6": 3340,
                    "summoner1Id": 4,
                    "summoner2Id": 12,
                    "perks": {
                        "styles": [{
                            "selections": [
                                { "perk": 8010 },
                                { "perk": 9111 }
                            ]
                        }]
                    }
                }]
            }
        });

        let mapped = match_v5_from_detail(&detail, "fallback").expect("valid detail");
        assert_eq!(mapped.match_id, "EUW1_1");
        assert_eq!(mapped.queue_id, 420);
        assert_eq!(mapped.patch, "16.10");
        assert_eq!(mapped.participants.len(), 1);
        let participant = &mapped.participants[0];
        assert_eq!(participant.champion_id, 266);
        assert_eq!(participant.team_position, "TOP");
        assert_eq!(participant.items, vec![1055, 3047, 0, 0, 0, 0, 3340]);
        assert_eq!(participant.runes, vec![8010, 9111]);
        assert_eq!(participant.summoner_spells, vec![4, 12]);
    }

    #[test]
    fn bans_are_parsed_from_both_teams_skipping_empty_slots() {
        let detail = json!({
            "metadata": { "matchId": "EUW1_2" },
            "info": {
                "queueId": 420,
                "gameVersion": "16.10.1",
                "participants": [],
                "teams": [
                    { "bans": [ { "championId": 64 }, { "championId": -1 } ] },
                    { "bans": [ { "championId": 157 } ] }
                ]
            }
        });
        let mapped = match_v5_from_detail(&detail, "fb").expect("valid");
        assert_eq!(mapped.bans, vec![64, 157], "both teams, -1 slot filtered");
    }

    #[test]
    fn malformed_without_info_returns_none() {
        let detail = json!({ "metadata": { "matchId": "EUW1_missing" } });
        assert!(match_v5_from_detail(&detail, "fallback").is_none());
    }

    #[test]
    fn missing_optional_participant_fields_are_zero_or_empty() {
        let detail = json!({
            "metadata": {},
            "info": {
                "queueId": 420,
                "participants": [{
                    "championId": 238,
                    "teamPosition": null,
                    "perks": { "styles": [] }
                }]
            }
        });

        let mapped = match_v5_from_detail(&detail, "fallback-id").expect("info exists");
        assert_eq!(mapped.match_id, "fallback-id");
        assert_eq!(mapped.patch, "unknown");
        let participant = &mapped.participants[0];
        assert_eq!(participant.participant_id, 0);
        assert_eq!(participant.champion_id, 238);
        assert_eq!(participant.team_id, 0);
        assert_eq!(participant.team_position, "");
        assert_eq!(participant.items, vec![0, 0, 0, 0, 0, 0, 0]);
        assert!(participant.runes.is_empty());
        assert_eq!(participant.summoner_spells, vec![0, 0]);
    }
}
