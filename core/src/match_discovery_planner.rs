//! Match Discovery Planner Core v1 (Sprint, Claude) — engine-pure.
//!
//! Decides which player **hashes** to crawl and which discovered match ids enter
//! the candidate pool — to grow Match-V5 volume beyond the active player's recent
//! games. Pure: no DB / network / Riot / command / UI. **PII-safe by construction:**
//! the module only ever sees + returns `puuid_hash` (the runtime keeps the
//! hash↔raw-PUUID map). No fabrication: no seeds → no crawl; no candidates → no new
//! ids. Decision names are stable machine keys. Upstream of `match_fetch_planner`.
#![allow(dead_code)] // consumed by the crawl / fetch-history wiring (Codex, later)

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use ts_rs::TS;

pub const PLAYER_DECISIONS: [&str; 6] = [
    "crawl",
    "skip_already_crawled",
    "skip_champ_select",
    "skip_budget",
    "skip_breadth_full",
    "skip_invalid",
];
pub const MATCH_DECISIONS: [&str; 4] = ["new", "skip_known", "skip_invalid", "skip_player_cap"];

// ── Inputs (caller-built; Rust-only — carry timestamps as i64) ───────────────────

#[derive(Debug, Clone)]
pub struct DiscoverySeed {
    pub puuid_hash: String,
    pub region: String,
    /// "active_player" | "match_participant" | "manual_seed".
    pub source: String,
    pub seen_at: i64,
    pub contribution_count: u32,
}

#[derive(Debug, Clone)]
pub struct CrawledPlayerRecord {
    pub puuid_hash: String,
    pub region: String,
    pub last_crawled_at: i64,
    pub crawl_count: u32,
}

#[derive(Debug, Clone)]
pub struct DiscoveredMatchCandidate {
    pub match_id: String,
    pub region: String,
    pub source_puuid_hash: String,
    pub discovered_at: i64,
}

#[derive(Debug, Clone)]
pub struct KnownMatchRecord {
    pub match_id: String,
    pub region: String,
    /// Any status (even "failed") counts as known — detail retry is the fetch layer's job.
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct MatchDiscoveryInput {
    pub now: i64,
    pub champ_select_active: bool,
    pub crawl_budget: u32,
    pub max_breadth: u32,
    pub per_player_match_cap: u32,
    pub seeds: Vec<DiscoverySeed>,
    pub crawled_players: Vec<CrawledPlayerRecord>,
    pub candidate_matches: Vec<DiscoveredMatchCandidate>,
    pub known_matches: Vec<KnownMatchRecord>,
}

// ── Outputs (ts-rs; no i64 → no bigint; only `*_hash`, never raw PUUID) ──────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct PlayerCrawlDecision {
    pub puuid_hash: String,
    pub region: String,
    pub decision: String,
    pub reason: String,
    pub priority: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct MatchDiscoveryDecision {
    pub match_id: String,
    pub region: String,
    pub decision: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct MatchDiscoveryPlan {
    pub to_crawl: Vec<String>,
    pub new_match_ids: Vec<String>,
    pub player_decisions: Vec<PlayerCrawlDecision>,
    pub match_decisions: Vec<MatchDiscoveryDecision>,
    pub selected_crawl_count: u32,
    pub new_match_count: u32,
    pub skipped_count: u32,
}

fn source_rank(source: &str) -> u32 {
    match source {
        "active_player" => 3,
        "match_participant" => 2,
        "manual_seed" => 1,
        _ => 0,
    }
}

fn pcd(s: &DiscoverySeed, decision: &str, reason: &str, priority: u32) -> PlayerCrawlDecision {
    PlayerCrawlDecision {
        puuid_hash: s.puuid_hash.clone(),
        region: s.region.clone(),
        decision: decision.to_string(),
        reason: reason.to_string(),
        priority,
    }
}

fn mdd(c: &DiscoveredMatchCandidate, decision: &str, reason: &str) -> MatchDiscoveryDecision {
    MatchDiscoveryDecision {
        match_id: c.match_id.clone(),
        region: c.region.clone(),
        decision: decision.to_string(),
        reason: reason.to_string(),
    }
}

/// Plan player crawl + match candidate intake. Pure + deterministic.
pub fn plan_match_discovery(input: &MatchDiscoveryInput) -> MatchDiscoveryPlan {
    let mut player_decisions: Vec<PlayerCrawlDecision> = Vec::new();
    let mut to_crawl: Vec<String> = Vec::new();

    if input.champ_select_active {
        for s in &input.seeds {
            player_decisions.push(pcd(
                s,
                "skip_champ_select",
                "Champ-select aktif; crawl ertelendi.",
                0,
            ));
        }
    } else {
        let crawled: HashSet<&str> = input
            .crawled_players
            .iter()
            .map(|c| c.puuid_hash.as_str())
            .collect();
        let mut eligible: Vec<&DiscoverySeed> = Vec::new();
        for s in &input.seeds {
            if s.puuid_hash.trim().is_empty() {
                player_decisions.push(pcd(s, "skip_invalid", "Boş puuid hash.", 0));
            } else if crawled.contains(s.puuid_hash.as_str()) {
                player_decisions.push(pcd(s, "skip_already_crawled", "Zaten crawl'lanmış.", 0));
            } else {
                eligible.push(s);
            }
        }
        eligible.sort_by(|a, b| {
            source_rank(&b.source)
                .cmp(&source_rank(&a.source))
                .then(b.contribution_count.cmp(&a.contribution_count))
                .then(b.seen_at.cmp(&a.seen_at))
                .then(a.puuid_hash.cmp(&b.puuid_hash))
        });
        let cap = if input.crawl_budget == 0 {
            0
        } else {
            input.crawl_budget.min(input.max_breadth) as usize
        };
        for (i, s) in eligible.iter().enumerate() {
            let priority = source_rank(&s.source);
            let (decision, reason) = if input.crawl_budget == 0 {
                ("skip_budget", "Crawl bütçesi yetersiz.")
            } else if i < cap {
                to_crawl.push(s.puuid_hash.clone());
                ("crawl", "Crawl edilecek.")
            } else if i >= input.max_breadth as usize {
                ("skip_breadth_full", "Breadth limiti doldu.")
            } else {
                ("skip_budget", "Crawl bütçesi yetersiz.")
            };
            player_decisions.push(pcd(s, decision, reason, priority));
        }
    }

    // ── Match candidates: dedup vs known + per-player cap ────────────────────────
    let known: HashSet<&str> = input
        .known_matches
        .iter()
        .map(|m| m.match_id.as_str())
        .collect();
    let mut candidates: Vec<&DiscoveredMatchCandidate> = input.candidate_matches.iter().collect();
    candidates.sort_by(|a, b| {
        a.source_puuid_hash
            .cmp(&b.source_puuid_hash)
            .then(b.discovered_at.cmp(&a.discovered_at))
            .then(a.match_id.cmp(&b.match_id))
    });
    let mut per_player: HashMap<&str, u32> = HashMap::new();
    let mut match_decisions: Vec<MatchDiscoveryDecision> = Vec::new();
    let mut new_match_ids: Vec<String> = Vec::new();
    for c in &candidates {
        if c.match_id.trim().is_empty() {
            match_decisions.push(mdd(c, "skip_invalid", "Boş match id."));
        } else if known.contains(c.match_id.as_str()) {
            match_decisions.push(mdd(c, "skip_known", "Zaten bilinen match."));
        } else {
            let count = per_player.entry(c.source_puuid_hash.as_str()).or_insert(0);
            if *count >= input.per_player_match_cap {
                match_decisions.push(mdd(
                    c,
                    "skip_player_cap",
                    "Oyuncu başına aday limiti doldu.",
                ));
            } else {
                *count += 1;
                new_match_ids.push(c.match_id.clone());
                match_decisions.push(mdd(c, "new", "Yeni aday match."));
            }
        }
    }

    // Deterministic output ordering for the decision lists.
    player_decisions.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then(a.puuid_hash.cmp(&b.puuid_hash))
    });
    match_decisions.sort_by(|a, b| a.match_id.cmp(&b.match_id));

    let selected_crawl_count = to_crawl.len() as u32;
    let new_match_count = new_match_ids.len() as u32;
    let total = (input.seeds.len() + input.candidate_matches.len()) as u32;
    let skipped_count = total - selected_crawl_count - new_match_count;

    MatchDiscoveryPlan {
        to_crawl,
        new_match_ids,
        player_decisions,
        match_decisions,
        selected_crawl_count,
        new_match_count,
        skipped_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(hash: &str, source: &str, contribution: u32, seen: i64) -> DiscoverySeed {
        DiscoverySeed {
            puuid_hash: hash.into(),
            region: "euw1".into(),
            source: source.into(),
            seen_at: seen,
            contribution_count: contribution,
        }
    }

    fn crawled(hash: &str) -> CrawledPlayerRecord {
        CrawledPlayerRecord {
            puuid_hash: hash.into(),
            region: "euw1".into(),
            last_crawled_at: 0,
            crawl_count: 1,
        }
    }

    fn candi(id: &str, src_hash: &str, discovered: i64) -> DiscoveredMatchCandidate {
        DiscoveredMatchCandidate {
            match_id: id.into(),
            region: "euw1".into(),
            source_puuid_hash: src_hash.into(),
            discovered_at: discovered,
        }
    }

    fn known(id: &str, status: &str) -> KnownMatchRecord {
        KnownMatchRecord {
            match_id: id.into(),
            region: "euw1".into(),
            status: status.into(),
        }
    }

    fn input(
        champ: bool,
        budget: u32,
        breadth: u32,
        cap: u32,
        seeds: Vec<DiscoverySeed>,
        crawled_players: Vec<CrawledPlayerRecord>,
        candidates: Vec<DiscoveredMatchCandidate>,
        known_matches: Vec<KnownMatchRecord>,
    ) -> MatchDiscoveryInput {
        MatchDiscoveryInput {
            now: 1000,
            champ_select_active: champ,
            crawl_budget: budget,
            max_breadth: breadth,
            per_player_match_cap: cap,
            seeds,
            crawled_players,
            candidate_matches: candidates,
            known_matches,
        }
    }

    fn pdec<'a>(plan: &'a MatchDiscoveryPlan, hash: &str) -> &'a PlayerCrawlDecision {
        plan.player_decisions
            .iter()
            .find(|d| d.puuid_hash == hash)
            .unwrap()
    }
    fn mdec<'a>(plan: &'a MatchDiscoveryPlan, id: &str) -> &'a MatchDiscoveryDecision {
        plan.match_decisions
            .iter()
            .find(|d| d.match_id == id)
            .unwrap()
    }

    #[test]
    fn champ_select_skips_all_crawl() {
        let plan = plan_match_discovery(&input(
            true,
            10,
            10,
            10,
            vec![seed("h1", "active_player", 5, 1)],
            vec![],
            vec![],
            vec![],
        ));
        assert!(plan.to_crawl.is_empty());
        assert_eq!(pdec(&plan, "h1").decision, "skip_champ_select");
    }

    #[test]
    fn already_crawled_is_skipped() {
        let plan = plan_match_discovery(&input(
            false,
            10,
            10,
            10,
            vec![seed("h1", "active_player", 5, 1)],
            vec![crawled("h1")],
            vec![],
            vec![],
        ));
        assert_eq!(pdec(&plan, "h1").decision, "skip_already_crawled");
    }

    #[test]
    fn budget_caps_crawl() {
        let plan = plan_match_discovery(&input(
            false,
            2,
            10,
            10,
            vec![
                seed("a", "match_participant", 5, 3),
                seed("b", "match_participant", 5, 2),
                seed("c", "match_participant", 5, 1),
            ],
            vec![],
            vec![],
            vec![],
        ));
        assert_eq!(plan.selected_crawl_count, 2);
        assert_eq!(pdec(&plan, "c").decision, "skip_budget");
    }

    #[test]
    fn breadth_caps_crawl() {
        let plan = plan_match_discovery(&input(
            false,
            10,
            2,
            10,
            vec![
                seed("a", "match_participant", 5, 3),
                seed("b", "match_participant", 5, 2),
                seed("c", "match_participant", 5, 1),
            ],
            vec![],
            vec![],
            vec![],
        ));
        assert_eq!(plan.selected_crawl_count, 2);
        assert_eq!(pdec(&plan, "c").decision, "skip_breadth_full");
    }

    #[test]
    fn active_player_outranks_other_sources() {
        let plan = plan_match_discovery(&input(
            false,
            10,
            1,
            10,
            vec![
                seed("manual", "manual_seed", 99, 99),
                seed("participant", "match_participant", 99, 99),
                seed("active", "active_player", 1, 1),
            ],
            vec![],
            vec![],
            vec![],
        ));
        assert_eq!(plan.to_crawl, vec!["active".to_string()]);
    }

    #[test]
    fn contribution_count_breaks_within_source() {
        let plan = plan_match_discovery(&input(
            false,
            10,
            1,
            10,
            vec![
                seed("low", "match_participant", 2, 5),
                seed("high", "match_participant", 50, 5),
            ],
            vec![],
            vec![],
            vec![],
        ));
        assert_eq!(plan.to_crawl, vec!["high".to_string()]);
    }

    #[test]
    fn deterministic_tie_break_by_hash() {
        let i = input(
            false,
            10,
            1,
            10,
            vec![
                seed("zzz", "match_participant", 5, 5),
                seed("aaa", "match_participant", 5, 5),
            ],
            vec![],
            vec![],
            vec![],
        );
        assert_eq!(plan_match_discovery(&i), plan_match_discovery(&i));
        assert_eq!(plan_match_discovery(&i).to_crawl, vec!["aaa".to_string()]);
    }

    #[test]
    fn empty_seeds_no_fabrication() {
        let plan = plan_match_discovery(&input(false, 10, 10, 10, vec![], vec![], vec![], vec![]));
        assert!(plan.to_crawl.is_empty() && plan.new_match_ids.is_empty());
    }

    #[test]
    fn known_match_is_deduped_even_if_failed() {
        let plan = plan_match_discovery(&input(
            false,
            10,
            10,
            10,
            vec![],
            vec![],
            vec![candi("M1", "h1", 5)],
            vec![known("M1", "failed")], // failed still counts as known
        ));
        assert_eq!(mdec(&plan, "M1").decision, "skip_known");
        assert!(plan.new_match_ids.is_empty());
    }

    #[test]
    fn invalid_match_id_is_skipped() {
        let plan = plan_match_discovery(&input(
            false,
            10,
            10,
            10,
            vec![],
            vec![],
            vec![candi("  ", "h1", 5)],
            vec![],
        ));
        assert_eq!(plan.match_decisions[0].decision, "skip_invalid");
    }

    #[test]
    fn per_player_match_cap_limits_new_ids() {
        let plan = plan_match_discovery(&input(
            false,
            10,
            10,
            2,
            vec![],
            vec![],
            vec![
                candi("M1", "h1", 3),
                candi("M2", "h1", 2),
                candi("M3", "h1", 1), // over cap 2
            ],
            vec![],
        ));
        assert_eq!(plan.new_match_count, 2);
        assert_eq!(mdec(&plan, "M3").decision, "skip_player_cap");
    }

    #[test]
    fn selected_and_skipped_counts_are_consistent() {
        let plan = plan_match_discovery(&input(
            false,
            1,
            10,
            1,
            vec![
                seed("a", "active_player", 5, 1),
                seed("b", "active_player", 5, 1),
            ],
            vec![],
            vec![candi("M1", "a", 2), candi("M2", "a", 1)],
            vec![],
        ));
        let total = 2 + 2;
        assert_eq!(
            plan.selected_crawl_count + plan.new_match_count + plan.skipped_count,
            total
        );
    }

    #[test]
    fn emitted_decisions_stay_in_vocabulary() {
        assert_eq!(PLAYER_DECISIONS.len(), 6);
        assert_eq!(MATCH_DECISIONS.len(), 4);
        let plan = plan_match_discovery(&input(
            false,
            1,
            1,
            1,
            vec![
                seed("a", "active_player", 5, 3),     // crawl
                seed("b", "match_participant", 5, 2), // breadth/budget skip
                seed("", "manual_seed", 1, 1),        // invalid
                seed("done", "active_player", 9, 9),  // already crawled
            ],
            vec![crawled("done")],
            vec![candi("M1", "a", 2), candi("M2", "a", 1), candi("", "a", 0)],
            vec![known("M2", "fetched")],
        ));
        for d in &plan.player_decisions {
            assert!(
                PLAYER_DECISIONS.contains(&d.decision.as_str()),
                "player {}",
                d.decision
            );
        }
        for d in &plan.match_decisions {
            assert!(
                MATCH_DECISIONS.contains(&d.decision.as_str()),
                "match {}",
                d.decision
            );
        }
    }

    #[test]
    fn output_exposes_only_hashes_no_raw_puuid() {
        let plan = plan_match_discovery(&input(
            false,
            10,
            10,
            10,
            vec![seed("h1", "active_player", 5, 1)],
            vec![],
            vec![],
            vec![],
        ));
        let json = serde_json::to_value(&plan.player_decisions[0]).unwrap();
        let keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(|s| s.as_str())
            .collect();
        // Only the hash field carries identity; no bare "puuid" / "summoner" / "name".
        assert!(keys.contains(&"puuid_hash"));
        assert!(!keys
            .iter()
            .any(|k| *k == "puuid" || k.contains("summoner") || k.contains("name")));
    }
}
