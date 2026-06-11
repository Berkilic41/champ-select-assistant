//! Match-V5 Aggregation Core v1 (Sprint, Claude) — engine-pure.
//!
//! Turns a batch of simplified Riot Match-V5 matches into aggregated champion
//! rates, lane matchups, and builds with sample-size confidence. Fully pure: no
//! network/Riot-key/DB/UI. Codex later wires the real fetcher → this aggregator →
//! DB upsert. No fabrication: with nothing to aggregate the output is empty + a
//! warning. `source` is always `"riot_match_v5"`; `patch` is keyed per row so
//! cross-patch batches don't mix.

#![allow(dead_code)] // consumed by the ingestion fetcher/DB-upsert wiring (Codex, later)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

const HIGH_SAMPLE: u32 = 100;
const MEDIUM_SAMPLE: u32 = 30;
const SOURCE: &str = "riot_match_v5";
const ARAM_QUEUE: u32 = 450;
const ARENA_QUEUE: u32 = 1700;
/// Trinket / ward item ids filtered out of builds.
const TRINKETS: [u32; 4] = [3340, 3363, 3364, 3330];
const CORE_ITEMS: usize = 3;
const SITUATIONAL_ITEMS: usize = 3;

// ── Input (simplified Match-V5) ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MatchV5Participant {
    pub participant_id: u32,
    pub champion_id: u32,
    pub team_id: u32, // 100 | 200
    pub team_position: String,
    pub win: bool,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    /// item0..item6 (0 = empty; trinkets filtered).
    pub items: Vec<u32>,
    pub runes: Vec<u32>,
    pub summoner_spells: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct MatchV5 {
    pub match_id: String,
    pub queue_id: u32,
    /// Normalized patch, e.g. "14.11" (caller derives from game_version).
    pub patch: String,
    pub participants: Vec<MatchV5Participant>,
    /// Champion ids banned this game (both teams). `-1`/`0` (no-ban) filtered by
    /// the mapper. Drives real `ban_rate` (= ban presence) from our own games.
    pub bans: Vec<u32>,
}

// ── Output DTOs (ts-rs) ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct AggregatedChampionRate {
    pub champion_id: u32,
    pub position: String,
    pub games: u32,
    pub wins: u32,
    pub win_rate: f32,
    pub pick_rate: f32,
    /// Champion-wide ban presence (banned-in / total-matches), same across the
    /// champion's positions. 0 when no ban data. Pick + ban = "meta heat".
    pub ban_rate: f32,
    pub sample_size: u32,
    pub patch: String,
    pub source: String,
    pub confidence: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct AggregatedMatchup {
    pub champion_id: u32,
    pub opponent_id: u32,
    pub position: String,
    pub games: u32,
    pub wins: u32,
    pub win_rate: f32,
    pub patch: String,
    pub source: String,
    pub confidence: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct AggregatedBuild {
    pub champion_id: u32,
    pub position: String,
    pub patch: String,
    pub core_items: Vec<u32>,
    pub situational_items: Vec<u32>,
    pub rune_ids: Vec<u32>,
    pub summoner_spells: Vec<u32>,
    pub games: u32,
    pub win_rate: f32,
    pub source: String,
    pub confidence: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct AggregationQuality {
    pub match_count: u32,
    pub champion_rate_count: u32,
    pub matchup_count: u32,
    pub build_count: u32,
    pub skipped_matches: u32,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct AggregationResult {
    pub rates: Vec<AggregatedChampionRate>,
    pub matchups: Vec<AggregatedMatchup>,
    pub builds: Vec<AggregatedBuild>,
    pub quality: AggregationQuality,
}

// ── Helpers ─────────────────────────────────────────────────────────────────────

fn confidence_for(sample: u32) -> &'static str {
    if sample >= HIGH_SAMPLE {
        "high"
    } else if sample >= MEDIUM_SAMPLE {
        "medium"
    } else {
        "low"
    }
}

/// Normalize a Riot `team_position` to the lowercase LCU role used by the DB.
/// Empty / unknown (e.g. ARAM) → None (participant skipped).
fn normalize_position(pos: &str) -> Option<String> {
    let role = match pos.to_uppercase().as_str() {
        "TOP" => "top",
        "JUNGLE" => "jungle",
        "MIDDLE" | "MID" => "middle",
        "BOTTOM" | "BOT" => "bottom",
        "UTILITY" | "SUPPORT" => "utility",
        _ => return None,
    };
    Some(role.to_string())
}

fn is_real_item(id: u32) -> bool {
    id != 0 && !TRINKETS.contains(&id)
}

/// Ids sorted by frequency desc, then id asc (deterministic).
fn sorted_by_freq(freq: &HashMap<u32, u32>) -> Vec<u32> {
    let mut v: Vec<(u32, u32)> = freq.iter().map(|(&id, &c)| (id, c)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.into_iter().map(|(id, _)| id).collect()
}

#[derive(Default)]
struct RateAcc {
    games: u32,
    wins: u32,
}

#[derive(Default)]
struct BuildAcc {
    games: u32,
    wins: u32,
    item_freq: HashMap<u32, u32>,
    rune_freq: HashMap<u32, u32>,
    spell_freq: HashMap<(u32, u32), u32>,
}

/// Aggregate a batch of matches. Per-patch keyed so cross-patch batches stay split.
pub fn aggregate_matches(matches: &[MatchV5]) -> AggregationResult {
    let mut rates: HashMap<(u32, String, String), RateAcc> = HashMap::new();
    let mut role_totals: HashMap<(String, String), u32> = HashMap::new();
    let mut matchups: HashMap<(u32, u32, String, String), RateAcc> = HashMap::new();
    let mut builds: HashMap<(u32, String, String), BuildAcc> = HashMap::new();
    // Ban presence: (champion, patch) → games banned, and per-patch match totals.
    let mut ban_counts: HashMap<(u32, String), u32> = HashMap::new();
    let mut patch_matches: HashMap<String, u32> = HashMap::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut skipped = 0u32;
    let mut match_count = 0u32;
    let warn = |w: &str, warnings: &mut Vec<String>| {
        if !warnings.iter().any(|x| x == w) {
            warnings.push(w.to_string());
        }
    };

    for m in matches {
        if m.queue_id == ARAM_QUEUE {
            skipped += 1;
            warn("aram_skipped", &mut warnings);
            continue;
        }
        if m.queue_id == ARENA_QUEUE {
            skipped += 1;
            warn("arena_skipped", &mut warnings);
            continue;
        }
        let valid: Vec<(&MatchV5Participant, String)> = m
            .participants
            .iter()
            .filter(|p| p.champion_id != 0)
            .filter_map(|p| normalize_position(&p.team_position).map(|pos| (p, pos)))
            .collect();
        if valid.is_empty() {
            skipped += 1;
            warn("no_positions", &mut warnings);
            continue;
        }
        match_count += 1;
        *patch_matches.entry(m.patch.clone()).or_insert(0) += 1;
        for &ban in m.bans.iter().filter(|&&b| b != 0) {
            *ban_counts.entry((ban, m.patch.clone())).or_insert(0) += 1;
        }

        for (p, pos) in &valid {
            let acc = rates
                .entry((p.champion_id, pos.clone(), m.patch.clone()))
                .or_default();
            acc.games += 1;
            acc.wins += u32::from(p.win);
            *role_totals
                .entry((pos.clone(), m.patch.clone()))
                .or_insert(0) += 1;

            let b = builds
                .entry((p.champion_id, pos.clone(), m.patch.clone()))
                .or_default();
            b.games += 1;
            b.wins += u32::from(p.win);
            for &it in p.items.iter().filter(|&&it| is_real_item(it)) {
                *b.item_freq.entry(it).or_insert(0) += 1;
            }
            for &r in &p.runes {
                *b.rune_freq.entry(r).or_insert(0) += 1;
            }
            if p.summoner_spells.len() >= 2 {
                let mut pair = [p.summoner_spells[0], p.summoner_spells[1]];
                pair.sort_unstable();
                *b.spell_freq.entry((pair[0], pair[1])).or_insert(0) += 1;
            }
        }

        // Lane matchups: exactly one participant per team at a position.
        let mut by_pos: HashMap<String, Vec<&MatchV5Participant>> = HashMap::new();
        for (p, pos) in &valid {
            by_pos.entry(pos.clone()).or_default().push(p);
        }
        for (pos, players) in &by_pos {
            if players.len() != 2 {
                continue; // not a clean 1v1 lane (role swap / missing)
            }
            let (a, b) = (players[0], players[1]);
            if a.team_id == b.team_id || a.champion_id == b.champion_id {
                continue; // same team OR mirror → no matchup
            }
            for (x, y) in [(a, b), (b, a)] {
                let acc = matchups
                    .entry((x.champion_id, y.champion_id, pos.clone(), m.patch.clone()))
                    .or_default();
                acc.games += 1;
                acc.wins += u32::from(x.win);
            }
        }
    }

    // ── Build outputs (deterministic order) ─────────────────────────────────────
    let mut rates_out: Vec<AggregatedChampionRate> = rates
        .into_iter()
        .map(|((champion_id, position, patch), acc)| {
            let total = role_totals
                .get(&(position.clone(), patch.clone()))
                .copied()
                .unwrap_or(0)
                .max(1);
            let patch_total = patch_matches.get(&patch).copied().unwrap_or(0).max(1);
            let ban_rate = ban_counts
                .get(&(champion_id, patch.clone()))
                .copied()
                .unwrap_or(0) as f32
                / patch_total as f32;
            AggregatedChampionRate {
                champion_id,
                position,
                games: acc.games,
                wins: acc.wins,
                win_rate: acc.wins as f32 / acc.games as f32,
                pick_rate: acc.games as f32 / total as f32,
                ban_rate,
                sample_size: acc.games,
                patch,
                source: SOURCE.to_string(),
                confidence: confidence_for(acc.games).to_string(),
            }
        })
        .collect();
    rates_out.sort_by(|a, b| {
        a.champion_id
            .cmp(&b.champion_id)
            .then(a.position.cmp(&b.position))
    });

    let mut matchups_out: Vec<AggregatedMatchup> = matchups
        .into_iter()
        .map(
            |((champion_id, opponent_id, position, patch), acc)| AggregatedMatchup {
                champion_id,
                opponent_id,
                position,
                games: acc.games,
                wins: acc.wins,
                win_rate: acc.wins as f32 / acc.games as f32,
                patch,
                source: SOURCE.to_string(),
                confidence: confidence_for(acc.games).to_string(),
            },
        )
        .collect();
    matchups_out.sort_by(|a, b| {
        a.champion_id
            .cmp(&b.champion_id)
            .then(a.position.cmp(&b.position))
            .then(a.opponent_id.cmp(&b.opponent_id))
    });

    let mut builds_out: Vec<AggregatedBuild> = builds
        .into_iter()
        .map(|((champion_id, position, patch), b)| {
            let items = sorted_by_freq(&b.item_freq);
            let core_items: Vec<u32> = items.iter().take(CORE_ITEMS).copied().collect();
            let situational_items: Vec<u32> = items
                .iter()
                .skip(CORE_ITEMS)
                .take(SITUATIONAL_ITEMS)
                .copied()
                .collect();
            let rune_ids = sorted_by_freq(&b.rune_freq).into_iter().take(6).collect();
            let summoner_spells = b
                .spell_freq
                .iter()
                .max_by(|a, c| a.1.cmp(c.1).then(c.0.cmp(a.0)))
                .map(|((s1, s2), _)| vec![*s1, *s2])
                .unwrap_or_default();
            AggregatedBuild {
                champion_id,
                position,
                patch,
                core_items,
                situational_items,
                rune_ids,
                summoner_spells,
                games: b.games,
                win_rate: b.wins as f32 / b.games as f32,
                source: SOURCE.to_string(),
                confidence: confidence_for(b.games).to_string(),
            }
        })
        .collect();
    builds_out.sort_by(|a, b| {
        a.champion_id
            .cmp(&b.champion_id)
            .then(a.position.cmp(&b.position))
    });

    if rates_out.is_empty() && skipped > 0 {
        warn("no_aggregatable_data", &mut warnings);
    }

    let quality = AggregationQuality {
        match_count,
        champion_rate_count: rates_out.len() as u32,
        matchup_count: matchups_out.len() as u32,
        build_count: builds_out.len() as u32,
        skipped_matches: skipped,
        warnings,
    };

    AggregationResult {
        rates: rates_out,
        matchups: matchups_out,
        builds: builds_out,
        quality,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(
        id: u32,
        champ: u32,
        team: u32,
        pos: &str,
        win: bool,
        items: Vec<u32>,
    ) -> MatchV5Participant {
        MatchV5Participant {
            participant_id: id,
            champion_id: champ,
            team_id: team,
            team_position: pos.into(),
            win,
            kills: 0,
            deaths: 0,
            assists: 0,
            items,
            runes: vec![8005, 9111],
            summoner_spells: vec![4, 12],
        }
    }

    /// A standard 5v5 SR match: team 100 (champs 1..5) vs team 200 (champs 11..15),
    /// one champ per position. `team100_wins` sets the winners.
    fn sr_match(id: &str, patch: &str, team100_wins: bool) -> MatchV5 {
        let positions = ["TOP", "JUNGLE", "MIDDLE", "BOTTOM", "UTILITY"];
        let mut participants = Vec::new();
        for (i, pos) in positions.iter().enumerate() {
            participants.push(part(
                i as u32 + 1,
                i as u32 + 1,
                100,
                pos,
                team100_wins,
                vec![1001, 3006],
            ));
            participants.push(part(
                i as u32 + 6,
                i as u32 + 11,
                200,
                pos,
                !team100_wins,
                vec![1001, 3006],
            ));
        }
        MatchV5 {
            match_id: id.into(),
            queue_id: 420,
            patch: patch.into(),
            participants,
            bans: Vec::new(),
        }
    }

    #[test]
    fn two_matches_produce_champion_rates() {
        let r = aggregate_matches(&[
            sr_match("m1", "14.11", true),
            sr_match("m2", "14.11", false),
        ]);
        let champ1 = r
            .rates
            .iter()
            .find(|x| x.champion_id == 1 && x.position == "top")
            .unwrap();
        assert_eq!(champ1.games, 2);
        assert_eq!(champ1.wins, 1); // won m1, lost m2
        assert!((champ1.win_rate - 0.5).abs() < 1e-4);
        assert_eq!(r.quality.match_count, 2);
    }

    #[test]
    fn bans_become_champion_ban_rate() {
        let mut m1 = sr_match("m1", "14.11", true);
        let mut m2 = sr_match("m2", "14.11", false);
        m1.bans = vec![1, 99]; // champ 1 banned; 99 banned-but-never-picked
        m2.bans = vec![1, 0]; // champ 1 banned again; 0 = no-ban → filtered
        let r = aggregate_matches(&[m1, m2]);
        // champ 1 banned in 2/2 matches → ban_rate 1.0 on its rate rows.
        let c1 = r
            .rates
            .iter()
            .find(|x| x.champion_id == 1 && x.position == "top")
            .unwrap();
        assert!((c1.ban_rate - 1.0).abs() < 1e-4);
        // a picked-but-never-banned champ → ban_rate 0.
        let c2 = r.rates.iter().find(|x| x.champion_id == 2).unwrap();
        assert_eq!(c2.ban_rate, 0.0);
    }

    #[test]
    fn same_role_opponents_produce_a_matchup() {
        let r = aggregate_matches(&[sr_match("m1", "14.11", true)]);
        // mid: champ 3 (team100) vs champ 13 (team200).
        let m = r
            .matchups
            .iter()
            .find(|x| x.champion_id == 3 && x.opponent_id == 13 && x.position == "middle")
            .expect("3 vs 13 mid matchup");
        assert_eq!(m.games, 1);
        assert_eq!(m.wins, 1); // team100 won
                               // Reverse direction exists too.
        assert!(r
            .matchups
            .iter()
            .any(|x| x.champion_id == 13 && x.opponent_id == 3 && x.wins == 0));
    }

    #[test]
    fn no_same_team_matchup() {
        let r = aggregate_matches(&[sr_match("m1", "14.11", true)]);
        // champ 1 (team100 top) and champ 2 (team100 jungle) are same team → never paired.
        assert!(!r
            .matchups
            .iter()
            .any(|x| (x.champion_id == 1 && x.opponent_id == 2)
                || (x.champion_id == 2 && x.opponent_id == 1)));
    }

    #[test]
    fn low_sample_is_low_confidence() {
        let r = aggregate_matches(&[sr_match("m1", "14.11", true)]);
        assert!(r.rates.iter().all(|x| x.confidence == "low")); // 1 game each
    }

    #[test]
    fn build_core_items_are_most_frequent_non_zero() {
        // Champ 1 top across 3 matches always builds 1001 + 3006; trinket 3340 + empty 0 filtered.
        let mut matches = Vec::new();
        for i in 0..3 {
            let mut m = sr_match(&format!("m{i}"), "14.11", true);
            m.participants[0].items = vec![1001, 3006, 0, 3340]; // champ 1 top
            matches.push(m);
        }
        let r = aggregate_matches(&matches);
        let b = r
            .builds
            .iter()
            .find(|x| x.champion_id == 1 && x.position == "top")
            .unwrap();
        assert!(b.core_items.contains(&1001) && b.core_items.contains(&3006));
        assert!(!b.core_items.contains(&0), "empty item filtered");
        assert!(!b.core_items.contains(&3340), "trinket filtered");
        assert_eq!(b.summoner_spells, vec![4, 12]);
    }

    #[test]
    fn aram_matches_are_skipped_with_warning() {
        let mut aram = sr_match("aram1", "14.11", true);
        aram.queue_id = ARAM_QUEUE;
        let r = aggregate_matches(&[aram]);
        assert_eq!(r.quality.match_count, 0);
        assert_eq!(r.quality.skipped_matches, 1);
        assert!(r.quality.warnings.contains(&"aram_skipped".to_string()));
        assert!(r.rates.is_empty());
    }

    #[test]
    fn empty_input_is_safe() {
        let r = aggregate_matches(&[]);
        assert_eq!(r.quality.match_count, 0);
        assert!(r.rates.is_empty() && r.matchups.is_empty() && r.builds.is_empty());
    }

    #[test]
    fn output_is_deterministic_and_sorted() {
        let matches = [
            sr_match("m1", "14.11", true),
            sr_match("m2", "14.11", false),
        ];
        let a = aggregate_matches(&matches);
        let b = aggregate_matches(&matches);
        assert_eq!(a, b);
        for w in a.rates.windows(2) {
            assert!(
                (w[0].champion_id, &w[0].position) <= (w[1].champion_id, &w[1].position),
                "rates must be sorted by champion_id then position"
            );
        }
    }

    // ── Schema-drift resilience (engine side) ───────────────────────────────────

    #[test]
    fn drifted_participants_are_skipped_gracefully() {
        // champ_id 0 (missing), empty position (null), and an unknown position
        // string must all be dropped without panicking; valid ones still count.
        let m = MatchV5 {
            match_id: "drift".into(),
            queue_id: 420,
            patch: "14.11".into(),
            participants: vec![
                part(1, 0, 100, "TOP", true, vec![1001]), // champ 0 → dropped
                part(2, 7, 100, "", true, vec![1001]),    // empty pos → dropped
                part(3, 8, 100, "AFK", true, vec![1001]), // unknown pos → dropped
                part(4, 9, 100, "MIDDLE", true, vec![1001]), // valid
                part(5, 19, 200, "MIDDLE", false, vec![1001]),
            ],
            bans: Vec::new(),
        };
        let r = aggregate_matches(&[m]);
        assert_eq!(r.quality.match_count, 1);
        assert!(r
            .rates
            .iter()
            .any(|x| x.champion_id == 9 && x.position == "middle"));
        assert!(!r.rates.iter().any(|x| x.champion_id == 0));
        // Mid lane (9 vs 19) is the only clean 1v1.
        assert!(r
            .matchups
            .iter()
            .any(|x| x.champion_id == 9 && x.opponent_id == 19));
    }

    #[test]
    fn role_swap_same_team_counts_rates_but_no_matchup() {
        // Two mids on team 100, none on team 200 (role swap / bad data): rates count
        // for both, but no matchup is invented (they're same team).
        let m = MatchV5 {
            match_id: "swap".into(),
            queue_id: 420,
            patch: "14.11".into(),
            participants: vec![
                part(1, 10, 100, "MIDDLE", true, vec![1001]),
                part(2, 11, 100, "MIDDLE", true, vec![1001]),
            ],
            bans: Vec::new(),
        };
        let r = aggregate_matches(&[m]);
        assert_eq!(r.rates.len(), 2);
        assert!(
            r.matchups.is_empty(),
            "same-team duo must not become a matchup"
        );
    }

    #[test]
    fn empty_items_yield_no_core_items_without_panicking() {
        let m = MatchV5 {
            match_id: "noitems".into(),
            queue_id: 420,
            patch: "14.11".into(),
            participants: vec![
                part(1, 5, 100, "TOP", true, vec![0, 0, 3340]), // all empty / trinket
                part(2, 15, 200, "TOP", false, vec![]),
            ],
            bans: Vec::new(),
        };
        let r = aggregate_matches(&[m]);
        let b = r.builds.iter().find(|x| x.champion_id == 5).unwrap();
        assert!(b.core_items.is_empty(), "no real items → empty core build");
    }
}
