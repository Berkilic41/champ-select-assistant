//! Pure parser helpers for LCU match-history and mastery responses.
//!
//! These functions are intentionally side-effect-free (no I/O, no DB) so they
//! can be unit-tested without a live League client.
//!
//! ⚠ The LCU match-history endpoint is not part of the official public Riot API.
//! Its format may change between client versions. Parsers must never panic on
//! unexpected shapes — unknown fields are skipped and errors are counted.

/// A single parsed match suitable for inserting into the `matches` DB table.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedLcuMatch {
    pub match_id: String, // "LCU_{gameId}" — prefixed to avoid clashes with Riot IDs
    pub champion_id: i64,
    pub position: Option<String>,
    pub win: bool,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub duration_secs: i64,
    pub queue_id: i64,
    pub played_at: i64, // Unix timestamp in seconds
}

/// A single parsed mastery entry for the `mastery` DB table.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedLcuMastery {
    pub champion_id: i64,
    pub level: i64,
    pub points: i64,
    pub last_play_time: Option<i64>,
}

// ── Match history parser ───────────────────────────────────────────────────

/// Parse an LCU `/lol-match-history/v1/products/lol/{puuid}/matches` response.
///
/// Returns only the games where `puuid` was a participant.  Games that cannot
/// be matched back to `puuid` via `participantIdentities` are silently skipped
/// (counted in the caller's `errors` counter instead of panicking).
pub fn parse_lcu_match_history(json: &serde_json::Value, puuid: &str) -> Vec<ParsedLcuMatch> {
    let games = match json["games"]["games"].as_array() {
        Some(g) => g,
        None => return vec![],
    };

    let mut result = Vec::new();

    for game in games {
        if let Some(m) = parse_single_game(game, puuid) {
            result.push(m);
        }
    }

    result
}

fn parse_single_game(game: &serde_json::Value, puuid: &str) -> Option<ParsedLcuMatch> {
    let game_id = game["gameId"].as_i64()?;
    let queue_id = game["queueId"].as_i64().unwrap_or(0);
    let duration_secs = game["gameDuration"].as_i64().unwrap_or(0);
    let played_at = game["gameCreation"].as_i64().unwrap_or(0) / 1000;

    // Find the participantId that belongs to the local player (by puuid)
    let identities = game["participantIdentities"].as_array()?;
    let local_participant_id = identities.iter().find_map(|id| {
        let player_puuid = id["player"]["puuid"].as_str()?;
        if player_puuid == puuid {
            id["participantId"].as_i64()
        } else {
            None
        }
    })?;

    // Find the participant data for that participantId
    let participants = game["participants"].as_array()?;
    let p = participants
        .iter()
        .find(|p| p["participantId"].as_i64() == Some(local_participant_id))?;

    let champion_id = p["championId"].as_i64()?;
    let win = p["stats"]["win"].as_bool().unwrap_or(false);
    let kills = p["stats"]["kills"].as_i64().unwrap_or(0);
    let deaths = p["stats"]["deaths"].as_i64().unwrap_or(0);
    let assists = p["stats"]["assists"].as_i64().unwrap_or(0);

    // Position derived from timeline.lane (e.g., "MIDDLE", "BOTTOM", "TOP", "JUNGLE")
    let position = p["timeline"]["lane"]
        .as_str()
        .filter(|s| !s.is_empty() && *s != "NONE")
        .map(str::to_string);

    Some(ParsedLcuMatch {
        match_id: format!("LCU_{game_id}"),
        champion_id,
        position,
        win,
        kills,
        deaths,
        assists,
        duration_secs,
        queue_id,
        played_at,
    })
}

// ── Mastery parser ─────────────────────────────────────────────────────────

/// Parse an LCU `/lol-champion-mastery/v1/local-player/champion-mastery` response.
///
/// Returns all entries where `championId` is parseable.
pub fn parse_lcu_mastery(json: &serde_json::Value) -> Vec<ParsedLcuMastery> {
    let entries = match json.as_array() {
        Some(a) => a,
        None => return vec![],
    };

    entries
        .iter()
        .filter_map(|e| {
            let champion_id = e["championId"].as_i64()?;
            let level = e["championLevel"].as_i64().unwrap_or(0);
            let points = e["championPoints"].as_i64().unwrap_or(0);
            let last_play_time = e["lastPlayTime"].as_i64();
            Some(ParsedLcuMastery {
                champion_id,
                level,
                points,
                last_play_time,
            })
        })
        .collect()
}

// ── Summoner name parsing ──────────────────────────────────────────────────

/// Extract (game_name, tag_line) from an LCU current-summoner response.
///
/// Priority:
/// 1. `gameName` + `tagLine` fields if both present
/// 2. Parse `displayName` as "Name#TAG"
/// 3. Use `displayName` as game_name with empty tag_line
pub fn parse_lcu_summoner_name(summoner: &serde_json::Value) -> (String, String) {
    let game_name = summoner["gameName"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();
    let tag_line = summoner["tagLine"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();

    if !game_name.is_empty() {
        return (game_name, tag_line);
    }

    // Fallback: parse displayName
    let display = summoner["displayName"]
        .as_str()
        .unwrap_or("Summoner")
        .trim()
        .to_string();

    if let Some(idx) = display.rfind('#') {
        let name = display[..idx].to_string();
        let tag = display[idx + 1..].to_string();
        if !name.is_empty() {
            return (name, tag);
        }
    }

    (display, String::new())
}

// ── Unit tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const MATCH_HISTORY_FIXTURE: &str = include_str!("../../tests/fixtures/lcu_match_history.json");
    const MASTERY_FIXTURE: &str = include_str!("../../tests/fixtures/lcu_mastery.json");

    const LOCAL_PUUID: &str = "test-puuid-local-player";

    fn match_history() -> serde_json::Value {
        serde_json::from_str(MATCH_HISTORY_FIXTURE).expect("fixture parse failed")
    }

    fn mastery() -> serde_json::Value {
        serde_json::from_str(MASTERY_FIXTURE).expect("fixture parse failed")
    }

    #[test]
    fn parse_lcu_match_history_finds_local_player() {
        let json = match_history();
        let matches = parse_lcu_match_history(&json, LOCAL_PUUID);
        assert_eq!(matches.len(), 2, "should find 2 games for local player");
        assert_eq!(matches[0].match_id, "LCU_9876543210");
        assert_eq!(matches[1].match_id, "LCU_9876543211");
    }

    #[test]
    fn parse_lcu_match_history_extracts_stats() {
        let json = match_history();
        let matches = parse_lcu_match_history(&json, LOCAL_PUUID);
        let first = &matches[0];
        assert_eq!(first.champion_id, 238); // Zed
        assert!(first.win);
        assert_eq!(first.kills, 8);
        assert_eq!(first.deaths, 2);
        assert_eq!(first.assists, 4);
        assert_eq!(first.duration_secs, 1823);
        assert_eq!(first.queue_id, 420);
    }

    #[test]
    fn parse_lcu_match_history_position_from_lane() {
        let json = match_history();
        let matches = parse_lcu_match_history(&json, LOCAL_PUUID);
        assert_eq!(matches[0].position.as_deref(), Some("MIDDLE"));
        assert_eq!(matches[1].position.as_deref(), Some("MIDDLE"));
    }

    #[test]
    fn parse_lcu_mastery_returns_all_entries() {
        let json = mastery();
        let entries = parse_lcu_mastery(&json);
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn parse_lcu_mastery_extracts_level_and_points() {
        let json = mastery();
        let entries = parse_lcu_mastery(&json);
        assert_eq!(entries[0].champion_id, 238);
        assert_eq!(entries[0].level, 7);
        assert_eq!(entries[0].points, 180_000);
        assert_eq!(entries[0].last_play_time, Some(1_700_000_000));
    }

    #[test]
    fn parse_lcu_summoner_name_gamename_tagline() {
        let v = serde_json::json!({ "gameName": "Berkilic", "tagLine": "TR1" });
        let (name, tag) = parse_lcu_summoner_name(&v);
        assert_eq!(name, "Berkilic");
        assert_eq!(tag, "TR1");
    }

    #[test]
    fn parse_lcu_summoner_name_display_name_with_hash() {
        let v = serde_json::json!({ "displayName": "Berkilic#TR1" });
        let (name, tag) = parse_lcu_summoner_name(&v);
        assert_eq!(name, "Berkilic");
        assert_eq!(tag, "TR1");
    }

    #[test]
    fn parse_lcu_summoner_name_display_name_no_hash() {
        let v = serde_json::json!({ "displayName": "JustAName" });
        let (name, tag) = parse_lcu_summoner_name(&v);
        assert_eq!(name, "JustAName");
        assert_eq!(tag, "");
    }

    #[test]
    fn parse_lcu_match_history_skips_other_player_games() {
        let json = match_history();
        // Player who appears only in game 1, not in game 2
        let matches = parse_lcu_match_history(&json, "other-player-puuid");
        assert_eq!(matches.len(), 1, "only one game for other player");
        assert_eq!(matches[0].match_id, "LCU_9876543210");
    }

    #[test]
    fn parse_lcu_match_history_returns_empty_for_unknown_puuid() {
        let json = match_history();
        let matches = parse_lcu_match_history(&json, "nonexistent-puuid");
        assert!(matches.is_empty());
    }
}
