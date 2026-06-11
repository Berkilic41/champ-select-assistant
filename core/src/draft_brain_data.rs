//! Data Supremacy v1 — data-source registry + local data-pack quality.
//!
//! Pure (no I/O): the command layer (`commands/data_quality.rs`) gathers counts
//! from the local SQLite DB + bundled resources and feeds them here. Reuses the
//! cloud-compatible [`crate::draft_brain::DataPack`] so the local
//! pack and the cloud pack share exactly one shape.
//!
//! This is intentionally separate from the cloud `DataPack`/quality report that
//! Codex owns (`draft_brain.rs`, `commands/meta_sync.rs::get_data_quality_report`).
//! It adds the *source registry* (8 kinds + risk posture) and the derived signals
//! that report does not carry yet: `primary_role_build_coverage`,
//! `exact_matchup_coverage`, `stale_sources`, `high_risk_sources`, `fallback_active`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::draft_brain::{DataPack, DataPackQuality, LOCAL_SEED_DATA_PACK_VERSION};

/// A (champion, role) build/matchup table is "deep enough" for high confidence
/// at these thresholds (same shape as the cloud pack's `confidence()`).
const HIGH_MATCHUPS: u32 = 1_000;
const HIGH_BUILD_COVERAGE: f32 = 0.95;

/// Known data-source kinds and their inherent risk posture. The risk/confidence
/// here describe the *kind*, before per-row sample-size weighting.
///
/// The registry intentionally enumerates ALL known kinds; in Data Supremacy v1
/// some (e.g. `RiotMatchV5`, `LcuLocal`, `ScraperHighRisk`) are part of the spec
/// but not yet wired to a live ingestion path — hence `allow(dead_code)`.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSourceKind {
    LocalSeed,
    ManualSeed,
    Meraki,
    Ddragon,
    RiotMatchV5,
    LcuLocal,
    CloudPostgres,
    ScraperHighRisk,
}

impl DataSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalSeed => "local_seed",
            Self::ManualSeed => "manual_seed",
            Self::Meraki => "meraki",
            Self::Ddragon => "ddragon",
            Self::RiotMatchV5 => "riot_match_v5",
            Self::LcuLocal => "lcu_local",
            Self::CloudPostgres => "cloud_postgres",
            Self::ScraperHighRisk => "scraper_high_risk",
        }
    }

    /// ToS / reliability risk of relying on this source.
    pub fn risk_level(self) -> &'static str {
        match self {
            Self::LocalSeed | Self::ManualSeed | Self::Ddragon | Self::LcuLocal => "low",
            Self::Meraki | Self::RiotMatchV5 | Self::CloudPostgres => "medium",
            Self::ScraperHighRisk => "high",
        }
    }

    /// Confidence we place in this source *kind* before sample weighting.
    pub fn default_confidence(self) -> &'static str {
        match self {
            Self::RiotMatchV5 | Self::CloudPostgres => "high",
            Self::Meraki | Self::Ddragon | Self::LcuLocal => "medium",
            Self::LocalSeed | Self::ManualSeed => "heuristic",
            Self::ScraperHighRisk => "low",
        }
    }
}

/// One source's live status in the registry. Carries the full Data Supremacy
/// metadata set (incl. `region` + `fallback_used`, which `DataSourceBadge` lacks).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct DataSourceEntry {
    pub source: String,
    pub patch: Option<String>,
    pub region: Option<String>,
    pub sample_size: Option<u32>,
    pub confidence: String,
    pub risk_level: String,
    pub updated_at: Option<i64>,
    pub fallback_used: bool,
}

impl DataSourceEntry {
    pub fn from_kind(kind: DataSourceKind, sample_size: Option<u32>, fallback_used: bool) -> Self {
        Self {
            source: kind.as_str().to_string(),
            patch: None,
            region: None,
            sample_size,
            confidence: kind.default_confidence().to_string(),
            risk_level: kind.risk_level().to_string(),
            updated_at: None,
            fallback_used,
        }
    }
}

/// Raw coverage counts gathered from the local DB + bundled resources.
/// Plain (not serialized) — only an input to [`compute_registry_report`].
#[derive(Debug, Clone, Default)]
pub struct LocalCoverage {
    pub total_champions: u32,
    pub champion_rates_count: u32,
    pub matchup_count: u32,
    pub build_count: u32,
    /// DISTINCT champion_id in `builds`.
    pub build_champions: u32,
    /// DISTINCT champion_id in `champion_rates`.
    pub meta_role_champions: u32,
    /// Source kinds currently contributing data.
    pub sources: Vec<DataSourceEntry>,
}

/// Derived data-source registry report — the signals the cloud quality report
/// does not carry yet.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct DataSourceRegistryReport {
    pub champion_rates_count: u32,
    pub matchup_count: u32,
    pub build_count: u32,
    /// 0..1 — fraction of champions that have at least one build row.
    pub primary_role_build_coverage: f32,
    /// 0..1 — fraction of champions with at least one live meta-rate row.
    pub meta_role_coverage: f32,
    /// Exact champion-vs-champion matchup rows available.
    pub exact_matchup_coverage: u32,
    /// Source labels whose cache has expired / was never synced.
    pub stale_sources: Vec<String>,
    /// Source labels with inherent "high" risk (e.g. scrapers).
    pub high_risk_sources: Vec<String>,
    /// True when no live/cloud pack is active and we run on local seeds.
    pub fallback_active: bool,
    /// Overall confidence: `high` / `medium` / `low`.
    pub confidence: String,
    pub sources: Vec<DataSourceEntry>,
    /// Unix epoch seconds. `u32` (→ TS `number`) to match the pack-family
    /// `generated_at` (`DataPack`/`PackSyncStatus`/`DraftBrainQualityReport`) so
    /// the frontend never sees a `bigint` for this field. See
    /// `docs/audit/data-quality-parity.md` (generated_at uniformity).
    pub generated_at: u32,
}

/// Pure: turn raw coverage + runtime flags into a registry report.
pub fn compute_registry_report(
    coverage: &LocalCoverage,
    stale_sources: Vec<String>,
    fallback_active: bool,
    generated_at: u32,
) -> DataSourceRegistryReport {
    let coverage_ratio = |n: u32| {
        if coverage.total_champions > 0 {
            (n as f32 / coverage.total_champions as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    let primary_role_build_coverage = coverage_ratio(coverage.build_champions);
    let meta_role_coverage = coverage_ratio(coverage.meta_role_champions);

    let high_risk_sources = coverage
        .sources
        .iter()
        .filter(|s| s.risk_level == "high")
        .map(|s| s.source.clone())
        .collect();

    let confidence =
        data_confidence(coverage, primary_role_build_coverage, fallback_active).to_string();

    DataSourceRegistryReport {
        champion_rates_count: coverage.champion_rates_count,
        matchup_count: coverage.matchup_count,
        build_count: coverage.build_count,
        primary_role_build_coverage,
        meta_role_coverage,
        exact_matchup_coverage: coverage.matchup_count,
        stale_sources,
        high_risk_sources,
        fallback_active,
        confidence,
        sources: coverage.sources.clone(),
        generated_at,
    }
}

fn data_confidence(
    coverage: &LocalCoverage,
    build_cov: f32,
    fallback_active: bool,
) -> &'static str {
    let has_any =
        coverage.matchup_count > 0 || coverage.build_count > 0 || coverage.champion_rates_count > 0;
    if !has_any {
        return "low";
    }
    if build_cov >= HIGH_BUILD_COVERAGE
        && coverage.matchup_count >= HIGH_MATCHUPS
        && coverage.champion_rates_count > 0
        && !fallback_active
    {
        "high"
    } else {
        "medium"
    }
}

/// Build a local data pack that carries REAL quality signal (not zeros), so
/// recommendation `DataSourceBadge`s reflect actual local coverage when the cloud
/// is unavailable. Mirrors [`crate::draft_brain::local_seed_data_pack`]
/// but with live counts; always `fallback = true`.
pub fn build_local_data_pack(
    coverage: &LocalCoverage,
    patch: Option<String>,
    region: Option<String>,
) -> DataPack {
    let build_cov = if coverage.total_champions == 0 {
        0.0
    } else {
        coverage.build_champions as f32 / coverage.total_champions as f32
    };
    let confidence = data_confidence(coverage, build_cov, true).to_string();
    let sources: Vec<String> = if coverage.sources.is_empty() {
        vec![
            "builds_seed".to_string(),
            "matchup_seed".to_string(),
            "draft_iq".to_string(),
        ]
    } else {
        coverage.sources.iter().map(|s| s.source.clone()).collect()
    };

    DataPack {
        version: LOCAL_SEED_DATA_PACK_VERSION.to_string(),
        patch,
        region: region.or_else(|| Some("local".to_string())),
        sources,
        quality: DataPackQuality {
            champion_rates: coverage.champion_rates_count,
            matchups: coverage.matchup_count,
            builds: coverage.build_count,
            feedback: 0,
            draft_samples: 0,
        },
        fallback: true,
        confidence: Some(confidence),
        generated_at: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coverage(
        total: u32,
        rates: u32,
        matchups: u32,
        builds: u32,
        build_champs: u32,
        sources: Vec<DataSourceEntry>,
    ) -> LocalCoverage {
        LocalCoverage {
            total_champions: total,
            champion_rates_count: rates,
            matchup_count: matchups,
            build_count: builds,
            build_champions: build_champs,
            meta_role_champions: 0,
            sources,
        }
    }

    #[test]
    fn source_kind_risk_and_confidence_posture() {
        assert_eq!(DataSourceKind::ScraperHighRisk.risk_level(), "high");
        assert_eq!(DataSourceKind::LocalSeed.risk_level(), "low");
        assert_eq!(DataSourceKind::RiotMatchV5.risk_level(), "medium");
        assert_eq!(DataSourceKind::RiotMatchV5.default_confidence(), "high");
        assert_eq!(DataSourceKind::LocalSeed.default_confidence(), "heuristic");
    }

    #[test]
    fn local_data_pack_carries_real_quality_not_zeros() {
        let c = coverage(
            172,
            100,
            80,
            60,
            31,
            vec![DataSourceEntry::from_kind(
                DataSourceKind::LocalSeed,
                Some(80),
                true,
            )],
        );
        let pack = build_local_data_pack(&c, Some("16.11".into()), None);
        assert_eq!(pack.quality.matchups, 80, "matchup quality must be real");
        assert_eq!(pack.quality.builds, 60, "build quality must be real");
        assert_eq!(pack.quality.champion_rates, 100);
        assert!(pack.fallback, "local pack is always a fallback");
        assert!(!pack.sources.is_empty(), "sources must not be empty");
        assert_eq!(pack.region.as_deref(), Some("local"));
    }

    #[test]
    fn empty_coverage_still_yields_valid_fallback_pack() {
        // Invalid/empty cache → local fallback must still be a usable pack.
        let c = coverage(172, 0, 0, 0, 0, vec![]);
        let pack = build_local_data_pack(&c, None, None);
        assert!(pack.fallback);
        assert_eq!(pack.version, LOCAL_SEED_DATA_PACK_VERSION);
        assert_eq!(
            pack.sources.len(),
            3,
            "defaults seeded when no sources known"
        );
    }

    #[test]
    fn high_coverage_yields_high_confidence() {
        let c = coverage(
            172,
            172,
            1_200,
            200,
            170,
            vec![DataSourceEntry::from_kind(
                DataSourceKind::RiotMatchV5,
                Some(1_200),
                false,
            )],
        );
        let r = compute_registry_report(&c, vec![], false, 0);
        assert_eq!(r.confidence, "high");
        assert!(r.primary_role_build_coverage > 0.9);
        assert_eq!(r.exact_matchup_coverage, 1_200);
    }

    #[test]
    fn no_data_yields_low_confidence() {
        let c = coverage(172, 0, 0, 0, 0, vec![]);
        let r = compute_registry_report(&c, vec![], true, 0);
        assert_eq!(r.confidence, "low");
        assert_eq!(r.primary_role_build_coverage, 0.0);
    }

    #[test]
    fn partial_data_is_medium_and_flags_stale_and_high_risk() {
        let c = coverage(
            172,
            100,
            80,
            60,
            31,
            vec![
                DataSourceEntry::from_kind(DataSourceKind::ManualSeed, Some(80), true),
                DataSourceEntry::from_kind(DataSourceKind::ScraperHighRisk, Some(50), false),
            ],
        );
        let r = compute_registry_report(&c, vec!["data_pack".to_string()], true, 0);
        assert_eq!(r.confidence, "medium");
        assert!(r.fallback_active);
        assert!(r.high_risk_sources.iter().any(|s| s == "scraper_high_risk"));
        assert_eq!(r.stale_sources, vec!["data_pack".to_string()]);
    }

    /// Locks the confidence thresholds documented in
    /// `docs/audit/data-quality-parity.md` and mirrored by the backend
    /// `confidence_band` (HIGH_MATCHUPS=1000, HIGH_BUILD_COVERAGE=0.95, target=172).
    /// If these boundaries change, backend/client parity drifts → this test fires.
    #[test]
    fn confidence_thresholds_match_documented_parity() {
        let band = |c: &LocalCoverage, fallback: bool| {
            compute_registry_report(c, vec![], fallback, 0).confidence
        };
        // matchups 999 < 1000 → just below the high bar → medium.
        assert_eq!(
            band(&coverage(172, 50, 999, 200, 170, vec![]), false),
            "medium"
        );
        // build coverage 163/172 = 0.948 < 0.95 → medium even with matchups ≥ 1000.
        assert_eq!(
            band(&coverage(172, 50, 1_500, 300, 163, vec![]), false),
            "medium"
        );
        // all bars cleared (164/172 = 0.953 ≥ 0.95, matchups = 1000, rates > 0) → high.
        assert_eq!(
            band(&coverage(172, 50, 1_000, 300, 164, vec![]), false),
            "high"
        );
        // fallback active forces medium regardless of coverage.
        assert_eq!(
            band(&coverage(172, 50, 1_000, 300, 170, vec![]), true),
            "medium"
        );
        // no data at all → low.
        assert_eq!(band(&coverage(172, 0, 0, 0, 0, vec![]), true), "low");
    }
}
