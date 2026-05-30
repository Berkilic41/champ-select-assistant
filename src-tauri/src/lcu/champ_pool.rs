use crate::lcu::client::LcuClient;
use anyhow::Result;
use std::collections::HashMap;

/// Fetch the PUUID for a given LCU summonerId.
/// Returns an error if the LCU call fails or the `puuid` field is absent.
pub async fn fetch_summoner_puuid(client: &LcuClient, summoner_id: i64) -> Result<String> {
    let resp = client
        .get_raw(&format!("/lol-summoner/v1/summoners/{summoner_id}"))
        .await?;
    resp["puuid"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("LCU summoner yanıtında puuid eksik"))
}

/// Fetch last `count` match history entries for a PUUID via LCU.
pub async fn fetch_match_history(
    client: &LcuClient,
    puuid: &str,
    count: u32,
) -> Result<serde_json::Value> {
    client
        .get_raw(&format!(
            "/lol-match-history/v1/products/lol/{puuid}/matches?begIndex=0&endIndex={count}"
        ))
        .await
}

/// Parse a LCU match-history response and return `(champion_id, count)` pairs
/// sorted by count descending, for the participant identified by `puuid`.
///
/// Games where the puuid cannot be matched are silently skipped.
pub fn compute_champion_pool(history_json: &serde_json::Value, puuid: &str) -> Vec<(u32, u32)> {
    let games = match history_json["games"]["games"].as_array() {
        Some(g) => g,
        None => return vec![],
    };

    let mut counts: HashMap<u32, u32> = HashMap::new();

    for game in games {
        if let Some(champ_id) = find_champion_for_puuid(game, puuid) {
            *counts.entry(champ_id).or_insert(0) += 1;
        }
    }

    let mut result: Vec<(u32, u32)> = counts.into_iter().collect();
    result.sort_by_key(|b| std::cmp::Reverse(b.1));
    result
}

fn find_champion_for_puuid(game: &serde_json::Value, puuid: &str) -> Option<u32> {
    let identities = game["participantIdentities"].as_array()?;
    let participant_id = identities.iter().find_map(|id| {
        if id["player"]["puuid"].as_str() == Some(puuid) {
            id["participantId"].as_i64()
        } else {
            None
        }
    })?;

    let participants = game["participants"].as_array()?;
    participants.iter().find_map(|p| {
        if p["participantId"].as_i64() == Some(participant_id) {
            let id = p["championId"].as_u64()? as u32;
            if id != 0 {
                Some(id)
            } else {
                None
            }
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_history(entries: &[(i64, &str, u64)]) -> serde_json::Value {
        // entries: (participant_id, puuid, champion_id)
        let games: Vec<serde_json::Value> = entries
            .chunks(3)
            .enumerate()
            .map(|(game_idx, _)| {
                let game_entries: Vec<serde_json::Value> = entries
                    .iter()
                    .filter(|(pid, _, _)| {
                        // Assign each group of participants to a game by index
                        let _gidx = game_idx;
                        *pid > 0
                    })
                    .map(|(pid, puuid, champ)| {
                        json!({
                            "participantIdentities": [{"participantId": *pid, "player": {"puuid": *puuid}}],
                            "participants": [{"participantId": *pid, "championId": *champ}]
                        })
                    })
                    .take(1)
                    .collect();
                game_entries.into_iter().next().unwrap_or(json!({}))
            })
            .collect();
        json!({ "games": { "games": games } })
    }

    fn make_game(puuid: &str, champ_id: u64) -> serde_json::Value {
        json!({
            "participantIdentities": [
                {"participantId": 1, "player": {"puuid": puuid}},
                {"participantId": 2, "player": {"puuid": "other-puuid"}}
            ],
            "participants": [
                {"participantId": 1, "championId": champ_id},
                {"participantId": 2, "championId": 99}
            ]
        })
    }

    #[test]
    fn compute_pool_counts_correctly() {
        let games = vec![
            make_game("p1", 222), // Jinx
            make_game("p1", 222), // Jinx
            make_game("p1", 51),  // Caitlyn
            make_game("p1", 222), // Jinx
        ];
        let history = json!({ "games": { "games": games } });

        let pool = compute_champion_pool(&history, "p1");
        assert!(!pool.is_empty());
        assert_eq!(pool[0].0, 222, "Jinx en çok oynanmış olmalı");
        assert_eq!(pool[0].1, 3, "Jinx 3 oyunda görünmeli");
        assert_eq!(pool[1].0, 51, "Caitlyn ikinci olmalı");
        assert_eq!(pool[1].1, 1);
    }

    #[test]
    fn compute_pool_skips_unknown_puuid() {
        let games = vec![make_game("p1", 222)];
        let history = json!({ "games": { "games": games } });

        let pool = compute_champion_pool(&history, "other-puuid");
        // other-puuid maps to championId=99 in participant 2
        assert!(!pool.is_empty());
        assert_eq!(pool[0].0, 99);
    }

    #[test]
    fn compute_pool_empty_when_no_games() {
        let history = json!({ "games": { "games": [] } });
        let pool = compute_champion_pool(&history, "p1");
        assert!(pool.is_empty(), "Boş tarih için boş pool dönmeli");
    }

    #[test]
    fn compute_pool_handles_missing_puuid() {
        let games = vec![make_game("p1", 238)];
        let history = json!({ "games": { "games": games } });

        let pool = compute_champion_pool(&history, "nonexistent");
        assert!(pool.is_empty(), "Eşleşmeyen puuid için boş pool");
    }

    #[test]
    fn compute_pool_sorted_by_count_desc() {
        let games = vec![
            make_game("p1", 10),
            make_game("p1", 10),
            make_game("p1", 10),
            make_game("p1", 20),
            make_game("p1", 20),
            make_game("p1", 30),
        ];
        let history = json!({ "games": { "games": games } });

        let pool = compute_champion_pool(&history, "p1");
        assert_eq!(pool[0].1, 3, "En çok oynanan önce gelmeli");
        assert_eq!(pool[1].1, 2);
        assert_eq!(pool[2].1, 1);

        for window in pool.windows(2) {
            assert!(window[0].1 >= window[1].1, "Azalan sırayla sıralı olmalı");
        }
    }
}
