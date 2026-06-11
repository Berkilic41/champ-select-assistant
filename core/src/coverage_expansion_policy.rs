//! Data Coverage Expansion Policy Core v1 (Sprint, Claude) — engine-pure.
//!
//! Decides *which coverage frontier to grow first* — region/patch/role(/champion)
//! dimensions ranked by how far below their sample target they are. Pure: no DB /
//! network / command / UI. No fabrication: no frontier / no deficit → no targets
//! (it never invents coverage). PII-free: player contribution is anonymous sample
//! counts only. Decision/health names are stable machine keys.
#![allow(dead_code)] // consumed by the Riot/fetch-history/scheduler wiring (Codex, later)

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Below this total current sample count the data is too `thin` to expand reliably.
const THIN_TOTAL_SAMPLES: u32 = 200;
/// A single player contributing more than this share of samples is an over-load risk.
const SINGLE_PLAYER_MAX_SHARE: f32 = 0.70;

pub const RISK_FACTORS: [&str; 4] = [
    "champ_select_active",
    "single_player_overload",
    "thin_data",
    "no_open_frontier",
];
pub const RISK_LEVELS: [&str; 3] = ["low", "medium", "high"];
pub const DATA_STATES: [&str; 3] = ["rich", "thin", "insufficient"];

// ── Input (caller-built; Rust-only) ─────────────────────────────────────────────

/// One region/patch/role(/champion) dimension's current sample state.
#[derive(Debug, Clone)]
pub struct FrontierSample {
    pub region: String,
    pub patch: String,
    pub role: String,
    pub champion_id: Option<u32>,
    pub current_samples: u32,
    pub target_samples: u32,
}

#[derive(Debug, Clone)]
pub struct CoverageExpansionInput {
    pub champ_select_active: bool,
    pub frontiers: Vec<FrontierSample>,
    /// Anonymous per-player sample counts (no ids → no PII) for over-load detection.
    pub player_sample_counts: Vec<u32>,
    /// Cap the number of prioritized targets.
    pub max_targets: u32,
}

// ── Output DTOs (ts-rs; no i64 → no bigint) ─────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct CoverageFrontier {
    pub region: String,
    pub patch: String,
    pub role: String,
    pub champion_id: Option<u32>,
    pub current_samples: u32,
    pub target_samples: u32,
    pub deficit: u32,
    pub coverage_ratio: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct CoverageTarget {
    pub frontier: CoverageFrontier,
    pub priority: u32,
    pub needed_samples: u32,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct ExpansionRisk {
    pub level: String,
    pub factors: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct CoverageExpansionPlan {
    pub data_state: String,
    pub targets: Vec<CoverageTarget>,
    pub risk: ExpansionRisk,
    pub total_deficit: u32,
    pub frontier_count: u32,
}

fn priority_band(coverage_ratio: f32) -> u32 {
    if coverage_ratio < 0.25 {
        4
    } else if coverage_ratio < 0.50 {
        3
    } else if coverage_ratio < 0.75 {
        2
    } else {
        1
    }
}

fn enrich(f: &FrontierSample) -> CoverageFrontier {
    let deficit = f.target_samples.saturating_sub(f.current_samples);
    let coverage_ratio = if f.target_samples == 0 {
        1.0
    } else {
        (f.current_samples as f32 / f.target_samples as f32).min(1.0)
    };
    CoverageFrontier {
        region: f.region.clone(),
        patch: f.patch.clone(),
        role: f.role.clone(),
        champion_id: f.champion_id,
        current_samples: f.current_samples,
        target_samples: f.target_samples,
        deficit,
        coverage_ratio,
    }
}

/// Rank coverage frontiers for expansion. Pure + deterministic.
pub fn plan_coverage_expansion(input: &CoverageExpansionInput) -> CoverageExpansionPlan {
    let frontiers: Vec<CoverageFrontier> = input.frontiers.iter().map(enrich).collect();
    let frontier_count = frontiers.len() as u32;
    let total_current: u32 = frontiers.iter().map(|f| f.current_samples).sum();
    let total_deficit: u32 = frontiers.iter().map(|f| f.deficit).sum();

    let data_state = if frontiers.is_empty() || total_current == 0 {
        "insufficient"
    } else if total_current < THIN_TOTAL_SAMPLES {
        "thin"
    } else {
        "rich"
    };

    // Targets = frontiers below target. Lowest coverage first (low-sample priority).
    let mut targets: Vec<CoverageTarget> = frontiers
        .iter()
        .filter(|f| f.deficit > 0)
        .map(|f| {
            let champ = f
                .champion_id
                .map(|id| format!(" · champ {id}"))
                .unwrap_or_default();
            CoverageTarget {
                priority: priority_band(f.coverage_ratio),
                needed_samples: f.deficit,
                rationale: format!(
                    "{}/{} {} {}/{} örneklem{}; {} eksik.",
                    f.region,
                    f.patch,
                    f.role,
                    f.current_samples,
                    f.target_samples,
                    champ,
                    f.deficit
                ),
                frontier: f.clone(),
            }
        })
        .collect();
    targets.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then(b.frontier.deficit.cmp(&a.frontier.deficit))
            .then(a.frontier.region.cmp(&b.frontier.region))
            .then(a.frontier.patch.cmp(&b.frontier.patch))
            .then(a.frontier.role.cmp(&b.frontier.role))
            .then(a.frontier.champion_id.cmp(&b.frontier.champion_id))
    });
    if input.max_targets > 0 {
        targets.truncate(input.max_targets as usize);
    }

    let risk = build_risk(input, data_state, targets.is_empty(), frontier_count);

    CoverageExpansionPlan {
        data_state: data_state.to_string(),
        targets,
        risk,
        total_deficit,
        frontier_count,
    }
}

fn build_risk(
    input: &CoverageExpansionInput,
    data_state: &str,
    targets_empty: bool,
    frontier_count: u32,
) -> ExpansionRisk {
    let mut factors = Vec::new();
    if input.champ_select_active {
        factors.push("champ_select_active".to_string());
    }
    let total_player: u32 = input.player_sample_counts.iter().sum();
    if total_player > 0 {
        let max = input
            .player_sample_counts
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        if max as f32 / total_player as f32 > SINGLE_PLAYER_MAX_SHARE {
            factors.push("single_player_overload".to_string());
        }
    }
    if data_state == "thin" {
        factors.push("thin_data".to_string());
    }
    if targets_empty && frontier_count > 0 {
        factors.push("no_open_frontier".to_string());
    }

    let severe = factors
        .iter()
        .any(|f| f == "champ_select_active" || f == "single_player_overload");
    let level = if severe {
        "high"
    } else if !factors.is_empty() {
        "medium"
    } else {
        "low"
    };
    let summary = if factors.is_empty() {
        "Belirgin genişletme riski yok.".to_string()
    } else {
        format!("Dikkat: {}.", factors.join(", "))
    };
    ExpansionRisk {
        level: level.to_string(),
        factors,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fs(
        region: &str,
        patch: &str,
        role: &str,
        champ: Option<u32>,
        cur: u32,
        target: u32,
    ) -> FrontierSample {
        FrontierSample {
            region: region.into(),
            patch: patch.into(),
            role: role.into(),
            champion_id: champ,
            current_samples: cur,
            target_samples: target,
        }
    }

    fn input(
        champ: bool,
        frontiers: Vec<FrontierSample>,
        players: Vec<u32>,
        max: u32,
    ) -> CoverageExpansionInput {
        CoverageExpansionInput {
            champ_select_active: champ,
            frontiers,
            player_sample_counts: players,
            max_targets: max,
        }
    }

    #[test]
    fn lowest_coverage_frontier_is_top_priority() {
        let plan = plan_coverage_expansion(&input(
            false,
            vec![
                fs("euw1", "14.11", "top", None, 750, 1000), // ratio 0.75 → band 1
                fs("euw1", "14.11", "middle", None, 100, 1000), // ratio 0.10 → band 4
                fs("euw1", "14.11", "jungle", None, 400, 1000), // ratio 0.40 → band 3
            ],
            vec![100, 100, 100],
            10,
        ));
        assert_eq!(
            plan.targets[0].frontier.role, "middle",
            "lowest coverage first"
        );
        assert_eq!(plan.targets[0].priority, 4);
        assert!(plan.targets[0].priority >= plan.targets[1].priority);
    }

    #[test]
    fn met_frontiers_yield_no_targets() {
        let plan = plan_coverage_expansion(&input(
            false,
            vec![fs("euw1", "14.11", "top", None, 1000, 1000)],
            vec![1000],
            10,
        ));
        assert!(plan.targets.is_empty());
        assert!(plan.risk.factors.contains(&"no_open_frontier".to_string()));
    }

    #[test]
    fn no_frontiers_is_insufficient_no_fabrication() {
        let plan = plan_coverage_expansion(&input(false, vec![], vec![], 10));
        assert_eq!(plan.data_state, "insufficient");
        assert!(plan.targets.is_empty());
    }

    #[test]
    fn champ_select_is_a_high_risk_but_plan_still_computed() {
        let plan = plan_coverage_expansion(&input(
            true,
            vec![fs("euw1", "14.11", "middle", None, 100, 1000)],
            vec![100],
            10,
        ));
        assert!(plan
            .risk
            .factors
            .contains(&"champ_select_active".to_string()));
        assert_eq!(plan.risk.level, "high");
        // The strategic plan is still visible (execution deferred at runtime).
        assert_eq!(plan.targets.len(), 1);
    }

    #[test]
    fn single_player_overload_is_flagged() {
        let plan = plan_coverage_expansion(&input(
            false,
            vec![fs("euw1", "14.11", "middle", None, 300, 1000)],
            vec![900, 50, 50], // one player = 90% of samples
            10,
        ));
        assert!(plan
            .risk
            .factors
            .contains(&"single_player_overload".to_string()));
        assert_eq!(plan.risk.level, "high");
    }

    #[test]
    fn thin_data_is_flagged() {
        let plan = plan_coverage_expansion(&input(
            false,
            vec![fs("euw1", "14.11", "middle", None, 50, 1000)], // total 50 < 200
            vec![10, 20, 20],
            10,
        ));
        assert_eq!(plan.data_state, "thin");
        assert!(plan.risk.factors.contains(&"thin_data".to_string()));
    }

    #[test]
    fn max_targets_caps_the_plan() {
        let frontiers: Vec<FrontierSample> = (0..5)
            .map(|i| fs("euw1", "14.11", "middle", Some(i), 100, 1000))
            .collect();
        let plan = plan_coverage_expansion(&input(false, frontiers, vec![500], 2));
        assert_eq!(plan.targets.len(), 2);
    }

    #[test]
    fn output_is_deterministic() {
        let i = input(
            false,
            vec![
                fs("kr", "14.11", "top", None, 100, 1000),
                fs("euw1", "14.11", "top", None, 100, 1000),
            ],
            vec![200],
            10,
        );
        assert_eq!(plan_coverage_expansion(&i), plan_coverage_expansion(&i));
        // Same priority/deficit → region asc tie-break (euw1 before kr).
        let plan = plan_coverage_expansion(&i);
        assert_eq!(plan.targets[0].frontier.region, "euw1");
    }

    #[test]
    fn total_deficit_and_counts_are_correct() {
        let plan = plan_coverage_expansion(&input(
            false,
            vec![
                fs("euw1", "14.11", "top", None, 600, 1000), // deficit 400
                fs("euw1", "14.11", "middle", None, 900, 1000), // deficit 100
            ],
            vec![1500],
            10,
        ));
        assert_eq!(plan.total_deficit, 500);
        assert_eq!(plan.frontier_count, 2);
        assert_eq!(plan.data_state, "rich");
    }

    #[test]
    fn emitted_tokens_stay_in_vocabulary() {
        assert_eq!(RISK_FACTORS.len(), 4);
        assert_eq!(RISK_LEVELS.len(), 3);
        assert_eq!(DATA_STATES.len(), 3);
        let plan = plan_coverage_expansion(&input(
            true,
            vec![fs("euw1", "14.11", "middle", None, 30, 1000)],
            vec![900, 100],
            10,
        ));
        assert!(DATA_STATES.contains(&plan.data_state.as_str()));
        assert!(RISK_LEVELS.contains(&plan.risk.level.as_str()));
        for f in &plan.risk.factors {
            assert!(
                RISK_FACTORS.contains(&f.as_str()),
                "unknown risk factor {f}"
            );
        }
    }

    /// Cross-language drift guard: every risk factor / level / data_state token must
    /// have a `dataPipeline.coverageExpansion.*` label in tr.json (the plan/risk UI
    /// renders them). en parity is covered by the TS `i18n tr/en parity` test. A new
    /// token without i18n turns this red.
    #[test]
    fn every_emitted_expansion_token_has_an_i18n_label() {
        const TR: &str = include_str!("../../src/i18n/tr.json");
        let tr: serde_json::Value = serde_json::from_str(TR).expect("tr.json parses");
        let ce = &tr["dataPipeline"]["coverageExpansion"];
        for f in RISK_FACTORS {
            assert!(
                !ce["risk"][f].is_null(),
                "risk factor '{f}' has no i18n label"
            );
        }
        for l in RISK_LEVELS {
            assert!(!ce["level"][l].is_null(), "level '{l}' has no i18n label");
        }
        for s in DATA_STATES {
            assert!(
                !ce["dataState"][s].is_null(),
                "dataState '{s}' has no i18n label"
            );
        }
    }
}
