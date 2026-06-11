//! op.gg aggregate-stats source.
//!
//! Pulls champion rates + builds + matchups from op.gg's internal API. op.gg sits
//! behind stricter Cloudflare protection than lolalytics/u.gg, so blocks are
//! expected and handled gracefully (return `Err` → other sources carry coverage,
//! last-good cache stays). Faz B1 implements the real fetch.
#![allow(dead_code)]

use crate::meta::source::FetchCtx;
use crate::recommendation::ingestion_contract::CanonicalRowSet;
use anyhow::Result;

pub async fn fetch(_ctx: &FetchCtx) -> Result<CanonicalRowSet> {
    anyhow::bail!("op.gg Cloudflare-gated (internal API 403); hafiflik için bağlanmadı")
}
