//! Live Client Data API **pure parsers** + in-game plan assembly.
//!
//! Core port of the pure halves of `src-tauri/src/riot/live_client.rs` (parsers)
//! and `src-tauri/src/commands/overlay.rs` (`build_ingame_plan` + the champion /
//! opponent resolution of `get_ingame_plan`). The src-tauri copies are legacy and
//! die with the Tauri host; this module is the forward single source.
//!
//! The network fetch (`https://127.0.0.1:2999/liveclientdata/allgamedata`) stays
//! in the host — this module only transforms the `allgamedata` JSON. Policy-safe
//! by construction: it consumes ONLY the official API's public game time, the
//! neutral-objective takes already shown on the scoreboard, and YOUR OWN
//! champion + on-screen score. **No hidden info**, no fabrication: missing/odd
//! fields yield empty values, never a panic.

use serde::{Deserialize, Serialize};

use crate::draft_iq::archetype::ChampionArchetype;
use crate::draft_iq::narrative;
use crate::draft_iq::DraftKnowledgeBase;
use crate::macro_timers::{MacroTimerInput, ObjectiveEvent};
use crate::types::ChampionRecord;

/// Live Client Data API `EventName` → our objective token. The single source of
/// truth, reused by the parser + its test. `HordeKill` = Voidgrubs.
const EVENT_OBJECTIVE_MAP: [(&str, &str); 4] = [
    ("DragonKill", "dragon"),
    ("BaronKill", "baron"),
    ("HeraldKill", "herald"),
    ("HordeKill", "grubs"),
];

/// Map an `EventName` to an objective token, if it is a neutral objective we track.
fn objective_token(event_name: &str) -> Option<&'static str> {
    EVENT_OBJECTIVE_MAP
        .iter()
        .find(|(name, _)| *name == event_name)
        .map(|(_, token)| *token)
}

/// Pure: `allgamedata` JSON → `MacroTimerInput`. Defensive — missing/odd fields
/// yield `game_time_secs: 0` + empty events, never a panic. No fabrication.
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

/// The active (local) player's public, on-screen snapshot from the Live Client
/// Data API. Policy-safe: this is YOUR OWN champion + score, all visible on your
/// HUD — no hidden info.
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

/// Extract the locale-independent champion key from the Live Client
/// `rawChampionName` (e.g. "game_character_displayname_MissFortune" →
/// "MissFortune"). Robust against a localized `championName` (Turkish client
/// etc.). `None` when the prefix is absent.
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
/// presence) across client versions, so matching on the normalized game-name is
/// the robust common denominator — avoids a silent "no plan" when the fields
/// disagree.
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

/// Pure: the champion name of the enemy laner — the player on the OPPOSITE team
/// in the SAME role. Returns `None` when role/team is unknown or no such player
/// exists. This is public, on-screen info (loading screen + scoreboard), not
/// hidden data — used to make the in-game lane note matchup-aware.
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

/// Your in-game game plan — regenerated from the KB archetype of the champion you
/// are actually playing (read from the official Live Client Data API) plus your
/// live, on-screen score. JSON twin of the Tauri host's `IngamePlan` (which keeps
/// the ts-rs export until the Tauri host dies — no double export from core, so the
/// generated TS file has exactly one writer).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct IngamePlan {
    pub champion_name: String,
    /// KB/DDragon key for the champion icon (e.g. "MissFortune").
    pub champion_key: String,
    /// Normalized lane ("top"/"jungle"/"middle"/"bottom"/"utility" or "").
    pub position: String,
    pub win_condition: String,
    pub team_role: String,
    pub damage_profile: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spike_note: Option<String>,
    /// Matchup'a özel güç penceresi (iki power_curve karşılaştırması) — yalnız
    /// rakip laner biliniyorken ve belirgin faz farkı varken set edilir.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spike_window: Option<String>,
    /// Generic lane-phase micro — set only when the lane opponent is unknown
    /// (otherwise the richer `matchup_tips` carry the lane advice).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane_note: Option<String>,
    /// The enemy laner (opposite team, same role), when resolvable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opponent_name: Option<String>,
    /// KB/DDragon key for the enemy laner's icon, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opponent_key: Option<String>,
    /// Up to 4 matchup-specific lane tips vs the enemy laner. Empty when
    /// blind/unknown. Always serialized so the TS side can read `.length` safely.
    #[serde(default)]
    pub matchup_tips: Vec<String>,
    /// Mid-game macro advice (archetype-grounded).
    pub mid_plan: String,
    /// Late-game advice keyed by the champion's win condition.
    pub late_plan: String,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub cs: u32,
    pub level: u32,
    /// CS per minute, when game time is known (≥ 60s in).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cs_per_min: Option<f32>,
}

/// Pure: assemble the in-game plan from the parsed player snapshot + KB archetype.
/// When the enemy laner is known, the richer matchup tips carry the lane advice
/// (and the generic `lane_note` is dropped to avoid duplication).
fn build_ingame_plan(
    info: &ActivePlayerInfo,
    champion_key: &str,
    arch: &ChampionArchetype,
    position: &str,
    game_time_secs: u32,
    opponent: Option<(&ChampionArchetype, &str, &str)>, // (archetype, name, key)
) -> IngamePlan {
    let cs_per_min = if game_time_secs >= 60 {
        let per = info.cs as f32 / (game_time_secs as f32 / 60.0);
        Some((per * 10.0).round() / 10.0)
    } else {
        None
    };
    let (lane_note, matchup_tips) = match opponent {
        Some((opp, _, _)) => (None, narrative::build_matchup_tips(arch, opp, position)),
        None => (
            narrative::build_lane_phase_advice(arch, None, position),
            Vec::new(),
        ),
    };
    // Güç penceresi yalnız rakip arketipi elimizdeyken (blind/ARAM'da None).
    let spike_window =
        opponent.and_then(|(opp, _, _)| narrative::build_matchup_spike_window(arch, opp));
    IngamePlan {
        champion_name: info.champion_name.clone(),
        champion_key: champion_key.to_string(),
        position: position.to_string(),
        win_condition: narrative::build_win_condition_text(&arch.win_condition),
        team_role: narrative::build_team_role_text(arch),
        damage_profile: narrative::build_damage_profile_label(&arch.damage_profile),
        spike_note: narrative::build_spike_note(arch),
        spike_window,
        lane_note,
        opponent_name: opponent.map(|(_, name, _)| name.to_string()),
        opponent_key: opponent.map(|(_, _, key)| key.to_string()),
        matchup_tips,
        mid_plan: narrative::build_mid_game_advice(arch),
        late_plan: narrative::build_late_game_advice(&arch.win_condition),
        kills: info.kills,
        deaths: info.deaths,
        assists: info.assists,
        cs: info.cs,
        level: info.level,
        cs_per_min,
    }
}

/// Pure: the whole `get_ingame_plan` decision path minus I/O — raw `allgamedata`
/// + the host's champion table → `IngamePlan`. `None` when there is no active
/// player in the payload or the played champion isn't in the KB (quiet, expected).
pub fn compute_ingame_plan(
    raw: &serde_json::Value,
    all_champions: &[ChampionRecord],
    kb: &DraftKnowledgeBase,
) -> Option<IngamePlan> {
    let info = parse_active_player(raw)?;
    let game_time_secs = parse_macro_input(raw).game_time_secs;

    // Prefer the locale-independent key from rawChampionName (robust against a
    // localized championName), fall back to a case-insensitive display-name match.
    let key = info
        .champion_key
        .as_deref()
        .and_then(|k| all_champions.iter().find(|c| c.key.eq_ignore_ascii_case(k)))
        .or_else(|| {
            all_champions
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(&info.champion_name))
        })
        .map(|c| c.key.clone())?;

    let arch = kb.get_archetype(&key)?;
    let position = info.position.to_lowercase();

    // Resolve the enemy laner (public scoreboard info) → key → archetype for a
    // matchup-aware lane read. None for ARAM / unknown role → generic lane note.
    let opp = enemy_laner_name(raw, &info.team, &info.position).and_then(|name| {
        all_champions
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(&name))
            .map(|c| (name, c.key.clone()))
    });
    let opponent = opp
        .as_ref()
        .and_then(|(name, k)| kb.get_archetype(k).map(|a| (a, name.as_str(), k.as_str())));

    Some(build_ingame_plan(
        &info,
        &key,
        arch,
        &position,
        game_time_secs,
        opponent,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macro_timers::compute_macro_state;

    const ALLGAMEDATA: &str = include_str!("../tests/fixtures/live_client_allgamedata.json");

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

    #[test]
    fn ingame_plan_resolves_champion_opponent_and_pace_from_raw_payload() {
        // Garen vs Darius top — both KB champions, 10:00 in, 70 CS.
        let raw = serde_json::json!({
            "activePlayer": { "riotIdGameName": "Me" },
            "allPlayers": [
                { "riotIdGameName": "Me", "championName": "Garen",
                  "rawChampionName": "game_character_displayname_Garen",
                  "position": "TOP", "team": "ORDER", "level": 9,
                  "scores": { "kills": 2, "deaths": 1, "assists": 1, "creepScore": 70 } },
                { "riotIdGameName": "Foe", "championName": "Darius",
                  "position": "TOP", "team": "CHAOS" }
            ],
            "gameData": { "gameTime": 600.0 }
        });
        let champions = vec![
            ChampionRecord {
                champion_id: 86,
                key: "Garen".into(),
                name: "Garen".into(),
                title: "t".into(),
            },
            ChampionRecord {
                champion_id: 122,
                key: "Darius".into(),
                name: "Darius".into(),
                title: "t".into(),
            },
        ];
        let kb = DraftKnowledgeBase::load().expect("KB loads");

        let plan = compute_ingame_plan(&raw, &champions, &kb).expect("plan built");
        assert_eq!(plan.champion_key, "Garen");
        assert_eq!(plan.position, "top");
        assert_eq!(plan.opponent_key.as_deref(), Some("Darius"));
        assert!(!plan.matchup_tips.is_empty(), "known opponent → matchup tips");
        assert!(plan.lane_note.is_none(), "lane advice folded into matchup tips");
        assert_eq!(plan.cs_per_min, Some(7.0)); // 70 CS / 10 min
        assert!(!plan.win_condition.is_empty());

        // Unknown champion (not in the table) → quiet None.
        let none = compute_ingame_plan(&raw, &[], &kb);
        assert!(none.is_none());
    }
}
