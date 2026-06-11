//! Riot **Live Client Data API** client + pure parser (Faz 4 overlay runtime).
//!
//! The only network layer for the in-game overlay. Reads the official local API at
//! `https://127.0.0.1:2999/liveclientdata/allgamedata` (self-signed cert → rustls, like
//! the LCU client) and maps it to `macro_timers::MacroTimerInput`.
//!
//! Policy-safe by construction: it consumes ONLY the official API's public game time +
//! the neutral-objective takes already shown on the scoreboard. **No hidden info**
//! (enemy cooldowns / wards / summoners), no process injection, no game-memory read.
//! `parse_macro_input` is pure + defensive (missing fields → empty, never panic) so the
//! overlay is fully unit-testable without a live game.

use crate::recommendation::macro_timers::{MacroTimerInput, ObjectiveEvent};
use anyhow::Result;
use reqwest::{Client, ClientBuilder};
use std::time::Duration;

const BASE_URL: &str = "https://127.0.0.1:2999";

/// Live Client Data API `EventName` → our objective token. The single source of truth,
/// reused by the parser + its test. `HordeKill` = Voidgrubs.
const EVENT_OBJECTIVE_MAP: [(&str, &str); 4] = [
    ("DragonKill", "dragon"),
    ("BaronKill", "baron"),
    ("HeraldKill", "herald"),
    ("HordeKill", "grubs"),
];

pub struct LiveClientApi {
    http: Client,
}

impl LiveClientApi {
    pub fn new() -> Result<Self> {
        let http = ClientBuilder::new()
            .danger_accept_invalid_certs(true) // Live Client Data self-signed cert
            .timeout(Duration::from_secs(2))
            .build()?;
        Ok(Self { http })
    }

    /// GET `/liveclientdata/allgamedata`. Any error (no game ⇒ connection refused) is the
    /// caller's "no live game" signal — do not log as an error.
    pub async fn fetch_all_game_data(&self) -> Result<serde_json::Value> {
        let url = format!("{BASE_URL}/liveclientdata/allgamedata");
        let value = self
            .http
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        Ok(value)
    }
}

/// Map an `EventName` to an objective token, if it is a neutral objective we track.
fn objective_token(event_name: &str) -> Option<&'static str> {
    EVENT_OBJECTIVE_MAP
        .iter()
        .find(|(name, _)| *name == event_name)
        .map(|(_, token)| *token)
}

/// Pure: `allgamedata` JSON → `MacroTimerInput`. Defensive — missing/odd fields yield
/// `game_time_secs: 0` + empty events, never a panic. No fabrication.
pub fn parse_macro_input(raw: &serde_json::Value) -> MacroTimerInput {
    let game_time_secs = raw
        .get("gameData")
        .and_then(|g| g.get("gameTime"))
        .and_then(serde_json::Value::as_f64)
        .map(|t| t.max(0.0).floor() as u32)
        .unwrap_or(0);

    let events = raw
        .get("events")
        .and_then(|e| e.get("Events"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    let name = e.get("EventName").and_then(serde_json::Value::as_str)?;
                    let objective = objective_token(name)?;
                    let killed_at_secs = e
                        .get("EventTime")
                        .and_then(serde_json::Value::as_f64)
                        .map(|t| t.max(0.0).floor() as u32)
                        .unwrap_or(0);
                    Some(ObjectiveEvent {
                        objective: objective.to_string(),
                        killed_at_secs,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    MacroTimerInput {
        game_time_secs,
        events,
    }
}

/// The active (local) player's public, on-screen snapshot from the Live Client Data
/// API. Policy-safe: this is YOUR OWN champion + score, all visible on your HUD — no
/// hidden info. Used to regenerate your champ-select game plan from the KB in-game so
/// you can alt-tab and review it during the match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivePlayerInfo {
    pub champion_name: String,
    /// Locale-independent champion key from `rawChampionName` (e.g. "MissFortune"),
    /// when present — preferred over the (localizable) `championName` for KB lookup.
    pub champion_key: Option<String>,
    /// Raw API position ("TOP"/"JUNGLE"/"MIDDLE"/"BOTTOM"/"UTILITY" or "").
    pub position: String,
    /// Raw API team ("ORDER"/"CHAOS" or ""). Used to resolve the enemy laner.
    pub team: String,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub cs: u32,
    pub level: u32,
}

/// Extract the locale-independent champion key from the Live Client `rawChampionName`
/// (e.g. "game_character_displayname_MissFortune" → "MissFortune"). Robust against a
/// localized `championName` (Turkish client etc.). `None` when the prefix is absent.
fn key_from_raw_name(raw_name: &str) -> Option<String> {
    raw_name
        .strip_prefix("game_character_displayname_")
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn u32_field(scores: Option<&serde_json::Value>, key: &str) -> u32 {
    scores
        .and_then(|s| s.get(key))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as u32
}

/// Game-name part of a riot id / summoner name, lowercased ("Name#TAG" → "name").
/// Live Client mixes `riotIdGameName` / `summonerName` / `riotId` (and "#TAG"
/// presence) across client versions, so matching on the normalized game-name is the
/// robust common denominator — avoids a silent "no plan" when the fields disagree.
fn norm_ident(s: &str) -> String {
    s.split('#').next().unwrap_or(s).trim().to_lowercase()
}

/// All non-empty normalized identity candidates for a player / activePlayer object.
fn player_idents(p: &serde_json::Value) -> Vec<String> {
    ["riotIdGameName", "summonerName", "riotId"]
        .iter()
        .filter_map(|k| p.get(*k).and_then(serde_json::Value::as_str))
        .map(norm_ident)
        .filter(|s| !s.is_empty())
        .collect()
}

/// Pure: locate the active player inside `allPlayers` by matching any of their
/// normalized identity fields, and extract their champion + score. Returns `None`
/// when there is no `allPlayers` array or no match (spectator / pre-load) — never
/// fabricates.
pub fn parse_active_player(raw: &serde_json::Value) -> Option<ActivePlayerInfo> {
    let active = raw.get("activePlayer")?;
    let active_idents = player_idents(active);
    if active_idents.is_empty() {
        return None;
    }

    let players = raw
        .get("allPlayers")
        .and_then(serde_json::Value::as_array)?;
    let me = players.iter().find(|p| {
        player_idents(p)
            .iter()
            .any(|cand| active_idents.iter().any(|a| a == cand))
    })?;

    let champion_name = me
        .get("championName")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())?
        .to_string();
    let champion_key = me
        .get("rawChampionName")
        .and_then(serde_json::Value::as_str)
        .and_then(key_from_raw_name);
    let position = me
        .get("position")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let team = me
        .get("team")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let level = me
        .get("level")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as u32;
    let scores = me.get("scores");

    Some(ActivePlayerInfo {
        champion_name,
        champion_key,
        position,
        team,
        kills: u32_field(scores, "kills"),
        deaths: u32_field(scores, "deaths"),
        assists: u32_field(scores, "assists"),
        cs: u32_field(scores, "creepScore"),
        level,
    })
}

/// Pure: the champion name of the enemy laner — the player on the OPPOSITE team in the
/// SAME role. Returns `None` when role/team is unknown or no such player exists. This is
/// public, on-screen info (loading screen + scoreboard), not hidden data — used to make
/// the in-game lane note matchup-aware.
pub fn enemy_laner_name(
    raw: &serde_json::Value,
    my_team: &str,
    my_position: &str,
) -> Option<String> {
    if my_team.is_empty() || my_position.is_empty() {
        return None;
    }
    let players = raw
        .get("allPlayers")
        .and_then(serde_json::Value::as_array)?;
    players.iter().find_map(|p| {
        let team = p.get("team").and_then(serde_json::Value::as_str)?;
        let pos = p.get("position").and_then(serde_json::Value::as_str)?;
        if !team.eq_ignore_ascii_case(my_team) && pos.eq_ignore_ascii_case(my_position) {
            p.get("championName")
                .and_then(serde_json::Value::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recommendation::macro_timers::compute_macro_state;

    const ALLGAMEDATA: &str = include_str!("../../tests/fixtures/live_client_allgamedata.json");

    #[test]
    fn parses_game_time_and_objective_events_from_fixture() {
        let raw: serde_json::Value = serde_json::from_str(ALLGAMEDATA).unwrap();
        let input = parse_macro_input(&raw);

        // gameTime 1503.42 → floored.
        assert_eq!(input.game_time_secs, 1503);

        // Only the 4 neutral-objective events map; ChampionKill/Turret/GameStart filtered.
        let tokens: Vec<&str> = input.events.iter().map(|e| e.objective.as_str()).collect();
        assert_eq!(tokens, vec!["grubs", "dragon", "herald", "baron"]);

        let dragon = input
            .events
            .iter()
            .find(|e| e.objective == "dragon")
            .unwrap();
        assert_eq!(dragon.killed_at_secs, 385, "EventTime floored");
    }

    #[test]
    fn parser_feeds_engine_dragon_respawn() {
        // End-to-end: parsed fixture → engine. Dragon taken at 385 → next at 385+300=685.
        let raw: serde_json::Value = serde_json::from_str(ALLGAMEDATA).unwrap();
        let state = compute_macro_state(&parse_macro_input(&raw));
        let dragon = state
            .objectives
            .iter()
            .find(|o| o.objective == "dragon")
            .unwrap();
        assert_eq!(dragon.next_spawn_secs, 685);
    }

    #[test]
    fn missing_or_malformed_fields_never_panic() {
        for raw in [
            serde_json::json!({}),
            serde_json::json!({ "gameData": {} }),
            serde_json::json!({ "events": { "Events": "nope" } }),
            serde_json::json!({ "gameData": { "gameTime": -5.0 }, "events": { "Events": [] } }),
        ] {
            let input = parse_macro_input(&raw);
            assert_eq!(input.game_time_secs, 0);
            assert!(input.events.is_empty());
        }
    }

    #[test]
    fn active_player_matched_by_riot_id_with_scores() {
        let raw = serde_json::json!({
            "activePlayer": { "riotIdGameName": "Faker", "summonerName": "" },
            "allPlayers": [
                { "riotIdGameName": "Faker", "championName": "Ahri", "position": "MIDDLE", "team": "ORDER",
                  "level": 11, "scores": { "kills": 5, "deaths": 2, "assists": 7, "creepScore": 142 } },
                { "riotIdGameName": "Other", "championName": "Zed", "position": "MIDDLE", "team": "CHAOS" }
            ]
        });
        let info = parse_active_player(&raw).expect("active player found");
        assert_eq!(info.champion_name, "Ahri");
        assert_eq!(info.position, "MIDDLE");
        assert_eq!(info.team, "ORDER");
        assert_eq!((info.kills, info.deaths, info.assists), (5, 2, 7));
        assert_eq!(info.cs, 142);
        assert_eq!(info.level, 11);
    }

    #[test]
    fn enemy_laner_resolved_by_opposite_team_same_role() {
        let raw = serde_json::json!({
            "allPlayers": [
                { "championName": "Ahri", "position": "MIDDLE", "team": "ORDER" },
                { "championName": "Zed", "position": "MIDDLE", "team": "CHAOS" },
                { "championName": "Lulu", "position": "UTILITY", "team": "CHAOS" }
            ]
        });
        assert_eq!(
            enemy_laner_name(&raw, "ORDER", "MIDDLE").as_deref(),
            Some("Zed")
        );
        // No opponent in that role on the other team → None.
        assert_eq!(enemy_laner_name(&raw, "ORDER", "TOP"), None);
        // Unknown team/role → None (blind/ARAM safety).
        assert_eq!(enemy_laner_name(&raw, "", "MIDDLE"), None);
        assert_eq!(enemy_laner_name(&raw, "ORDER", ""), None);
    }

    #[test]
    fn active_player_champion_key_from_raw_name_survives_localization() {
        assert_eq!(
            key_from_raw_name("game_character_displayname_MissFortune").as_deref(),
            Some("MissFortune")
        );
        assert_eq!(key_from_raw_name("MissFortune"), None); // no prefix → fall back to name
                                                            // Even if championName is localized, the raw key is locale-independent.
        let raw = serde_json::json!({
            "activePlayer": { "riotIdGameName": "Me" },
            "allPlayers": [
                { "riotIdGameName": "Me", "championName": "Bayan Talih",
                  "rawChampionName": "game_character_displayname_MissFortune",
                  "position": "BOTTOM", "team": "ORDER" }
            ]
        });
        let info = parse_active_player(&raw).expect("found");
        assert_eq!(info.champion_key.as_deref(), Some("MissFortune"));
        assert_eq!(info.champion_name, "Bayan Talih"); // localized display kept for UI
    }

    #[test]
    fn active_player_matches_across_mixed_identity_fields_and_tags() {
        // Real-world drift: activePlayer carries the full riot id incl "#TAG" under
        // summonerName, while allPlayers exposes only the tag-less riotIdGameName. The
        // normalized game-name match must still find the player (no silent "no plan").
        let raw = serde_json::json!({
            "activePlayer": { "summonerName": "Hide on bush#KR1", "riotIdGameName": "" },
            "allPlayers": [
                { "riotIdGameName": "Hide on bush", "summonerName": "", "championName": "Ahri",
                  "position": "MIDDLE", "team": "ORDER" }
            ]
        });
        let info = parse_active_player(&raw).expect("matched on normalized game-name");
        assert_eq!(info.champion_name, "Ahri");
        assert_eq!(info.team, "ORDER");
    }

    #[test]
    fn active_player_falls_back_to_summoner_name_and_zeroes_missing_scores() {
        let raw = serde_json::json!({
            "activePlayer": { "summonerName": "You" },
            "allPlayers": [ { "summonerName": "You", "championName": "Lux", "position": "" } ]
        });
        let info = parse_active_player(&raw).expect("matched by summoner name");
        assert_eq!(info.champion_name, "Lux");
        assert_eq!(info.position, "");
        assert_eq!(
            (info.kills, info.deaths, info.assists, info.cs),
            (0, 0, 0, 0)
        );
    }

    #[test]
    fn active_player_none_without_allplayers_or_match() {
        // Minimal payload (no allPlayers) → None, not a panic.
        assert!(parse_active_player(
            &serde_json::json!({ "activePlayer": { "summonerName": "You" } })
        )
        .is_none());
        // allPlayers present but no identity match → None.
        let no_match = serde_json::json!({
            "activePlayer": { "summonerName": "You" },
            "allPlayers": [ { "summonerName": "Enemy", "championName": "Zed" } ]
        });
        assert!(parse_active_player(&no_match).is_none());
    }

    #[test]
    fn unknown_events_are_ignored_not_fabricated() {
        let raw = serde_json::json!({
            "gameData": { "gameTime": 100.9 },
            "events": { "Events": [
                { "EventName": "ChampionKill", "EventTime": 50.0 },
                { "EventName": "TurretKilled", "EventTime": 60.0 },
            ] }
        });
        let input = parse_macro_input(&raw);
        assert_eq!(input.game_time_secs, 100);
        assert!(input.events.is_empty(), "no neutral objectives → no events");
    }
}
