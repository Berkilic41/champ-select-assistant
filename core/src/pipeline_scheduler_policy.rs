//! Data Pipeline Scheduler Policy Core (Sprint, Claude) — engine-pure.
//!
//! The decision brain for the (future, Codex-owned) runtime refresh scheduler:
//!   * `plan_refresh` — per-source refresh/skip decisions (champ-select guard,
//!     freshness, rate-limit, enable flag, budget).
//!   * `compute_rate_budget` — sliding-window request budget + next-allowed time.
//!   * `summarize_fetch_logs` — per-source health from `source_fetch_log` entries.
//!
//! Pure: no DB / network / tokio / command / UI. No fabrication: a source with no
//! successful fetch is `insufficient`, never a fake `healthy`. Decision/health/
//! status names are stable machine keys (UI i18n-maps later — see doc).
#![allow(dead_code)] // consumed by the runtime scheduler / observability wiring (Codex, later)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

/// Last success older than this reads as `stale`.
const STALE_SECS: i64 = 48 * 3600;
/// This many consecutive failures reads as `degraded`.
const FAIL_DEGRADE_STREAK: u32 = 3;

/// Fixed token vocabularies (UI i18n binds these later under `dataPipeline.scheduler.*`).
pub const REFRESH_DECISIONS: [&str; 6] = [
    "refresh",
    "skip_fresh",
    "skip_rate_limited",
    "skip_champ_select",
    "skip_disabled",
    "skip_no_budget",
];
pub const SOURCE_HEALTHS: [&str; 4] = ["healthy", "degraded", "stale", "insufficient"];
pub const FETCH_STATUSES: [&str; 4] = ["success", "failed", "rate_limited", "skipped"];

// ── plan_refresh ────────────────────────────────────────────────────────────────

/// One source's state for refresh planning (caller-supplied).
#[derive(Debug, Clone)]
pub struct RefreshSourceInput {
    pub source: String,
    pub enabled: bool,
    pub last_fetch_at: Option<i64>,
    pub ttl_secs: i64,
    /// Current health (from `summarize_fetch_logs` / pipeline quality).
    pub health: String,
    /// Per-source rate-limit cooldown end (unix seconds), if any.
    pub next_allowed_at: Option<i64>,
}

/// Full refresh-planning request.
#[derive(Debug, Clone)]
pub struct RefreshPolicyInput {
    pub now: i64,
    pub champ_select_active: bool,
    /// Global request budget left this window (`compute_rate_budget().remaining`).
    pub remaining_budget: u32,
    pub sources: Vec<RefreshSourceInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct RefreshSourceDecision {
    pub source: String,
    pub decision: String,
    pub reason: String,
    pub next_allowed_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct RefreshPlan {
    pub decisions: Vec<RefreshSourceDecision>,
    pub refresh_count: u32,
    pub champ_select_blocked: bool,
}

/// Plan which sources to refresh now. Deterministic (sorted by source name).
/// **Champ-select hard rule:** when active, every source is `skip_champ_select`
/// (no network during champ-select).
pub fn plan_refresh(input: &RefreshPolicyInput) -> RefreshPlan {
    let mut sources: Vec<&RefreshSourceInput> = input.sources.iter().collect();
    sources.sort_by(|a, b| a.source.cmp(&b.source));

    let mut remaining = input.remaining_budget;
    let mut refresh_count = 0u32;
    let mut decisions = Vec::with_capacity(sources.len());

    for s in sources {
        let (decision, reason, next): (&str, &str, Option<i64>) = if input.champ_select_active {
            (
                "skip_champ_select",
                "Champ-select aktif; refresh ertelendi.",
                None,
            )
        } else if !s.enabled {
            ("skip_disabled", "Kaynak devre dışı.", None)
        } else if s.next_allowed_at.map(|t| t > input.now).unwrap_or(false) {
            (
                "skip_rate_limited",
                "Kaynak rate-limit cooldown'da.",
                s.next_allowed_at,
            )
        } else {
            let fresh = s
                .last_fetch_at
                .map(|t| t + s.ttl_secs > input.now)
                .unwrap_or(false);
            let needs =
                !fresh || matches!(s.health.as_str(), "stale" | "insufficient" | "degraded");
            if !needs {
                ("skip_fresh", "Veri taze; yenileme gereksiz.", None)
            } else if remaining == 0 {
                (
                    "skip_no_budget",
                    "Rate-limit bütçesi tükendi; sonraki pencerede.",
                    None,
                )
            } else {
                remaining -= 1;
                refresh_count += 1;
                ("refresh", "Bayat/eksik kaynak; yenilenecek.", None)
            }
        };
        decisions.push(RefreshSourceDecision {
            source: s.source.clone(),
            decision: decision.to_string(),
            reason: reason.to_string(),
            next_allowed_at: next,
        });
    }

    RefreshPlan {
        decisions,
        refresh_count,
        champ_select_blocked: input.champ_select_active,
    }
}

// ── compute_rate_budget ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RateLimitInput {
    pub now: i64,
    pub window_secs: i64,
    pub max_requests: u32,
    /// Unix-second timestamps of recent requests.
    pub request_timestamps: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct RateLimitBudget {
    pub max_requests: u32,
    pub used: u32,
    pub remaining: u32,
    pub window_secs: i64,
    /// When a slot frees (oldest in-window request + window). None when budget left.
    pub next_allowed_at: Option<i64>,
}

/// Sliding-window request budget. Pure.
pub fn compute_rate_budget(input: &RateLimitInput) -> RateLimitBudget {
    let window_start = input.now - input.window_secs;
    let in_window: Vec<i64> = input
        .request_timestamps
        .iter()
        .copied()
        .filter(|&t| t > window_start && t <= input.now)
        .collect();
    let used = in_window.len() as u32;
    let remaining = input.max_requests.saturating_sub(used);
    let next_allowed_at = if remaining > 0 {
        None
    } else {
        in_window
            .iter()
            .min()
            .map(|&oldest| oldest + input.window_secs)
    };
    RateLimitBudget {
        max_requests: input.max_requests,
        used,
        remaining,
        window_secs: input.window_secs,
        next_allowed_at,
    }
}

// ── summarize_fetch_logs ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FetchLogEntry {
    pub source: String,
    /// One of FETCH_STATUSES.
    pub status: String,
    pub at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct SourceFetchHealth {
    pub source: String,
    pub total: u32,
    pub success: u32,
    pub failed: u32,
    pub success_streak: u32,
    pub fail_streak: u32,
    pub last_success_at: Option<i64>,
    pub last_attempt_at: Option<i64>,
    pub health: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct FetchLogSummary {
    pub sources: Vec<SourceFetchHealth>,
    pub total_entries: u32,
    pub healthy_count: u32,
    pub degraded_count: u32,
}

/// Per-source health from fetch-log entries. Deterministic (sorted by source).
/// A source with no `success` entry is `insufficient` (no fabrication).
pub fn summarize_fetch_logs(logs: &[FetchLogEntry], now: i64) -> FetchLogSummary {
    let mut by_source: HashMap<&str, Vec<&FetchLogEntry>> = HashMap::new();
    for e in logs {
        by_source.entry(e.source.as_str()).or_default().push(e);
    }

    let mut sources: Vec<SourceFetchHealth> = by_source
        .into_iter()
        .map(|(source, mut entries)| {
            entries.sort_by_key(|e| e.at); // ascending; latest = last
            let total = entries.len() as u32;
            let success = entries.iter().filter(|e| e.status == "success").count() as u32;
            let failed = entries.iter().filter(|e| e.status == "failed").count() as u32;

            let mut success_streak = 0;
            for e in entries.iter().rev() {
                if e.status == "success" {
                    success_streak += 1;
                } else {
                    break;
                }
            }
            let mut fail_streak = 0;
            for e in entries.iter().rev() {
                if e.status == "failed" {
                    fail_streak += 1;
                } else {
                    break;
                }
            }

            let last_success_at = entries
                .iter()
                .filter(|e| e.status == "success")
                .map(|e| e.at)
                .max();
            let last_attempt_at = entries.iter().map(|e| e.at).max();

            let health = if success == 0 {
                "insufficient"
            } else if fail_streak >= FAIL_DEGRADE_STREAK {
                "degraded"
            } else if last_success_at
                .map(|t| now - t > STALE_SECS)
                .unwrap_or(true)
            {
                "stale"
            } else {
                "healthy"
            };

            SourceFetchHealth {
                source: source.to_string(),
                total,
                success,
                failed,
                success_streak,
                fail_streak,
                last_success_at,
                last_attempt_at,
                health: health.to_string(),
            }
        })
        .collect();
    sources.sort_by(|a, b| a.source.cmp(&b.source));

    let healthy_count = sources.iter().filter(|s| s.health == "healthy").count() as u32;
    let degraded_count = sources.iter().filter(|s| s.health == "degraded").count() as u32;

    FetchLogSummary {
        sources,
        total_entries: logs.len() as u32,
        healthy_count,
        degraded_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3600;
    const NOW: i64 = 1_000 * HOUR;

    fn src(
        source: &str,
        enabled: bool,
        last_fetch_h: Option<i64>,
        ttl_h: i64,
        health: &str,
    ) -> RefreshSourceInput {
        RefreshSourceInput {
            source: source.into(),
            enabled,
            last_fetch_at: last_fetch_h.map(|h| NOW - h * HOUR),
            ttl_secs: ttl_h * HOUR,
            health: health.into(),
            next_allowed_at: None,
        }
    }

    fn policy(
        champ_select: bool,
        budget: u32,
        sources: Vec<RefreshSourceInput>,
    ) -> RefreshPolicyInput {
        RefreshPolicyInput {
            now: NOW,
            champ_select_active: champ_select,
            remaining_budget: budget,
            sources,
        }
    }

    fn decision_of<'a>(plan: &'a RefreshPlan, source: &str) -> &'a RefreshSourceDecision {
        plan.decisions.iter().find(|d| d.source == source).unwrap()
    }

    #[test]
    fn champ_select_blocks_all_sources() {
        let plan = plan_refresh(&policy(
            true,
            100,
            vec![
                src("meraki", true, Some(100), 24, "stale"),
                src("ddragon", true, None, 24, "insufficient"),
            ],
        ));
        assert!(plan.champ_select_blocked);
        assert_eq!(plan.refresh_count, 0);
        assert!(plan
            .decisions
            .iter()
            .all(|d| d.decision == "skip_champ_select"));
    }

    #[test]
    fn fresh_healthy_source_skips() {
        let plan = plan_refresh(&policy(
            false,
            100,
            vec![src("meraki", true, Some(1), 24, "healthy")],
        ));
        assert_eq!(decision_of(&plan, "meraki").decision, "skip_fresh");
    }

    #[test]
    fn stale_source_refreshes() {
        let plan = plan_refresh(&policy(
            false,
            100,
            vec![src("meraki", true, Some(48), 24, "stale")],
        ));
        assert_eq!(decision_of(&plan, "meraki").decision, "refresh");
        assert_eq!(plan.refresh_count, 1);
    }

    #[test]
    fn insufficient_source_refreshes_even_if_recently_fetched() {
        let plan = plan_refresh(&policy(
            false,
            100,
            vec![src("match_v5", true, Some(1), 24, "insufficient")],
        ));
        assert_eq!(decision_of(&plan, "match_v5").decision, "refresh");
    }

    #[test]
    fn disabled_source_skips() {
        let plan = plan_refresh(&policy(
            false,
            100,
            vec![src("scraper", false, Some(99), 24, "stale")],
        ));
        assert_eq!(decision_of(&plan, "scraper").decision, "skip_disabled");
    }

    #[test]
    fn rate_limited_source_skips_with_next_allowed() {
        let mut s = src("riot", true, Some(48), 24, "stale");
        s.next_allowed_at = Some(NOW + 5 * HOUR);
        let plan = plan_refresh(&policy(false, 100, vec![s]));
        let d = decision_of(&plan, "riot");
        assert_eq!(d.decision, "skip_rate_limited");
        assert_eq!(d.next_allowed_at, Some(NOW + 5 * HOUR));
    }

    #[test]
    fn exhausted_budget_skips_eligible_sources() {
        // Two eligible (stale) sources but only budget for one.
        let plan = plan_refresh(&policy(
            false,
            1,
            vec![
                src("aaa", true, Some(48), 24, "stale"),
                src("bbb", true, Some(48), 24, "stale"),
            ],
        ));
        assert_eq!(plan.refresh_count, 1);
        // Deterministic: sorted by name → "aaa" refreshes, "bbb" runs out of budget.
        assert_eq!(decision_of(&plan, "aaa").decision, "refresh");
        assert_eq!(decision_of(&plan, "bbb").decision, "skip_no_budget");
    }

    #[test]
    fn rate_budget_counts_in_window_requests() {
        let input = RateLimitInput {
            now: NOW,
            window_secs: HOUR,
            max_requests: 5,
            request_timestamps: vec![NOW - 10, NOW - 100, NOW - 2 * HOUR /* old, excluded */],
        };
        let b = compute_rate_budget(&input);
        assert_eq!(b.used, 2);
        assert_eq!(b.remaining, 3);
        assert_eq!(b.next_allowed_at, None);
    }

    #[test]
    fn exhausted_rate_budget_reports_next_allowed_at() {
        let input = RateLimitInput {
            now: NOW,
            window_secs: HOUR,
            max_requests: 2,
            request_timestamps: vec![NOW - 600, NOW - 1200], // both in window, oldest = NOW-1200
        };
        let b = compute_rate_budget(&input);
        assert_eq!(b.remaining, 0);
        assert_eq!(b.next_allowed_at, Some(NOW - 1200 + HOUR));
    }

    fn log(source: &str, status: &str, at_h_ago: i64) -> FetchLogEntry {
        FetchLogEntry {
            source: source.into(),
            status: status.into(),
            at: NOW - at_h_ago * HOUR,
        }
    }

    #[test]
    fn fetch_log_streaks_and_last_success() {
        // latest two are failures → fail_streak 2; an earlier success exists.
        let logs = vec![
            log("meraki", "success", 10),
            log("meraki", "failed", 2),
            log("meraki", "failed", 1),
        ];
        let s = &summarize_fetch_logs(&logs, NOW).sources[0];
        assert_eq!(s.total, 3);
        assert_eq!(s.success, 1);
        assert_eq!(s.failed, 2);
        assert_eq!(s.fail_streak, 2);
        assert_eq!(s.success_streak, 0);
        assert_eq!(s.last_success_at, Some(NOW - 10 * HOUR));
    }

    #[test]
    fn source_with_no_success_is_insufficient() {
        let logs = vec![log("scraper", "skipped", 1), log("scraper", "failed", 2)];
        let s = &summarize_fetch_logs(&logs, NOW).sources[0];
        assert_eq!(s.health, "insufficient");
    }

    #[test]
    fn fetch_log_health_bands() {
        // healthy: recent success, no fail streak.
        let healthy = summarize_fetch_logs(&[log("a", "success", 1)], NOW);
        assert_eq!(healthy.sources[0].health, "healthy");
        // stale: only old success.
        let stale = summarize_fetch_logs(&[log("a", "success", 72)], NOW);
        assert_eq!(stale.sources[0].health, "stale");
        // degraded: recent success but 3 trailing failures.
        let degraded = summarize_fetch_logs(
            &[
                log("a", "success", 5),
                log("a", "failed", 3),
                log("a", "failed", 2),
                log("a", "failed", 1),
            ],
            NOW,
        );
        assert_eq!(degraded.sources[0].health, "degraded");
    }

    #[test]
    fn outputs_are_deterministic_and_sorted() {
        let p = policy(
            false,
            100,
            vec![
                src("zeta", true, Some(48), 24, "stale"),
                src("alpha", true, Some(48), 24, "stale"),
            ],
        );
        assert_eq!(plan_refresh(&p), plan_refresh(&p));
        let plan = plan_refresh(&p);
        assert!(plan.decisions[0].source < plan.decisions[1].source);

        let logs = vec![log("zeta", "success", 1), log("alpha", "success", 1)];
        let summary = summarize_fetch_logs(&logs, NOW);
        assert!(summary.sources[0].source < summary.sources[1].source);
    }

    #[test]
    fn emitted_tokens_stay_in_fixed_vocabulary() {
        assert_eq!(REFRESH_DECISIONS.len(), 6);
        assert_eq!(SOURCE_HEALTHS.len(), 4);
        assert_eq!(FETCH_STATUSES.len(), 4);

        // Drive plan_refresh across every decision branch and check the vocabulary.
        let mut rl = src("rl", true, Some(48), 24, "stale");
        rl.next_allowed_at = Some(NOW + HOUR);
        let plan = plan_refresh(&policy(
            false,
            1,
            vec![
                src("aaa", true, Some(48), 24, "stale"),  // refresh
                src("bbb", true, Some(48), 24, "stale"),  // skip_no_budget
                src("ccc", true, Some(1), 24, "healthy"), // skip_fresh
                src("ddd", false, Some(1), 24, "stale"),  // skip_disabled
                rl,                                       // skip_rate_limited
            ],
        ));
        for d in &plan.decisions {
            assert!(
                REFRESH_DECISIONS.contains(&d.decision.as_str()),
                "unknown decision {}",
                d.decision
            );
        }
        // champ-select branch.
        let cs = plan_refresh(&policy(true, 1, vec![src("aaa", true, None, 24, "stale")]));
        assert_eq!(cs.decisions[0].decision, "skip_champ_select");

        // Health tokens from the summarizer.
        let summary = summarize_fetch_logs(
            &[
                log("a", "success", 1),
                log("b", "skipped", 1),
                log("c", "success", 72),
            ],
            NOW,
        );
        for s in &summary.sources {
            assert!(
                SOURCE_HEALTHS.contains(&s.health.as_str()),
                "unknown health {}",
                s.health
            );
        }
    }
}
