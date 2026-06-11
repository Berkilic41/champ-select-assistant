//! Mobalytics aggregate-stats source.
//!
//! Pulls champion rates + builds + matchups from Mobalytics' GraphQL API. Like
//! op.gg, Mobalytics sits behind stricter Cloudflare protection; blocks are
//! expected and handled gracefully. Faz B1 implements the real fetch.
#![allow(dead_code)]

use crate::meta::source::FetchCtx;
use crate::recommendation::ingestion_contract::CanonicalRowSet;
use anyhow::Result;

pub async fn fetch(_ctx: &FetchCtx) -> Result<CanonicalRowSet> {
    anyhow::bail!("Mobalytics Cloudflare-gated (GraphQL); hafiflik için bağlanmadı")
}
