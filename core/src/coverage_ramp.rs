//! Live Data Coverage Ramp v1 (Sprint, Claude) — engine-pure.
//!
//! Interprets a **before → after** pair of pipeline table snapshots from one (or
//! several) ingestion runs into a structured ramp verdict: did coverage actually
//! grow, is the discovery → fetch → process funnel advancing or stuck, and where.
//! Complements `data_pipeline_quality` (point-in-time health) by measuring *motion*.
//! Pure: no DB / network / Riot / command / UI. The command layer (Codex) reads the
//! row counts before + after a live run and feeds them here; the thresholds below are
//! the regression-guard surface we tighten once real numbers land.
//!
//! No fabrication: a deferred run (champ-select / no budget) is `no_budget`, never a
//! fake "progressing"; identical snapshots are `no_activity`. State / funnel-stage /
//! observation names are stable machine keys (UI i18n-maps); prose is TR.
#![allow(dead_code)] // public DTOs + engine consumed by a measurement command later

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const RAMP_STATES: [&str; 5] = [
    "progressing",
    "stalled",
    "regressed",
    "no_activity",
    "no_budget",
];
pub const FUNNEL_STAGES: [&str; 3] = ["discovery", "fetch", "process"];
pub const RAMP_OBSERVATIONS: [&str; 10] = [
    "coverage_growing",
    "coverage_flat",
    "coverage_regressed",
    "new_matches_discovered",
    "no_new_matches",
    "fetch_backlog_growing",
    "processing_advancing",
    "processing_below_expected",
    "high_failure_rate",
    "champ_select_deferred",
];

/// User-facing data trajectory = fusion of point-in-time quality status + ramp motion.
/// `warming_up` = below the quality line (degraded/stale) yet climbing; `enriching` =
/// already healthy but still collecting toward the richer scheduler target (1000 quality
/// line vs 5000 collection target — two distinct numbers, see audit §14).
pub const DATA_TRAJECTORIES: [&str; 6] = [
    "healthy",
    "enriching",
    "warming_up",
    "stagnant",
    "regressing",
    "deferred",
];

/// Failure ratio (failed / total known) above which the run is flagged.
/// CONFIRMED across 4 live runs (2026-06-07): real failure_ratio was 0.0 every run, so
/// 0.25 is kept untouched until a genuine failure floor appears (no evidence to tighten).
const HIGH_FAILURE_RATIO: f32 = 0.25;

/// Grounded steady-state processed-matches per single refresh — 3 consecutive live runs
/// (2026-06-07) held +6/run at batch_limit=6 with failure_ratio 0.0. PROVISIONAL baseline.
pub const GROUNDED_PROCESSED_PER_RUN: u32 = 6;

/// Floor below which a single refresh that still has a backlog is flagged
/// `processing_below_expected` ("moving but sluggish", not a hard stall). Half the
/// grounded rate so a one-short run (5/6) is not noise; ≤2/6 with work queued is. PROVISIONAL.
const MIN_HEALTHY_PROCESSED_PER_RUN: i32 = (GROUNDED_PROCESSED_PER_RUN / 2) as i32;

// ── Inputs (caller-built; Rust-only — `taken_at` carried as i64) ─────────────────

/// One point-in-time read of the pipeline's persisted state.
#[derive(Debug, Clone)]
pub struct RampSnapshot {
    pub taken_at: i64,
    pub champion_rate_rows: u32,
    pub matchup_rows: u32,
    pub build_rows: u32,
    /// `match_v5_fetch_history` status='discovered' (pending fetch).
    pub discovered_matches: u32,
    /// status='fetched' | 'parsed'.
    pub fetched_matches: u32,
    /// status='processed'.
    pub processed_matches: u32,
    /// status='failed'.
    pub failed_matches: u32,
    /// `match_discovery_players` row count.
    pub crawled_players: u32,
}

#[derive(Debug, Clone)]
pub struct CoverageRampInput {
    pub before: RampSnapshot,
    pub after: RampSnapshot,
    /// The run was (or would be) deferred — champ-select hard rule.
    pub champ_select_active: bool,
    /// 0 → the run had no crawl budget (rate-limited / disabled).
    pub crawl_budget: u32,
}

// ── Outputs (ts-rs; deltas as i32 → number, no bigint) ───────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct CoverageDeltas {
    pub champion_rate_delta: i32,
    pub matchup_delta: i32,
    pub build_delta: i32,
    pub discovered_delta: i32,
    pub processed_delta: i32,
    pub crawled_player_delta: i32,
    pub elapsed_secs: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct DiscoveryFunnel {
    /// Backlog: matches discovered but not yet fetched (after-state).
    pub pending: u32,
    pub fetched: u32,
    pub processed: u32,
    pub failed: u32,
    /// processed / total_known (after); 0 when nothing known.
    pub process_ratio: f32,
    /// failed / total_known (after); 0 when nothing known.
    pub failure_ratio: f32,
    /// Where the funnel is stuck (`FUNNEL_STAGES`), or null when advancing.
    pub stalled_stage: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct CoverageRampReport {
    pub ramp_state: String,
    pub deltas: CoverageDeltas,
    pub funnel: DiscoveryFunnel,
    pub observations: Vec<String>,
    pub summary: String,
    pub data_growing: bool,
}

fn total_known(s: &RampSnapshot) -> u32 {
    s.discovered_matches
        .saturating_add(s.fetched_matches)
        .saturating_add(s.processed_matches)
        .saturating_add(s.failed_matches)
}

fn delta(after: u32, before: u32) -> i32 {
    after as i32 - before as i32
}

/// Evaluate one before → after ramp. Pure + deterministic.
pub fn evaluate_coverage_ramp(input: &CoverageRampInput) -> CoverageRampReport {
    let b = &input.before;
    let a = &input.after;

    let deltas = CoverageDeltas {
        champion_rate_delta: delta(a.champion_rate_rows, b.champion_rate_rows),
        matchup_delta: delta(a.matchup_rows, b.matchup_rows),
        build_delta: delta(a.build_rows, b.build_rows),
        discovered_delta: delta(a.discovered_matches, b.discovered_matches),
        processed_delta: delta(a.processed_matches, b.processed_matches),
        crawled_player_delta: delta(a.crawled_players, b.crawled_players),
        elapsed_secs: (a.taken_at - b.taken_at).max(0) as u32,
    };

    let known_after = total_known(a);
    let known_delta = delta(known_after, total_known(b));
    let (process_ratio, failure_ratio) = if known_after == 0 {
        (0.0, 0.0)
    } else {
        (
            a.processed_matches as f32 / known_after as f32,
            a.failed_matches as f32 / known_after as f32,
        )
    };
    let backlog_growing =
        a.discovered_matches > b.discovered_matches && deltas.processed_delta == 0;

    let deferred = input.champ_select_active || input.crawl_budget == 0;
    // Coverage aggregation only ever upserts forward; a drop means data loss.
    let coverage_regressed = deltas.champion_rate_delta < 0
        || deltas.matchup_delta < 0
        || deltas.build_delta < 0
        || deltas.processed_delta < 0;
    let coverage_grew =
        deltas.champion_rate_delta > 0 || deltas.matchup_delta > 0 || deltas.build_delta > 0;
    let advanced = coverage_grew || deltas.processed_delta > 0;

    // ── Ramp state (priority: deferred → regressed → progressing → stalled → idle) ─
    let ramp_state = if deferred && !advanced && !coverage_regressed {
        "no_budget"
    } else if coverage_regressed {
        "regressed"
    } else if advanced {
        "progressing"
    } else if known_delta > 0 || a.discovered_matches > 0 {
        // New candidates discovered or a backlog exists, yet nothing moved forward.
        "stalled"
    } else {
        "no_activity"
    };

    // ── Funnel stall localization ────────────────────────────────────────────────
    let stalled_stage = if ramp_state == "stalled" {
        if a.discovered_matches > 0 && deltas.processed_delta == 0 && a.fetched_matches == 0 {
            Some("fetch".to_string())
        } else if a.fetched_matches > 0 && deltas.processed_delta == 0 {
            Some("process".to_string())
        } else {
            Some("discovery".to_string())
        }
    } else {
        None
    };

    // ── Observations (machine tokens; deduped + sorted) ──────────────────────────
    let mut observations: Vec<String> = Vec::new();
    if deferred {
        observations.push("champ_select_deferred".to_string());
    }
    if coverage_regressed {
        observations.push("coverage_regressed".to_string());
    } else if coverage_grew {
        observations.push("coverage_growing".to_string());
    } else if !deferred {
        observations.push("coverage_flat".to_string());
    }
    if known_delta > 0 {
        observations.push("new_matches_discovered".to_string());
    } else if !deferred && known_delta == 0 {
        observations.push("no_new_matches".to_string());
    }
    if deltas.processed_delta > 0 {
        observations.push("processing_advancing".to_string());
    }
    // Moving, but below the grounded healthy rate while a backlog is queued — a soft
    // partial-stall watch (distinct from a full stall, which has processed_delta == 0).
    if !deferred
        && a.discovered_matches > 0
        && deltas.processed_delta > 0
        && deltas.processed_delta < MIN_HEALTHY_PROCESSED_PER_RUN
    {
        observations.push("processing_below_expected".to_string());
    }
    if backlog_growing {
        observations.push("fetch_backlog_growing".to_string());
    }
    if failure_ratio >= HIGH_FAILURE_RATIO {
        observations.push("high_failure_rate".to_string());
    }
    observations.sort();
    observations.dedup();

    let summary = match ramp_state {
        "progressing" => "Veri büyüyor; keşif → fetch → işleme zinciri ilerliyor.",
        "stalled" => "Keşif var ama funnel takıldı; fetch/işleme ilerlemiyor.",
        "regressed" => "Veri geriledi (satır kaybı); son-iyi cache / rollback incelenmeli.",
        "no_activity" => "Bu turda değişiklik yok (yeni keşif/işleme olmadı).",
        _ => "Çalıştırma ertelendi (champ-select / bütçe yok); ölçüm yapılmadı.",
    }
    .to_string();

    CoverageRampReport {
        ramp_state: ramp_state.to_string(),
        deltas,
        funnel: DiscoveryFunnel {
            pending: a.discovered_matches,
            fetched: a.fetched_matches,
            processed: a.processed_matches,
            failed: a.failed_matches,
            process_ratio,
            failure_ratio,
            stalled_stage,
        },
        observations,
        summary,
        data_growing: coverage_grew,
    }
}

/// Fuse `data_pipeline_quality` status with the ramp verdict into one honest,
/// user-facing trajectory token (`DATA_TRAJECTORIES`). The first live run showed the
/// gap this closes: quality stays `degraded`/`stale` for the whole early ramp (below
/// 1000 matchups), which alone reads "stuck" — yet the data is climbing. Priority:
/// data loss (`regressing`) and deferral (`deferred`) override; then `healthy` if at
/// target; then `warming_up` if motion is forward; else `stagnant` (below target, not
/// moving). Pure: no fabrication — it only relabels two existing verdicts.
pub fn classify_data_trajectory(quality_status: &str, ramp_state: &str) -> String {
    let token = if ramp_state == "regressed" {
        "regressing"
    } else if ramp_state == "no_budget" {
        "deferred"
    } else if quality_status == "healthy" {
        // At the quality line; still climbing toward the richer collection target → enriching.
        if ramp_state == "progressing" {
            "enriching"
        } else {
            "healthy"
        }
    } else if ramp_state == "progressing" {
        "warming_up"
    } else {
        "stagnant"
    };
    token.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(
        taken_at: i64,
        rates: u32,
        matchups: u32,
        builds: u32,
        discovered: u32,
        fetched: u32,
        processed: u32,
        failed: u32,
        players: u32,
    ) -> RampSnapshot {
        RampSnapshot {
            taken_at,
            champion_rate_rows: rates,
            matchup_rows: matchups,
            build_rows: builds,
            discovered_matches: discovered,
            fetched_matches: fetched,
            processed_matches: processed,
            failed_matches: failed,
            crawled_players: players,
        }
    }

    fn input(before: RampSnapshot, after: RampSnapshot) -> CoverageRampInput {
        CoverageRampInput {
            before,
            after,
            champ_select_active: false,
            crawl_budget: 6,
        }
    }

    #[test]
    fn progressing_when_coverage_and_processing_grow() {
        let r = evaluate_coverage_ramp(&input(
            snap(0, 100, 200, 50, 10, 0, 0, 0, 5),
            snap(60, 120, 240, 55, 4, 0, 20, 0, 9),
        ));
        assert_eq!(r.ramp_state, "progressing");
        assert!(r.data_growing);
        assert!(r.observations.contains(&"coverage_growing".to_string()));
        assert!(r.observations.contains(&"processing_advancing".to_string()));
        assert!(r.funnel.stalled_stage.is_none());
        assert_eq!(r.deltas.processed_delta, 20);
        assert_eq!(r.deltas.elapsed_secs, 60);
    }

    #[test]
    fn consumed_backlog_is_not_a_regression() {
        // discovered drops (consumed) while processed rises → progressing, not regressed.
        let r = evaluate_coverage_ramp(&input(
            snap(0, 100, 200, 50, 30, 0, 0, 0, 5),
            snap(60, 100, 200, 50, 5, 0, 25, 0, 5),
        ));
        assert_eq!(r.ramp_state, "progressing");
        assert_eq!(r.deltas.discovered_delta, -25);
        assert!(r.deltas.discovered_delta < 0);
    }

    #[test]
    fn regressed_when_processed_drops() {
        let r = evaluate_coverage_ramp(&input(
            snap(0, 100, 200, 50, 0, 0, 40, 0, 5),
            snap(60, 100, 200, 50, 0, 0, 30, 0, 5),
        ));
        assert_eq!(r.ramp_state, "regressed");
        assert!(r.observations.contains(&"coverage_regressed".to_string()));
    }

    #[test]
    fn regressed_when_matchups_drop() {
        let r = evaluate_coverage_ramp(&input(
            snap(0, 100, 200, 50, 0, 0, 0, 0, 5),
            snap(60, 100, 180, 50, 0, 0, 0, 0, 5),
        ));
        assert_eq!(r.ramp_state, "regressed");
    }

    #[test]
    fn stalled_at_fetch_when_discovered_but_none_fetched() {
        let r = evaluate_coverage_ramp(&input(
            snap(0, 100, 200, 50, 5, 0, 0, 0, 5),
            snap(60, 100, 200, 50, 25, 0, 0, 0, 12),
        ));
        assert_eq!(r.ramp_state, "stalled");
        assert_eq!(r.funnel.stalled_stage.as_deref(), Some("fetch"));
        assert!(r
            .observations
            .contains(&"fetch_backlog_growing".to_string()));
        assert!(r
            .observations
            .contains(&"new_matches_discovered".to_string()));
    }

    #[test]
    fn stalled_at_process_when_fetched_but_not_processed() {
        let r = evaluate_coverage_ramp(&input(
            snap(0, 100, 200, 50, 0, 5, 0, 0, 5),
            snap(60, 100, 200, 50, 0, 20, 0, 0, 5),
        ));
        assert_eq!(r.ramp_state, "stalled");
        assert_eq!(r.funnel.stalled_stage.as_deref(), Some("process"));
    }

    #[test]
    fn no_activity_when_nothing_changes() {
        let r = evaluate_coverage_ramp(&input(
            snap(0, 100, 200, 50, 0, 0, 40, 0, 5),
            snap(60, 100, 200, 50, 0, 0, 40, 0, 5),
        ));
        assert_eq!(r.ramp_state, "no_activity");
        assert!(r.observations.contains(&"no_new_matches".to_string()));
        assert!(r.observations.contains(&"coverage_flat".to_string()));
    }

    #[test]
    fn no_budget_when_champ_select_active() {
        let mut inp = input(
            snap(0, 100, 200, 50, 0, 0, 40, 0, 5),
            snap(60, 100, 200, 50, 0, 0, 40, 0, 5),
        );
        inp.champ_select_active = true;
        let r = evaluate_coverage_ramp(&inp);
        assert_eq!(r.ramp_state, "no_budget");
        assert!(r
            .observations
            .contains(&"champ_select_deferred".to_string()));
        // Deferred runs do not assert flat/no-new (we did not measure).
        assert!(!r.observations.contains(&"coverage_flat".to_string()));
        assert!(!r.observations.contains(&"no_new_matches".to_string()));
    }

    #[test]
    fn no_budget_when_crawl_budget_zero() {
        let mut inp = input(
            snap(0, 100, 200, 50, 3, 0, 0, 0, 5),
            snap(60, 100, 200, 50, 3, 0, 0, 0, 5),
        );
        inp.crawl_budget = 0;
        let r = evaluate_coverage_ramp(&inp);
        assert_eq!(r.ramp_state, "no_budget");
    }

    #[test]
    fn high_failure_rate_flagged() {
        let r = evaluate_coverage_ramp(&input(
            snap(0, 100, 200, 50, 0, 0, 0, 0, 5),
            snap(60, 105, 200, 50, 0, 0, 6, 4, 5),
        ));
        // 4 failed / 10 known = 0.4 ≥ 0.25.
        assert!(r.observations.contains(&"high_failure_rate".to_string()));
        assert!((r.funnel.failure_ratio - 0.4).abs() < 1e-6);
    }

    #[test]
    fn elapsed_clamps_when_after_before_clock_skew() {
        let r = evaluate_coverage_ramp(&input(
            snap(100, 100, 200, 50, 0, 0, 0, 0, 5),
            snap(40, 100, 200, 50, 0, 0, 0, 0, 5),
        ));
        assert_eq!(r.deltas.elapsed_secs, 0);
    }

    #[test]
    fn observations_are_sorted_and_deduped() {
        let r = evaluate_coverage_ramp(&input(
            snap(0, 100, 200, 50, 5, 0, 0, 0, 5),
            snap(60, 120, 240, 55, 25, 0, 20, 0, 9),
        ));
        let mut sorted = r.observations.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(r.observations, sorted);
    }

    #[test]
    fn ramp_state_and_tokens_stay_in_vocabulary() {
        // Drive a spread of inputs; every emitted token must be locked.
        let cases = vec![
            evaluate_coverage_ramp(&input(
                snap(0, 100, 200, 50, 10, 0, 0, 0, 5),
                snap(60, 120, 240, 55, 4, 0, 20, 0, 9),
            )),
            evaluate_coverage_ramp(&input(
                snap(0, 100, 200, 50, 0, 0, 40, 0, 5),
                snap(60, 100, 180, 50, 0, 0, 30, 0, 5),
            )),
            evaluate_coverage_ramp(&input(
                snap(0, 100, 200, 50, 5, 0, 0, 0, 5),
                snap(60, 100, 200, 50, 25, 0, 0, 0, 12),
            )),
            evaluate_coverage_ramp(&input(
                snap(0, 100, 200, 50, 0, 0, 40, 0, 5),
                snap(60, 100, 200, 50, 0, 0, 40, 0, 5),
            )),
        ];
        for r in &cases {
            assert!(
                RAMP_STATES.contains(&r.ramp_state.as_str()),
                "state {}",
                r.ramp_state
            );
            for o in &r.observations {
                assert!(RAMP_OBSERVATIONS.contains(&o.as_str()), "obs {o}");
            }
            if let Some(stage) = &r.funnel.stalled_stage {
                assert!(FUNNEL_STAGES.contains(&stage.as_str()), "stage {stage}");
            }
        }
    }

    /// Complementary-contract guard with `data_pipeline_quality`. The two engines look
    /// at the same early-ramp state through different lenses: quality (absolute target)
    /// stays `degraded` while matchups are below 1000 — which alone reads "stuck" — but
    /// ramp (motion) says `progressing`. The pairing is the actionable truth ("warming
    /// up well"). This locks the design so a future calibration change can't let the two
    /// contradict (e.g. ramp `progressing` while quality drops to `insufficient`).
    #[test]
    fn complements_pipeline_quality_below_target_but_growing() {
        use crate::data_pipeline_quality::{
            evaluate_pipeline_quality, PipelineQualityInput, PipelineSource,
        };
        let now = 1_000_000_i64 * 3600;
        let quality = evaluate_pipeline_quality(&PipelineQualityInput {
            now,
            current_patch: "14.11".into(),
            data_patch: Some("14.11".into()),
            target_champions: 172,
            champion_rate_count: 160,  // ~0.93 rate coverage
            matchup_count: 300,        // < 1000 → coverage_low floor
            build_champion_count: 165, // ~0.96 build coverage
            meta_role_count: 160,
            sources: vec![PipelineSource {
                source: "match_v5".into(),
                updated_at: now - 3600, // 1h old → fresh
                risk_level: "low".into(),
            }],
            fallback_available: true,
            last_good_cache_available: true,
        });
        // Honest: genuinely below the matchup target, so not "healthy".
        assert_eq!(quality.status, "degraded");

        let ramp = evaluate_coverage_ramp(&input(
            snap(0, 150, 80, 160, 5, 0, 0, 0, 5),
            snap(3600, 160, 300, 165, 4, 0, 30, 0, 9),
        ));
        // Motion: the same data is climbing toward that target.
        assert_eq!(ramp.ramp_state, "progressing");
        assert!(ramp.data_growing);
    }

    /// A stale-patch ramp still pairs with `progressing` (the first live run's `after`
    /// status was `stale`, not `degraded` — patch mismatch). Locks that "growing" is
    /// coherent with `stale` too, so `classify_data_trajectory` → `warming_up`.
    #[test]
    fn stale_quality_with_growing_ramp_is_warming_up() {
        use crate::data_pipeline_quality::{
            evaluate_pipeline_quality, PipelineQualityInput, PipelineSource,
        };
        let now = 1_000_000_i64 * 3600;
        let quality = evaluate_pipeline_quality(&PipelineQualityInput {
            now,
            current_patch: "14.12".into(),
            data_patch: Some("14.11".into()), // old patch → stale
            target_champions: 172,
            champion_rate_count: 49,
            matchup_count: 140,
            build_champion_count: 80,
            meta_role_count: 49,
            sources: vec![PipelineSource {
                source: "match_v5".into(),
                updated_at: now - 3600,
                risk_level: "low".into(),
            }],
            fallback_available: true,
            last_good_cache_available: true,
        });
        assert_eq!(quality.status, "stale");

        let ramp = evaluate_coverage_ramp(&input(
            snap(0, 0, 80, 31, 0, 0, 0, 0, 0),
            snap(3600, 49, 140, 80, 24, 0, 6, 0, 52),
        ));
        assert_eq!(ramp.ramp_state, "progressing");
        assert_eq!(
            classify_data_trajectory(&quality.status, &ramp.ramp_state),
            "warming_up"
        );
    }

    /// Regression anchor: the exact first live run (2026-06-07, key resolved via
    /// Account-V1). If the engine ever stops reproducing this measured verdict, this
    /// turns red. before/after are the runtime-reported counts verbatim.
    #[test]
    fn reproduces_first_live_ramp_2026_06_07() {
        let r = evaluate_coverage_ramp(&input(
            snap(0, 0, 80, 31, 0, 0, 0, 0, 0),
            snap(3600, 49, 140, 80, 24, 0, 6, 0, 52),
        ));
        assert_eq!(r.ramp_state, "progressing");
        assert_eq!(r.deltas.champion_rate_delta, 49);
        assert_eq!(r.deltas.matchup_delta, 60);
        assert_eq!(r.deltas.build_delta, 49);
        assert_eq!(r.deltas.discovered_delta, 24);
        assert_eq!(r.deltas.processed_delta, 6);
        assert_eq!(r.deltas.crawled_player_delta, 52);
        // process_ratio = 6 / (24 + 6) = 0.2; failure_ratio = 0.
        assert!((r.funnel.process_ratio - 0.2).abs() < 1e-6);
        assert_eq!(r.funnel.failure_ratio, 0.0);
        assert!(r.observations.contains(&"coverage_growing".to_string()));
        assert!(r
            .observations
            .contains(&"new_matches_discovered".to_string()));
        assert!(r.observations.contains(&"processing_advancing".to_string()));
        // No false alarms on a clean first tick.
        assert!(!r.observations.contains(&"high_failure_rate".to_string()));
        assert!(!r
            .observations
            .contains(&"fetch_backlog_growing".to_string()));
        assert!(r.data_growing);
    }

    /// Grounded steady-state run (2026-06-07, runs 2-4): a typical refresh on a mid-ramp
    /// DB adds ~+6 processed / +57 matchup / +24 build / +9 discovered / +50 crawled with
    /// 0 failures. Healthy processing → no `processing_below_expected`, no `high_failure_rate`;
    /// backlog grows but processing advances so no `fetch_backlog_growing`.
    #[test]
    fn steady_state_run_processes_healthily() {
        let r = evaluate_coverage_ramp(&input(
            snap(0, 139, 371, 185, 60, 0, 30, 0, 258),
            snap(3600, 169, 428, 209, 69, 0, 36, 0, 308),
        ));
        assert_eq!(r.ramp_state, "progressing");
        assert_eq!(r.deltas.processed_delta, 6);
        assert_eq!(r.deltas.matchup_delta, 57);
        assert!(!r
            .observations
            .contains(&"processing_below_expected".to_string()));
        assert!(!r.observations.contains(&"high_failure_rate".to_string()));
        assert!(!r
            .observations
            .contains(&"fetch_backlog_growing".to_string()));
        // process_ratio = 36 / (69 + 36) ≈ 0.343 — matches the live 0.34 band.
        assert!((r.funnel.process_ratio - 0.342857).abs() < 1e-4);
    }

    /// A single refresh that only inches the funnel forward (≤2/6) while matches are
    /// queued is flagged `processing_below_expected` — moving but sluggish, still progressing.
    #[test]
    fn sluggish_processing_with_backlog_flagged() {
        let r = evaluate_coverage_ramp(&input(
            snap(0, 100, 200, 50, 20, 0, 10, 0, 50),
            snap(3600, 100, 205, 50, 28, 0, 12, 0, 60),
        ));
        assert_eq!(r.ramp_state, "progressing");
        assert_eq!(r.deltas.processed_delta, 2);
        assert!(r
            .observations
            .contains(&"processing_below_expected".to_string()));
        assert!(r.observations.contains(&"processing_advancing".to_string()));
    }

    /// Post-1000 grounded state (2026-06-07, collection target raised 1000 → 5000): a
    /// steady run on top of a DB that has already crossed the quality matchup line
    /// (1454 matchups) keeps the same healthy +6-processed shape and 0 failures while
    /// climbing toward the richer 5000 target. Locks that crossing 1000 does not change
    /// the ramp verdict (still `progressing`) — the quality line and collection target
    /// are independent (see `classify_data_trajectory` → `enriching` once patch is fresh).
    #[test]
    fn post_1000_run_keeps_progressing_toward_5000() {
        let r = evaluate_coverage_ramp(&input(
            snap(0, 286, 1394, 467, 63, 0, 156, 0, 1364),
            snap(3600, 292, 1454, 473, 69, 0, 162, 0, 1414),
        ));
        assert_eq!(r.ramp_state, "progressing");
        assert_eq!(r.deltas.matchup_delta, 60);
        assert_eq!(r.deltas.processed_delta, 6);
        assert_eq!(r.funnel.failure_ratio, 0.0);
        assert!(!r
            .observations
            .contains(&"processing_below_expected".to_string()));
        assert!(!r.observations.contains(&"high_failure_rate".to_string()));
    }

    #[test]
    fn data_trajectory_truth_table() {
        // Data loss + deferral override quality.
        assert_eq!(
            classify_data_trajectory("degraded", "regressed"),
            "regressing"
        );
        assert_eq!(
            classify_data_trajectory("healthy", "regressed"),
            "regressing"
        );
        assert_eq!(
            classify_data_trajectory("degraded", "no_budget"),
            "deferred"
        );
        // At the quality line and idle → healthy; still growing toward 5000 → enriching.
        assert_eq!(
            classify_data_trajectory("healthy", "no_activity"),
            "healthy"
        );
        assert_eq!(classify_data_trajectory("healthy", "stalled"), "healthy");
        assert_eq!(
            classify_data_trajectory("healthy", "progressing"),
            "enriching"
        );
        // Below target but climbing → warming_up (the key signal).
        assert_eq!(
            classify_data_trajectory("degraded", "progressing"),
            "warming_up"
        );
        assert_eq!(
            classify_data_trajectory("stale", "progressing"),
            "warming_up"
        );
        assert_eq!(
            classify_data_trajectory("insufficient", "progressing"),
            "warming_up"
        );
        // Below target and not moving → stagnant.
        assert_eq!(classify_data_trajectory("degraded", "stalled"), "stagnant");
        assert_eq!(classify_data_trajectory("stale", "no_activity"), "stagnant");
        // Every output stays in the locked vocabulary.
        for q in ["healthy", "degraded", "stale", "insufficient"] {
            for r in RAMP_STATES {
                let t = classify_data_trajectory(q, r);
                assert!(DATA_TRAJECTORIES.contains(&t.as_str()), "trajectory {t}");
            }
        }
    }

    /// Drift guard: every trajectory token the UI badge can show must have a
    /// `dataPipeline.dataTrajectory.*` label (the `unknown` fallback included). Add a
    /// token without the i18n key → red. en parity is covered by `i18n-parity.test.ts`.
    #[test]
    fn every_emitted_trajectory_token_has_an_i18n_label() {
        const TR: &str = include_str!("../../src/i18n/tr.json");
        let tr: serde_json::Value = serde_json::from_str(TR).unwrap();
        let dt = &tr["dataPipeline"]["dataTrajectory"];
        for token in DATA_TRAJECTORIES {
            assert!(!dt[token].is_null(), "trajectory '{token}' i18n yok");
        }
        // `unknown` is a view-layer fallback (DataTrajectoryView) — also must be labelled.
        assert!(!dt["unknown"].is_null(), "trajectory 'unknown' i18n yok");
    }
}
