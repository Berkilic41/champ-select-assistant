//! Lolalytics aggregate-stats source.
//!
//! Lolalytics aggregate-stats source — **registered but not scheduled**.
//!
//! Lolalytics' `api.lolalytics.com/mega` endpoint and its pages sit behind a
//! Cloudflare browser challenge: working community scrapers drive it with a
//! headless browser (Selenium), not a plain HTTP client. Bundling a headless
//! browser would bloat this intentionally-lightweight app and add a fragile
//! dependency, so we keep the variant registered (provenance/risk are wired) but
//! `all_sources()` does not schedule it. `fetch` returns `Err` so that, if ever
//! called, it degrades gracefully and other sources carry coverage. Lane
//! matchups/builds instead come from u.gg (open CDN) + Match-V5 (real games).
#![allow(dead_code)]

use crate::meta::source::FetchCtx;
use crate::recommendation::ingestion_contract::CanonicalRowSet;
use anyhow::Result;

/// A real browser UA — kept for a future headless/residential path.
pub(crate) const LOLALYTICS_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120";

pub async fn fetch(_ctx: &FetchCtx) -> Result<CanonicalRowSet> {
    anyhow::bail!(
        "Lolalytics Cloudflare-gated (headless tarayıcı gerekir); hafiflik için bağlanmadı"
    )
}
