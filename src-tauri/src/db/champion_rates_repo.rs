//! Repository for `champion_rates` (V008) — patch-level win/pick/ban rates per
//! champion + position, sourced from external aggregators (Meraki, ...).
//!
//! Uniqueness is enforced on `(champion_id, position, source)` so multiple
//! sources can coexist for the same champion + lane; re-upsert overwrites the
//! existing source row in-place.

// Wired into the recommendation engine in a future sprint — until then the
// repo functions exist for the Meraki ingester (and unit tests) only.
#![allow(dead_code)]

use anyhow::Result;
use rusqlite::{params, Connection};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq)]
pub struct ChampionRateRow {
    pub champion_id: u32,
    pub position: String,
    pub win_rate: f32,
    pub pick_rate: f32,
    pub ban_rate: f32,
    pub sample_size: u32,
    pub patch: String,
    pub source: String,
    pub confidence: String,
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Insert-or-replace a single rate row keyed by (champion_id, position, source).
/// `cached_at` is always stamped to the current epoch on write.
pub fn upsert_rate(conn: &Connection, row: &ChampionRateRow) -> Result<()> {
    upsert_rate_with_region(conn, row, "global")
}

pub fn upsert_rate_with_region(
    conn: &Connection,
    row: &ChampionRateRow,
    region: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO champion_rates (
            champion_id, position, win_rate, pick_rate, ban_rate,
            sample_size, patch, source, confidence, region, cached_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(champion_id, position, source) DO UPDATE SET
             win_rate    = excluded.win_rate,
             pick_rate   = excluded.pick_rate,
             ban_rate    = excluded.ban_rate,
             sample_size = excluded.sample_size,
             patch       = excluded.patch,
             confidence  = excluded.confidence,
             region      = excluded.region,
             cached_at   = excluded.cached_at",
        params![
            row.champion_id as i64,
            row.position,
            row.win_rate as f64,
            row.pick_rate as f64,
            row.ban_rate as f64,
            row.sample_size as i64,
            row.patch,
            row.source,
            row.confidence,
            region,
            now_secs(),
        ],
    )?;
    Ok(())
}

fn map_row(r: &rusqlite::Row) -> rusqlite::Result<ChampionRateRow> {
    Ok(ChampionRateRow {
        champion_id: r.get::<_, i64>(0)? as u32,
        position: r.get(1)?,
        win_rate: r.get::<_, f64>(2)? as f32,
        pick_rate: r.get::<_, f64>(3)? as f32,
        ban_rate: r.get::<_, f64>(4)? as f32,
        sample_size: r.get::<_, i64>(5)? as u32,
        patch: r.get(6)?,
        source: r.get(7)?,
        confidence: r.get(8)?,
    })
}

/// Raw summed `sample_size` per (champion_id, position) across all sources. Used to
/// compute each champion's **role share** so off-role noise can be filtered: u.gg
/// lists every champion in every lane with non-trivial absolute samples (e.g. ~2.6k
/// "bottom" Olaf games), which clears a flat sample floor even though it's ~1% of his
/// play. Comparing a role's sample to the champion's cross-lane total separates a
/// real lane (Caitlyn 97% bottom) from noise (Olaf 1% bottom).
pub fn position_sample_totals(conn: &Connection) -> Result<Vec<(u32, String, u32)>> {
    let mut stmt = conn.prepare(
        "SELECT champion_id, position, CAST(SUM(sample_size) AS INTEGER)
         FROM champion_rates
         GROUP BY champion_id, position",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)? as u32,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)? as u32,
        ))
    })?;
    rows.map(|r| r.map_err(Into::into)).collect()
}

/// Bulk fetch every rate row for a given position — used by the scoring engine
/// to score all candidate champions for the local player's lane in one query.
pub fn get_all_for_position(conn: &Connection, position: &str) -> Result<Vec<ChampionRateRow>> {
    let mut stmt = conn.prepare(
        "SELECT champion_id, position, win_rate, pick_rate, ban_rate,
                sample_size, patch, source, confidence
         FROM champion_rates
         WHERE position = ?1
         ORDER BY champion_id, source",
    )?;
    let rows = stmt.query_map(params![position], map_row)?;
    rows.map(|r| r.map_err(Into::into)).collect()
}
