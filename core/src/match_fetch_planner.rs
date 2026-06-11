//! Match Fetch Planner Core (Sprint, Claude) — engine-pure.
//!
//! Before growing Match-V5 coverage, decides *which* candidate match ids to fetch:
//! deterministic, deduped against fetch history, coverage-gap-aware, rate-limit +
//! batch bounded, and champ-select-safe. Pure: no DB / network / command / UI.
//! No fabrication: with no coverage gap the plan fetches nothing (it never invents
//! a reason to spend rate budget). Decision names are stable machine keys.
#![allow(dead_code)] // consumed by the batch-fetch / fetch-history wiring (Codex, later)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

/// Fixed decision vocabulary (UI i18n binds later under `dataPipeline.fetchPlan.*`).
pub const FETCH_DECISIONS: [&str; 7] = [
    "fetch",
    "skip_already_fetched",
    "skip_rate_limited",
    "skip_champ_select",
    "skip_batch_full",
    "skip_invalid",
    "skip_no_gap",
];

// ── Inputs (caller-built; Rust-only) ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MatchCandidate {
    pub match_id: String,
    pub region: String,
    pub patch: String,
    pub queue_id: u32,
    pub role_hint: Option<String>,
    pub discovered_at: i64,
}

#[derive(Debug, Clone)]
pub struct FetchedMatchRecord {
    pub match_id: String,
    pub region: String,
    pub patch: String,
    /// "fetched" | "parsed" | "processed" | "failed".
    pub status: String,
    pub fetched_at: i64,
}

#[derive(Debug, Clone)]
pub struct CoverageGap {
    pub region: String,
    pub patch: String,
    pub role: String,
    pub current_samples: u32,
    pub target_samples: u32,
    pub priority: u32,
}

#[derive(Debug, Clone)]
pub struct MatchFetchPlannerInput {
    pub now: i64,
    pub champ_select_active: bool,
    pub rate_budget: u32,
    pub batch_limit: u32,
    pub candidates: Vec<MatchCandidate>,
    pub fetched_records: Vec<FetchedMatchRecord>,
    pub coverage_gaps: Vec<CoverageGap>,
}

// ── Outputs (ts-rs; no i64 → no bigint) ─────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct MatchFetchDecision {
    pub match_id: String,
    pub decision: String,
    pub reason: String,
    pub priority: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct MatchFetchPlan {
    /// Match ids to fetch, in selection (priority) order.
    pub to_fetch: Vec<String>,
    pub decisions: Vec<MatchFetchDecision>,
    pub batch_limit: u32,
    pub selected_count: u32,
    pub skipped_count: u32,
}

fn dec(match_id: &str, decision: &str, reason: &str, priority: u32) -> MatchFetchDecision {
    MatchFetchDecision {
        match_id: match_id.to_string(),
        decision: decision.to_string(),
        reason: reason.to_string(),
        priority,
    }
}

fn status_rank(status: &str) -> i32 {
    match status {
        "processed" => 3,
        "parsed" => 2,
        "fetched" => 1,
        "failed" => 0,
        _ => -1,
    }
}

/// Highest-priority active gap (current < target) matching the candidate's
/// region/patch (and role, when `role_hint` is set). None → no coverage need.
fn best_gap_priority(gaps: &[&CoverageGap], c: &MatchCandidate) -> Option<u32> {
    gaps.iter()
        .filter(|g| g.region == c.region && g.patch == c.patch)
        .filter(|g| c.role_hint.as_deref().map(|r| r == g.role).unwrap_or(true))
        .map(|g| g.priority)
        .max()
}

/// Plan which candidate matches to fetch. Pure + deterministic
/// (priority desc, discovered_at desc, match_id asc).
pub fn plan_match_fetch(input: &MatchFetchPlannerInput) -> MatchFetchPlan {
    let total = input.candidates.len() as u32;

    if input.champ_select_active {
        let mut decisions: Vec<MatchFetchDecision> = input
            .candidates
            .iter()
            .map(|c| {
                dec(
                    &c.match_id,
                    "skip_champ_select",
                    "Champ-select aktif; fetch ertelendi.",
                    0,
                )
            })
            .collect();
        decisions.sort_by(|a, b| a.match_id.cmp(&b.match_id));
        return MatchFetchPlan {
            to_fetch: Vec::new(),
            decisions,
            batch_limit: input.batch_limit,
            selected_count: 0,
            skipped_count: total,
        };
    }

    // Best fetch progress per match id (processed > parsed > fetched > failed).
    let mut fetched: HashMap<&str, i32> = HashMap::new();
    for r in &input.fetched_records {
        let rank = status_rank(&r.status);
        let e = fetched.entry(r.match_id.as_str()).or_insert(i32::MIN);
        if rank > *e {
            *e = rank;
        }
    }

    let active_gaps: Vec<&CoverageGap> = input
        .coverage_gaps
        .iter()
        .filter(|g| g.current_samples < g.target_samples)
        .collect();

    let mut decisions: Vec<MatchFetchDecision> = Vec::new();
    // (candidate, priority) for sources eligible to fetch.
    let mut eligible: Vec<(&MatchCandidate, u32)> = Vec::new();

    for c in &input.candidates {
        if c.match_id.trim().is_empty() {
            decisions.push(dec(&c.match_id, "skip_invalid", "Boş match id.", 0));
            continue;
        }
        let best = fetched
            .get(c.match_id.as_str())
            .copied()
            .unwrap_or(i32::MIN);
        if best >= 1 {
            decisions.push(dec(
                &c.match_id,
                "skip_already_fetched",
                "Zaten çekilmiş.",
                0,
            ));
            continue;
        }
        let is_failed_retry = best == 0;
        match best_gap_priority(&active_gaps, c) {
            None => decisions.push(dec(
                &c.match_id,
                "skip_no_gap",
                "Eşleşen coverage açığı yok.",
                0,
            )),
            Some(gap_priority) => {
                // Failed retries are allowed but deprioritized.
                let priority = if is_failed_retry {
                    gap_priority / 2
                } else {
                    gap_priority
                };
                eligible.push((c, priority));
            }
        }
    }

    eligible.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then(b.0.discovered_at.cmp(&a.0.discovered_at))
            .then(a.0.match_id.cmp(&b.0.match_id))
    });

    let cap = if input.rate_budget == 0 {
        0
    } else {
        input.batch_limit.min(input.rate_budget) as usize
    };

    let mut to_fetch = Vec::new();
    for (i, (c, priority)) in eligible.iter().enumerate() {
        let decision = if input.rate_budget == 0 {
            "skip_rate_limited"
        } else if i < cap {
            to_fetch.push(c.match_id.clone());
            "fetch"
        } else if i >= input.batch_limit as usize {
            "skip_batch_full"
        } else {
            "skip_rate_limited"
        };
        let reason = match decision {
            "fetch" => "Coverage açığı için çekilecek.",
            "skip_rate_limited" => "Rate-limit bütçesi yetersiz.",
            "skip_batch_full" => "Batch limiti doldu.",
            _ => "",
        };
        decisions.push(dec(&c.match_id, decision, reason, *priority));
    }

    let selected_count = to_fetch.len() as u32;
    decisions.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then(a.match_id.cmp(&b.match_id))
    });

    MatchFetchPlan {
        to_fetch,
        decisions,
        batch_limit: input.batch_limit,
        selected_count,
        skipped_count: total - selected_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(id: &str, role: Option<&str>, discovered: i64) -> MatchCandidate {
        MatchCandidate {
            match_id: id.into(),
            region: "euw1".into(),
            patch: "14.11".into(),
            queue_id: 420,
            role_hint: role.map(String::from),
            discovered_at: discovered,
        }
    }

    fn rec(id: &str, status: &str) -> FetchedMatchRecord {
        FetchedMatchRecord {
            match_id: id.into(),
            region: "euw1".into(),
            patch: "14.11".into(),
            status: status.into(),
            fetched_at: 0,
        }
    }

    fn gap(role: &str, cur: u32, target: u32, priority: u32) -> CoverageGap {
        CoverageGap {
            region: "euw1".into(),
            patch: "14.11".into(),
            role: role.into(),
            current_samples: cur,
            target_samples: target,
            priority,
        }
    }

    fn input(
        champ: bool,
        budget: u32,
        batch: u32,
        candidates: Vec<MatchCandidate>,
        fetched: Vec<FetchedMatchRecord>,
        gaps: Vec<CoverageGap>,
    ) -> MatchFetchPlannerInput {
        MatchFetchPlannerInput {
            now: 1000,
            champ_select_active: champ,
            rate_budget: budget,
            batch_limit: batch,
            candidates,
            fetched_records: fetched,
            coverage_gaps: gaps,
        }
    }

    fn dec_of<'a>(plan: &'a MatchFetchPlan, id: &str) -> &'a MatchFetchDecision {
        plan.decisions.iter().find(|d| d.match_id == id).unwrap()
    }

    fn open_gap() -> Vec<CoverageGap> {
        vec![gap("middle", 10, 1000, 5)]
    }

    #[test]
    fn already_fetched_is_deduped() {
        let plan = plan_match_fetch(&input(
            false,
            10,
            10,
            vec![cand("M1", Some("middle"), 1)],
            vec![rec("M1", "processed")],
            open_gap(),
        ));
        assert_eq!(dec_of(&plan, "M1").decision, "skip_already_fetched");
        assert!(plan.to_fetch.is_empty());
    }

    #[test]
    fn failed_record_is_retried_but_lower_priority() {
        // M_failed (failed retry) vs M_fresh (new); same gap, batch 1 → fresh wins.
        let plan = plan_match_fetch(&input(
            false,
            10,
            1,
            vec![
                cand("M_fresh", Some("middle"), 5),
                cand("M_failed", Some("middle"), 9),
            ],
            vec![rec("M_failed", "failed")],
            open_gap(),
        ));
        assert_eq!(
            plan.to_fetch,
            vec!["M_fresh".to_string()],
            "fresh outranks failed retry"
        );
        assert!(dec_of(&plan, "M_failed").priority < dec_of(&plan, "M_fresh").priority);
        assert_eq!(dec_of(&plan, "M_failed").decision, "skip_batch_full");
    }

    #[test]
    fn champ_select_skips_everything() {
        let plan = plan_match_fetch(&input(
            true,
            10,
            10,
            vec![cand("M1", Some("middle"), 1)],
            vec![],
            open_gap(),
        ));
        assert!(plan.to_fetch.is_empty());
        assert_eq!(dec_of(&plan, "M1").decision, "skip_champ_select");
    }

    #[test]
    fn zero_rate_budget_skips_eligible() {
        let plan = plan_match_fetch(&input(
            false,
            0,
            10,
            vec![cand("M1", Some("middle"), 1)],
            vec![],
            open_gap(),
        ));
        assert_eq!(dec_of(&plan, "M1").decision, "skip_rate_limited");
    }

    #[test]
    fn batch_limit_caps_selection() {
        let plan = plan_match_fetch(&input(
            false,
            10,
            2,
            vec![
                cand("A", Some("middle"), 3),
                cand("B", Some("middle"), 2),
                cand("C", Some("middle"), 1),
            ],
            vec![],
            open_gap(),
        ));
        assert_eq!(plan.selected_count, 2);
        // Sorted by discovered_at desc: A, B fetched; C overflow.
        assert_eq!(plan.to_fetch, vec!["A".to_string(), "B".to_string()]);
        assert_eq!(dec_of(&plan, "C").decision, "skip_batch_full");
    }

    #[test]
    fn empty_match_id_is_invalid() {
        let plan = plan_match_fetch(&input(
            false,
            10,
            10,
            vec![cand("   ", Some("middle"), 1)],
            vec![],
            open_gap(),
        ));
        assert_eq!(plan.decisions[0].decision, "skip_invalid");
    }

    #[test]
    fn coverage_gap_priority_orders_selection() {
        // Two roles, different gap priority; batch 1 → higher-priority role first.
        let plan = plan_match_fetch(&input(
            false,
            10,
            1,
            vec![cand("low", Some("top"), 5), cand("high", Some("middle"), 5)],
            vec![],
            vec![gap("top", 10, 1000, 2), gap("middle", 10, 1000, 9)],
        ));
        assert_eq!(plan.to_fetch, vec!["high".to_string()]);
        assert!(dec_of(&plan, "high").priority > dec_of(&plan, "low").priority);
    }

    #[test]
    fn no_gap_returns_no_fetch() {
        // No active gaps → nothing to fetch (no fabricated coverage).
        let plan = plan_match_fetch(&input(
            false,
            10,
            10,
            vec![cand("M1", Some("middle"), 1)],
            vec![],
            vec![],
        ));
        assert!(plan.to_fetch.is_empty());
        assert_eq!(dec_of(&plan, "M1").decision, "skip_no_gap");
        // A met gap (current >= target) is also no-gap.
        let met = plan_match_fetch(&input(
            false,
            10,
            10,
            vec![cand("M1", Some("middle"), 1)],
            vec![],
            vec![gap("middle", 1000, 1000, 5)],
        ));
        assert_eq!(dec_of(&met, "M1").decision, "skip_no_gap");
    }

    #[test]
    fn output_is_deterministic() {
        let i = input(
            false,
            10,
            2,
            vec![cand("B", Some("middle"), 2), cand("A", Some("middle"), 3)],
            vec![],
            open_gap(),
        );
        assert_eq!(plan_match_fetch(&i), plan_match_fetch(&i));
    }

    #[test]
    fn selected_plus_skipped_equals_total() {
        let plan = plan_match_fetch(&input(
            false,
            10,
            1,
            vec![
                cand("A", Some("middle"), 3),
                cand("B", Some("middle"), 2),
                cand("", Some("middle"), 1),
            ],
            vec![],
            open_gap(),
        ));
        assert_eq!(plan.selected_count + plan.skipped_count, 3);
        assert_eq!(plan.selected_count, 1);
    }

    #[test]
    fn emitted_decisions_stay_in_vocabulary() {
        assert_eq!(FETCH_DECISIONS.len(), 7);
        // Drive a mix that surfaces several decisions.
        let plan = plan_match_fetch(&input(
            false,
            1,
            1,
            vec![
                cand("A", Some("middle"), 3), // fetch
                cand("B", Some("middle"), 2), // skip_batch_full
                cand("", None, 1),            // skip_invalid
                cand("D", Some("middle"), 1), // already fetched
                cand("E", Some("jungle"), 1), // no gap (no jungle gap)
            ],
            vec![rec("D", "parsed")],
            open_gap(),
        ));
        for d in &plan.decisions {
            assert!(
                FETCH_DECISIONS.contains(&d.decision.as_str()),
                "unknown {}",
                d.decision
            );
        }
        // champ-select branch token.
        let cs = plan_match_fetch(&input(
            true,
            1,
            1,
            vec![cand("A", None, 1)],
            vec![],
            open_gap(),
        ));
        assert_eq!(cs.decisions[0].decision, "skip_champ_select");
    }
}
