//! Data Pipeline Production v1 — Quality/Freshness Core (Sprint, Claude).
//!
//! Pure decision engine: given the pipeline's current coverage + source freshness,
//! it decides whether the data is `healthy / degraded / stale / insufficient`, with
//! the gaps and the concrete refresh/fallback actions to take. **Independent of the
//! backend ingestion** — no DB/network/Riot. The command layer (Codex, later) feeds
//! it counts from `/v1/data-quality` + `get_data_source_registry`, and binds the
//! actions to the actual fetchers/cache/scheduler.
//!
//! No fabricated data: when there's nothing to judge, status is `insufficient`
//! (not a fake "healthy"). Dimension/action/status names are stable machine keys
//! (UI i18n-maps); prose is TR.
#![allow(dead_code)] // public DTOs + engine consumed by a command/UI in a later turn

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// Shared targets (mirror the backend + client confidence bands).
const TARGET_CHAMPIONS: u32 = 172;
const HIGH_MATCHUPS: u32 = 1000;
const HIGH_BUILD_COVERAGE: f32 = 0.95;
const RATE_COVERAGE_MIN: f32 = 0.90;
const ROLE_COVERAGE_MIN: f32 = 0.80;
const SOURCE_STALE_HOURS: u32 = 48;

/// One contributing source's raw state (caller-supplied).
#[derive(Debug, Clone)]
pub struct PipelineSource {
    pub source: String,
    /// Last successful fetch, unix seconds.
    pub updated_at: i64,
    /// "low" | "medium" | "high".
    pub risk_level: String,
}

/// Evaluation request.
#[derive(Debug, Clone)]
pub struct PipelineQualityInput {
    pub now: i64,
    pub current_patch: String,
    pub data_patch: Option<String>,
    /// 0 → default 172.
    pub target_champions: u32,
    pub champion_rate_count: u32,
    pub matchup_count: u32,
    pub build_champion_count: u32,
    pub meta_role_count: u32,
    pub sources: Vec<PipelineSource>,
    pub fallback_available: bool,
    pub last_good_cache_available: bool,
}

/// Per-source freshness read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct SourceFreshness {
    pub source: String,
    pub age_hours: u32,
    pub stale: bool,
    pub risk_level: String,
}

/// Coverage + freshness snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct PipelineCoverage {
    pub champion_rate_coverage: f32,
    pub build_coverage: f32,
    pub matchup_coverage: f32,
    pub role_coverage: f32,
    pub patch_fresh: bool,
    pub sources_fresh: bool,
    pub fallback_available: bool,
    pub last_good_cache_available: bool,
    pub has_high_risk_source: bool,
    pub sources: Vec<SourceFreshness>,
}

/// A missing/weak dimension. `dimension`/`severity` are machine keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct DataGap {
    pub dimension: String,
    pub severity: String,
    pub note: String,
}

/// A concrete remediation. `action` is a machine key (refresh_rates, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct PipelineAction {
    pub action: String,
    pub reason: String,
}

/// Full pipeline quality read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct PipelineQualityReport {
    /// "healthy" | "degraded" | "stale" | "insufficient".
    pub status: String,
    /// "high" | "medium" | "low".
    pub confidence: String,
    pub coverage: PipelineCoverage,
    pub gaps: Vec<DataGap>,
    pub actions: Vec<PipelineAction>,
    pub summary: String,
}

fn frac(count: u32, target: u32) -> f32 {
    if target == 0 {
        0.0
    } else {
        (count as f32 / target as f32).min(1.0)
    }
}

fn gap(dimension: &str, severity: &str, note: &str) -> DataGap {
    DataGap {
        dimension: dimension.to_string(),
        severity: severity.to_string(),
        note: note.to_string(),
    }
}

/// Evaluate the pipeline's data quality + freshness. Pure + deterministic.
pub fn evaluate_pipeline_quality(input: &PipelineQualityInput) -> PipelineQualityReport {
    let target = if input.target_champions == 0 {
        TARGET_CHAMPIONS
    } else {
        input.target_champions
    };

    let rate_cov = frac(input.champion_rate_count, target);
    let build_cov = frac(input.build_champion_count, target);
    let matchup_cov = (input.matchup_count as f32 / HIGH_MATCHUPS as f32).min(1.0);
    let role_cov = frac(input.meta_role_count, target);
    let patch_fresh = input
        .data_patch
        .as_deref()
        .map(|p| p == input.current_patch)
        .unwrap_or(false);

    let sources: Vec<SourceFreshness> = input
        .sources
        .iter()
        .map(|s| {
            let age_hours = ((input.now - s.updated_at).max(0) / 3600) as u32;
            SourceFreshness {
                source: s.source.clone(),
                age_hours,
                stale: age_hours >= SOURCE_STALE_HOURS,
                risk_level: s.risk_level.clone(),
            }
        })
        .collect();
    let sources_fresh = !sources.is_empty() && sources.iter().all(|s| !s.stale);
    let any_source_stale = sources.iter().any(|s| s.stale);
    let has_high_risk = input.sources.iter().any(|s| s.risk_level == "high");
    let empty = input.champion_rate_count == 0
        && input.matchup_count == 0
        && input.build_champion_count == 0;

    let coverage = PipelineCoverage {
        champion_rate_coverage: rate_cov,
        build_coverage: build_cov,
        matchup_coverage: matchup_cov,
        role_coverage: role_cov,
        patch_fresh,
        sources_fresh,
        fallback_available: input.fallback_available,
        last_good_cache_available: input.last_good_cache_available,
        has_high_risk_source: has_high_risk,
        sources,
    };

    // ── Gaps + actions ──────────────────────────────────────────────────────────
    let mut gaps = Vec::new();
    let mut actions: Vec<PipelineAction> = Vec::new();
    let mut add_action = |action: &str, reason: &str| {
        if !actions.iter().any(|a| a.action == action) {
            actions.push(PipelineAction {
                action: action.to_string(),
                reason: reason.to_string(),
            });
        }
    };

    if empty {
        add_action(
            "manual_seed_required",
            "Hiç veri yok; yerel seed / ilk ingestion gerekli.",
        );
    }
    if !patch_fresh && !empty {
        gaps.push(gap("patch_stale", "high", "Veri güncel patch'e ait değil."));
        add_action("refresh_rates", "Güncel patch için oranları yenile.");
    }
    if input.matchup_count < HIGH_MATCHUPS {
        gaps.push(gap(
            "matchup_coverage_low",
            "medium",
            "Eşleşme örneklemi hedefin altında.",
        ));
        add_action("refresh_matchups", "Eşleşme verisini çek/genişlet.");
    }
    if build_cov < HIGH_BUILD_COVERAGE {
        gaps.push(gap(
            "build_coverage_low",
            "medium",
            "Build kapsaması hedefin altında.",
        ));
        add_action("refresh_builds", "Build verisini çek/genişlet.");
    }
    if rate_cov < RATE_COVERAGE_MIN {
        gaps.push(gap(
            "rate_coverage_low",
            "medium",
            "Şampiyon oran kapsaması düşük.",
        ));
        add_action("refresh_rates", "Oran verisini çek/genişlet.");
    }
    if role_cov < ROLE_COVERAGE_MIN {
        gaps.push(gap("role_coverage_low", "low", "Rol kapsaması sınırlı."));
    }
    if any_source_stale {
        gaps.push(gap("source_stale", "medium", "En az bir kaynak bayatladı."));
        if input.last_good_cache_available {
            add_action(
                "use_last_good_cache",
                "Yenileme gelene kadar son-iyi cache kullan.",
            );
        } else {
            add_action("refresh_rates", "Bayat kaynakları yenile.");
        }
    }
    if has_high_risk {
        gaps.push(gap(
            "high_risk_source",
            "medium",
            "Yüksek riskli kaynak katkıda bulunuyor.",
        ));
        if !input.fallback_available {
            add_action(
                "manual_seed_required",
                "Yüksek riskli kaynak + fallback yok; yerel seed gerekli.",
            );
        }
    }

    // ── Status (priority: insufficient > stale > degraded > healthy) ─────────────
    let coverage_low = input.matchup_count < HIGH_MATCHUPS
        || build_cov < HIGH_BUILD_COVERAGE
        || rate_cov < RATE_COVERAGE_MIN;
    let status = if empty || (has_high_risk && !input.fallback_available) {
        "insufficient"
    } else if !patch_fresh {
        "stale"
    } else if coverage_low || any_source_stale || role_cov < ROLE_COVERAGE_MIN {
        "degraded"
    } else {
        "healthy"
    };

    let coverage_score = (rate_cov + build_cov + matchup_cov + role_cov) / 4.0;
    let confidence = match status {
        "healthy" => "high",
        "insufficient" => "low",
        "stale" => "medium",
        _ => {
            if coverage_score >= 0.5 {
                "medium"
            } else {
                "low"
            }
        }
    };

    let summary = build_summary(status, &gaps);

    PipelineQualityReport {
        status: status.to_string(),
        confidence: confidence.to_string(),
        coverage,
        gaps,
        actions,
        summary,
    }
}

fn build_summary(status: &str, gaps: &[DataGap]) -> String {
    let head = match status {
        "healthy" => "Veri taze ve kapsamlı görünüyor",
        "stale" => "Veri eski patch'e ait; güncelleme gerekli",
        "degraded" => "Veri kullanılabilir ama eksik/bayat",
        _ => "Yeterli veri yok; analiz sınırlı",
    };
    if gaps.is_empty() {
        format!("{head}.")
    } else {
        let dims: Vec<&str> = gaps.iter().take(3).map(|g| g.dimension.as_str()).collect();
        format!("{head}; öncelikli açıklar: {}.", dims.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3600;
    const NOW: i64 = 1_000_000 * HOUR;

    fn src(name: &str, age_hours: i64, risk: &str) -> PipelineSource {
        PipelineSource {
            source: name.into(),
            updated_at: NOW - age_hours * HOUR,
            risk_level: risk.into(),
        }
    }

    fn base() -> PipelineQualityInput {
        // Full, fresh, well-covered baseline.
        PipelineQualityInput {
            now: NOW,
            current_patch: "14.11".into(),
            data_patch: Some("14.11".into()),
            target_champions: 172,
            champion_rate_count: 170,
            matchup_count: 1200,
            build_champion_count: 168,
            meta_role_count: 160,
            sources: vec![src("riot_match_v5", 2, "low")],
            fallback_available: true,
            last_good_cache_available: true,
        }
    }

    #[test]
    fn full_fresh_data_is_healthy_high() {
        let r = evaluate_pipeline_quality(&base());
        assert_eq!(r.status, "healthy");
        assert_eq!(r.confidence, "high");
        assert!(r.gaps.is_empty());
    }

    #[test]
    fn old_patch_is_stale() {
        let mut input = base();
        input.data_patch = Some("14.10".into());
        let r = evaluate_pipeline_quality(&input);
        assert_eq!(r.status, "stale");
        assert!(r.gaps.iter().any(|g| g.dimension == "patch_stale"));
        assert!(r.actions.iter().any(|a| a.action == "refresh_rates"));
    }

    #[test]
    fn low_matchups_is_degraded() {
        let mut input = base();
        input.matchup_count = 150;
        let r = evaluate_pipeline_quality(&input);
        assert_eq!(r.status, "degraded");
        assert!(r.confidence == "medium" || r.confidence == "low");
        assert!(r.gaps.iter().any(|g| g.dimension == "matchup_coverage_low"));
        assert!(r.actions.iter().any(|a| a.action == "refresh_matchups"));
    }

    #[test]
    fn low_build_coverage_is_degraded() {
        let mut input = base();
        input.build_champion_count = 40; // ~0.23 coverage
        let r = evaluate_pipeline_quality(&input);
        assert_eq!(r.status, "degraded");
        assert!(r.gaps.iter().any(|g| g.dimension == "build_coverage_low"));
        assert!(r.actions.iter().any(|a| a.action == "refresh_builds"));
    }

    #[test]
    fn high_risk_source_without_fallback_is_insufficient() {
        let mut input = base();
        input.sources = vec![src("scraper", 1, "high")];
        input.fallback_available = false;
        let r = evaluate_pipeline_quality(&input);
        assert_eq!(r.status, "insufficient");
        assert!(r.gaps.iter().any(|g| g.dimension == "high_risk_source"));
        assert!(r.actions.iter().any(|a| a.action == "manual_seed_required"));
    }

    #[test]
    fn stale_source_with_last_good_cache_is_degraded_and_uses_cache() {
        let mut input = base();
        input.sources = vec![src("riot_match_v5", 72, "low")]; // 72h > 48h stale
        input.last_good_cache_available = true;
        let r = evaluate_pipeline_quality(&input);
        assert_eq!(r.status, "degraded");
        assert!(r.gaps.iter().any(|g| g.dimension == "source_stale"));
        assert!(r.actions.iter().any(|a| a.action == "use_last_good_cache"));
        // The freshness read reflects the age.
        assert!(r.coverage.sources[0].stale);
        assert_eq!(r.coverage.sources[0].age_hours, 72);
    }

    #[test]
    fn empty_input_is_insufficient_low() {
        let input = PipelineQualityInput {
            now: NOW,
            current_patch: "14.11".into(),
            data_patch: None,
            target_champions: 172,
            champion_rate_count: 0,
            matchup_count: 0,
            build_champion_count: 0,
            meta_role_count: 0,
            sources: vec![],
            fallback_available: false,
            last_good_cache_available: false,
        };
        let r = evaluate_pipeline_quality(&input);
        assert_eq!(r.status, "insufficient");
        assert_eq!(r.confidence, "low");
        assert!(r.actions.iter().any(|a| a.action == "manual_seed_required"));
    }

    #[test]
    fn is_deterministic_and_actions_deduped() {
        let mut input = base();
        input.data_patch = Some("14.10".into()); // stale → refresh_rates
        input.champion_rate_count = 100; // rate_coverage_low → refresh_rates again
        let r1 = evaluate_pipeline_quality(&input);
        let r2 = evaluate_pipeline_quality(&input);
        assert_eq!(r1, r2);
        let refresh_rates = r1
            .actions
            .iter()
            .filter(|a| a.action == "refresh_rates")
            .count();
        assert_eq!(refresh_rates, 1, "actions must be deduped");
    }

    /// Cross-language drift guard: every machine token the engine can emit (status,
    /// confidence, gap dimension/severity, action) must have a `dataPipeline.*`
    /// label in tr.json — otherwise the badge/tooltip renders a raw key. The four
    /// inputs below collectively surface every token. en parity is the TS test's.
    #[test]
    fn every_emitted_pipeline_token_has_an_i18n_label() {
        const TR: &str = include_str!("../../src/i18n/tr.json");
        let tr: serde_json::Value = serde_json::from_str(TR).expect("tr.json parses");
        let dp = &tr["dataPipeline"];

        let healthy = base();

        let mut stale_patch = base();
        stale_patch.data_patch = Some("14.10".into());

        // Degraded with every coverage gap + a stale high-risk source (fallback on,
        // so it's degraded not insufficient) + last-good cache → use_last_good_cache.
        let mut degraded = base();
        degraded.matchup_count = 150;
        degraded.build_champion_count = 40;
        degraded.champion_rate_count = 100;
        degraded.meta_role_count = 100;
        degraded.sources = vec![src("riot_match_v5", 72, "high")];
        degraded.fallback_available = true;
        degraded.last_good_cache_available = true;

        let empty = PipelineQualityInput {
            now: NOW,
            current_patch: "14.11".into(),
            data_patch: None,
            target_champions: 172,
            champion_rate_count: 0,
            matchup_count: 0,
            build_champion_count: 0,
            meta_role_count: 0,
            sources: vec![],
            fallback_available: false,
            last_good_cache_available: false,
        };

        for input in [&healthy, &stale_patch, &degraded, &empty] {
            let r = evaluate_pipeline_quality(input);
            assert!(
                !dp["status"][&r.status].is_null(),
                "status '{}' has no dataPipeline.status label",
                r.status
            );
            assert!(
                !dp["confidence"][&r.confidence].is_null(),
                "confidence '{}' has no dataPipeline.confidence label",
                r.confidence
            );
            for g in &r.gaps {
                assert!(
                    !dp["gap"][&g.dimension].is_null(),
                    "gap '{}' has no dataPipeline.gap label",
                    g.dimension
                );
                assert!(
                    !dp["severity"][&g.severity].is_null(),
                    "severity '{}' has no dataPipeline.severity label",
                    g.severity
                );
            }
            for a in &r.actions {
                assert!(
                    !dp["action"][&a.action].is_null(),
                    "action '{}' has no dataPipeline.action label",
                    a.action
                );
            }
        }
    }
}
