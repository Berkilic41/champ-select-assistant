//! Multi-source aggregate-stats ingestion framework (Faz B — aggressive data growth).
//!
//! Every external aggregate source (lolalytics, u.gg, op.gg, mobalytics, Meraki)
//! reduces to one shape: `fetch(ctx) -> CanonicalRowSet` (rates + matchups +
//! builds, each row carrying its own `source` / `confidence` / `sample_size`).
//! The command layer upserts those rows via `upsert_canonical_rows`; because each
//! row keeps its `source`, **multiple sources coexist** per (champion, role) and
//! the read layer cross-validates them (agreement → higher confidence; outliers
//! down-weighted). A blocked or erroring source returns `Err` and the others
//! carry coverage — the last-good cache stays intact. **No fabrication:** a source
//! that can't fetch yields nothing, never invented numbers.
//!
//! A closed `enum` (not a `dyn` trait) keeps native `async fn` free of
//! object-safety constraints and makes the registry trivially testable.
#![allow(dead_code)] // fetch/ttl wired into the scheduler in Faz B2

use crate::db::champion_repo::ChampionRecord;
use crate::recommendation::ingestion_contract::CanonicalRowSet;
use anyhow::Result;

/// Everything a source needs to fetch a full snapshot, resolved by the command
/// layer from the DB + DDragon (the engine-pure callers do no I/O themselves).
#[derive(Debug, Clone)]
pub struct FetchCtx {
    /// Normalized DDragon patch, e.g. "14.11".
    pub patch: String,
    /// Region tag written on every row; aggregate sources use "all".
    pub region: String,
    /// Champions to fetch (id ↔ key ↔ name), typically the full roster.
    pub champions: Vec<ChampionRecord>,
}

/// The fixed set of aggregate-stats sources. Adding one = a new variant + arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateSource {
    Lolalytics,
    UGg,
    OpGg,
    Mobalytics,
    Meraki,
}

impl AggregateSource {
    /// Stable machine key — also the `source` tag stamped on every row.
    pub fn key(&self) -> &'static str {
        match self {
            AggregateSource::Lolalytics => "lolalytics",
            AggregateSource::UGg => "u_gg",
            AggregateSource::OpGg => "op_gg",
            AggregateSource::Mobalytics => "mobalytics",
            AggregateSource::Meraki => "meraki",
        }
    }

    /// Provenance risk. Reputable aggregate CDNs (Meraki, u.gg — stable JSON, years
    /// of uptime) are `"medium"`; fragile HTML/Cloudflare-gated scrapers
    /// (lolalytics, op.gg, mobalytics) are `"high"`. The last-good cache policy
    /// never lets a high-risk candidate overwrite the cache.
    pub fn risk(&self) -> &'static str {
        match self {
            AggregateSource::Meraki | AggregateSource::UGg => "medium",
            _ => "high",
        }
    }

    /// Refresh cadence. Aggregate stats move on the order of a day, so a 6 h TTL
    /// keeps data fresh without hammering the source (sustainable, ban-averse).
    pub fn ttl_secs(&self) -> u64 {
        6 * 60 * 60
    }

    /// Fetch a full snapshot. A blocked/unreachable source returns `Err`; callers
    /// keep the last-good cache and let the other sources carry coverage.
    pub async fn fetch(&self, ctx: &FetchCtx) -> Result<CanonicalRowSet> {
        match self {
            AggregateSource::Lolalytics => super::lolalytics::fetch(ctx).await,
            AggregateSource::UGg => super::ugg::fetch(ctx).await,
            AggregateSource::OpGg => super::opgg::fetch(ctx).await,
            AggregateSource::Mobalytics => super::mobalytics::fetch(ctx).await,
            AggregateSource::Meraki => super::meraki::fetch(ctx).await,
        }
    }
}

/// The aggregate sources we actively schedule — those reachable from a plain
/// reqwest client (open CDN / official-ish JSON), with a real `fetch`.
///
/// `Lolalytics`, `OpGg` and `Mobalytics` are **registered enum variants** (the
/// framework is ready for them) but sit behind Cloudflare browser challenges, so
/// a headless-browser-free client can't reach them reliably. We deliberately do
/// **not** schedule them — bundling a headless browser would bloat this
/// intentionally-lightweight app, and scheduling always-failing sources would
/// pollute the data-quality report with fake failures. They light up the moment
/// a working `fetch` exists (or from a residential IP that isn't challenged).
pub fn all_sources() -> Vec<AggregateSource> {
    vec![AggregateSource::UGg, AggregateSource::Meraki]
}

/// Every enum variant, scheduled or not (for documentation/registry tests).
pub fn all_variants() -> [AggregateSource; 5] {
    [
        AggregateSource::Lolalytics,
        AggregateSource::UGg,
        AggregateSource::OpGg,
        AggregateSource::Mobalytics,
        AggregateSource::Meraki,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_has_a_unique_key() {
        let mut keys: Vec<&str> = all_variants().iter().map(|s| s.key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), 5, "source keys must be unique");
    }

    #[test]
    fn scheduled_sources_are_the_reqwest_reachable_ones() {
        // Only u.gg (open CDN) + Meraki (CDN) are scheduled; the Cloudflare-gated
        // variants are registered but intentionally not run (no fake failures).
        assert_eq!(
            all_sources(),
            vec![AggregateSource::UGg, AggregateSource::Meraki]
        );
        assert!(all_sources().iter().all(|s| s.ttl_secs() == 6 * 60 * 60));
    }

    #[test]
    fn provenance_risk_is_honest() {
        // Reputable CDNs (Meraki, u.gg) → "medium"; fragile/gated scrapers → "high".
        assert_eq!(AggregateSource::Meraki.risk(), "medium");
        assert_eq!(AggregateSource::UGg.risk(), "medium");
        for s in [
            AggregateSource::Lolalytics,
            AggregateSource::OpGg,
            AggregateSource::Mobalytics,
        ] {
            assert_eq!(s.risk(), "high", "{} must be high-risk", s.key());
        }
    }

    #[test]
    fn keys_match_source_risk_classifier_expectations() {
        // These keys flow into `commands::data_quality::source_risk` and the row
        // `source` column; keep them stable + snake_case.
        assert_eq!(AggregateSource::UGg.key(), "u_gg");
        assert_eq!(AggregateSource::OpGg.key(), "op_gg");
    }
}
