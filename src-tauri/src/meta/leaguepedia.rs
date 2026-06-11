//! Leaguepedia (Fandom MediaWiki **Cargo** API) pro-play source — closes the
//! pro-data gap with the same public data most LoL pro-stat tools use.
//!
//! **Confirmed reachable** (a polite single request returns real pro games), but
//! the API rate-limits bursts hard, so fetching is deliberately *patient*:
//! descriptive UA + `maxlag` + long spacing + backoff + a 24 h cache. Pro data
//! accumulates over time, exactly like the Match-V5 ranked pipeline.
//!
//! `PicksAndBansS7` carries every pro game's draft — `Team{1,2}Pick{1..5}` +
//! `Team{1,2}Ban{1..5}`. We aggregate **champion-level pro presence**:
//!   - `pick_rate = games_picked / games`
//!   - `ban_rate  = games_banned / games`
//!   - `presence  = pick_rate + ban_rate`
//!
//! MediaWiki returns rows as `cargoquery[].title` with **space-separated** field
//! keys. Champion display names are mapped to ids via a caller-built map
//! (`normalize_name(champions.name) → id`); unmapped names are skipped (no fab).
#![allow(dead_code)]

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

const CARGO_URL: &str = "https://lol.fandom.com/api.php";
const UA: &str = "champ-select-assistant/0.9 (personal LoL draft tool)";
const HTTP_TIMEOUT_SECS: u64 = 20;
/// Be a good MediaWiki citizen — small page, server-lag aware.
const PAGE_LIMIT: u32 = 200;

const PICK_KEYS: [&str; 10] = [
    "Team1Pick1",
    "Team1Pick2",
    "Team1Pick3",
    "Team1Pick4",
    "Team1Pick5",
    "Team2Pick1",
    "Team2Pick2",
    "Team2Pick3",
    "Team2Pick4",
    "Team2Pick5",
];
const BAN_KEYS: [&str; 10] = [
    "Team1Ban1",
    "Team1Ban2",
    "Team1Ban3",
    "Team1Ban4",
    "Team1Ban5",
    "Team2Ban1",
    "Team2Ban2",
    "Team2Ban3",
    "Team2Ban4",
    "Team2Ban5",
];

/// One champion's aggregated pro presence (champion-level, not role-specific —
/// a champion is contested globally in pro play).
#[derive(Debug, Clone, PartialEq)]
pub struct ProPresence {
    pub champion_id: u32,
    pub pick_games: u32,
    pub ban_games: u32,
    pub total_games: u32,
    pub pick_rate: f32,
    pub ban_rate: f32,
    pub presence: f32,
}

/// Normalize a champion display name to a match key: lowercase, alphanumerics only.
/// Both sides (Leaguepedia name + `champions.name`) go through this, so
/// "Lee Sin"/"Nunu & Willump"/"Kai'Sa" line up without per-champion special cases.
pub fn normalize_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Pure parse of a `PicksAndBansS7` cargoquery response into per-game
/// `(pick_names, ban_names)`. Empty slots and missing fields are dropped.
pub fn parse_draft_rows(v: &Value) -> Vec<(Vec<String>, Vec<String>)> {
    let Some(rows) = v.get("cargoquery").and_then(Value::as_array) else {
        return Vec::new();
    };
    rows.iter()
        .filter_map(|row| row.get("title"))
        .map(|t| {
            let collect = |keys: &[&str]| -> Vec<String> {
                keys.iter()
                    .filter_map(|k| t.get(*k).and_then(Value::as_str))
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect()
            };
            (collect(&PICK_KEYS), collect(&BAN_KEYS))
        })
        .collect()
}

/// Pure aggregation: per-game picks/bans → champion-level pro presence. Names are
/// resolved via `name_to_id` (keyed by [`normalize_name`]); unmapped → skipped.
/// A champion counts once per game even if (somehow) listed twice. Deterministic.
pub fn aggregate_presence(
    games: &[(Vec<String>, Vec<String>)],
    name_to_id: &HashMap<String, u32>,
) -> Vec<ProPresence> {
    let total = games.len() as u32;
    if total == 0 {
        return Vec::new();
    }
    let resolve = |name: &str| name_to_id.get(&normalize_name(name)).copied();

    let mut pick: HashMap<u32, u32> = HashMap::new();
    let mut ban: HashMap<u32, u32> = HashMap::new();
    for (picks, bans) in games {
        let mut seen: HashSet<u32> = HashSet::new();
        for id in picks.iter().filter_map(|n| resolve(n)) {
            if seen.insert(id) {
                *pick.entry(id).or_insert(0) += 1;
            }
        }
        seen.clear();
        for id in bans.iter().filter_map(|n| resolve(n)) {
            if seen.insert(id) {
                *ban.entry(id).or_insert(0) += 1;
            }
        }
    }

    let mut ids: Vec<u32> = pick.keys().chain(ban.keys()).copied().collect();
    ids.sort_unstable();
    ids.dedup();
    ids.into_iter()
        .map(|champion_id| {
            let pick_games = pick.get(&champion_id).copied().unwrap_or(0);
            let ban_games = ban.get(&champion_id).copied().unwrap_or(0);
            let pick_rate = pick_games as f32 / total as f32;
            let ban_rate = ban_games as f32 / total as f32;
            ProPresence {
                champion_id,
                pick_games,
                ban_games,
                total_games: total,
                pick_rate,
                ban_rate,
                presence: pick_rate + ban_rate,
            }
        })
        .collect()
}

fn build_client() -> Result<Client> {
    Ok(Client::builder()
        .use_rustls_tls()
        .user_agent(UA)
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()?)
}

/// Polite Cargo fetch of recent pro drafts (builds its own patient client).
/// `maxlag` + descriptive UA are required MediaWiki etiquette; a `ratelimited`
/// response surfaces as `Err` so the caller keeps the last-good cache and retries
/// on the next (slow, 24 h) tick. One page (200 drafts) per call is a meaningful
/// recent-pro snapshot and stays comfortably within the rate limit.
pub async fn fetch_draft_rows(offset: u32) -> Result<Value> {
    let client = build_client()?;
    let fields = PICK_KEYS
        .iter()
        .chain(BAN_KEYS.iter())
        .copied()
        .collect::<Vec<_>>()
        .join(",");
    let resp = client
        .get(CARGO_URL)
        .query(&[
            ("action", "cargoquery"),
            ("format", "json"),
            ("maxlag", "5"),
            ("tables", "PicksAndBansS7"),
            ("fields", &fields),
            ("order_by", "DateTime_UTC DESC"),
            ("limit", &PAGE_LIMIT.to_string()),
            ("offset", &offset.to_string()),
        ])
        .send()
        .await?
        .error_for_status()?;
    let v: Value = resp.json().await?;
    if let Some(err) = v
        .get("error")
        .and_then(|e| e.get("code"))
        .and_then(Value::as_str)
    {
        return Err(anyhow!("Leaguepedia API error: {err}"));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real cargoquery wrapper (confirmed): rows under `cargoquery[].title` with
    // space-keyed fields. PicksAndBansS7 field names per Leaguepedia's schema.
    const DRAFT_FIXTURE: &str = r#"{
      "cargoquery": [
        { "title": {
            "Team1Pick1": "Aurelion Sol", "Team1Pick2": "Lee Sin", "Team1Pick3": "Karma",
            "Team1Pick4": "Jinx", "Team1Pick5": "Nautilus",
            "Team2Pick1": "Ahri", "Team2Pick2": "Wukong", "Team2Pick3": "Renata Glasc",
            "Team2Pick4": "Aphelios", "Team2Pick5": "Maokai",
            "Team1Ban1": "Karma", "Team1Ban2": "Kai'Sa", "Team1Ban3": "",
            "Team2Ban1": "Lee Sin", "Team2Ban2": "Vi",
            "DateTime UTC": "2026-06-07 21:13:00"
        } },
        { "title": {
            "Team1Pick1": "Karma", "Team1Pick2": "Sejuani",
            "Team2Pick1": "Lee Sin", "Team2Pick2": "Orianna",
            "Team1Ban1": "Karma", "Team2Ban1": "Karma",
            "DateTime UTC": "2026-06-07 22:00:00"
        } }
      ]
    }"#;

    fn name_map() -> HashMap<String, u32> {
        // (normalized champions.name → id) the command layer builds from the DB.
        [
            ("Karma", 43u32),
            ("Lee Sin", 64),
            ("Aurelion Sol", 136),
            ("Wukong", 62),
            ("Kai'Sa", 145),
        ]
        .into_iter()
        .map(|(n, id)| (normalize_name(n), id))
        .collect()
    }

    #[test]
    fn normalize_handles_spaces_apostrophes_ampersands() {
        assert_eq!(normalize_name("Lee Sin"), "leesin");
        assert_eq!(normalize_name("Kai'Sa"), "kaisa");
        assert_eq!(normalize_name("Nunu & Willump"), "nunuwillump");
        assert_eq!(normalize_name("Aurelion Sol"), "aurelionsol");
    }

    #[test]
    fn parse_draft_rows_extracts_picks_and_bans_dropping_empties() {
        let v: Value = serde_json::from_str(DRAFT_FIXTURE).unwrap();
        let rows = parse_draft_rows(&v);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0.len(), 10, "10 picks in game 1");
        // Game 1 bans: Karma, Kai'Sa, Lee Sin, Vi (empty Team1Ban3 dropped).
        assert_eq!(rows[0].1, vec!["Karma", "Kai'Sa", "Lee Sin", "Vi"]);
    }

    #[test]
    fn aggregate_presence_computes_pick_ban_and_presence() {
        let v: Value = serde_json::from_str(DRAFT_FIXTURE).unwrap();
        let rows = parse_draft_rows(&v);
        let out = aggregate_presence(&rows, &name_map());

        let karma = out.iter().find(|p| p.champion_id == 43).unwrap();
        // Karma picked in both games (g1 Team1Pick3, g2 Team1Pick1) → 2/2.
        assert_eq!(karma.pick_games, 2);
        // Banned in g1 (Team1Ban1) + g2 (both teams, counted once per game) → 2/2.
        assert_eq!(karma.ban_games, 2);
        assert!((karma.pick_rate - 1.0).abs() < 1e-6);
        assert!((karma.ban_rate - 1.0).abs() < 1e-6);
        assert!((karma.presence - 2.0).abs() < 1e-6);

        // Lee Sin: picked g1+g2 (2), banned g1 only (1) → presence 1.0 + 0.5.
        let lee = out.iter().find(|p| p.champion_id == 64).unwrap();
        assert_eq!(lee.pick_games, 2);
        assert_eq!(lee.ban_games, 1);

        // Unmapped names (Jinx, Ahri, …) are skipped — no fabricated rows.
        assert!(out
            .iter()
            .all(|p| [43, 64, 136, 62, 145].contains(&p.champion_id)));
    }

    #[test]
    fn empty_input_is_safe() {
        assert!(aggregate_presence(&[], &name_map()).is_empty());
        let v: Value = serde_json::from_str(r#"{"cargoquery":[]}"#).unwrap();
        assert!(parse_draft_rows(&v).is_empty());
    }
}
