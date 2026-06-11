use anyhow::Result;
use rusqlite::{params, Connection};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct MatchupRow {
    pub champion_id: i64,
    pub opponent_id: i64,
    pub position: String,
    pub games: i64,
    pub wins: i64,
    pub win_rate: f64,
    pub source: String,
    pub patch_version: String,
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub fn upsert_matchup(conn: &Connection, row: &MatchupRow) -> Result<()> {
    upsert_matchup_with_metadata(conn, row, "global", "low", row.games)
}

pub fn upsert_matchup_with_metadata(
    conn: &Connection,
    row: &MatchupRow,
    region: &str,
    confidence: &str,
    sample_size: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO champion_matchups
             (champion_id, opponent_id, position, games, wins, win_rate,
              source, patch_version, region, confidence, sample_size, cached_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(champion_id, opponent_id, position, source) DO UPDATE SET
             games         = excluded.games,
             wins          = excluded.wins,
             win_rate      = excluded.win_rate,
             patch_version = excluded.patch_version,
             region        = excluded.region,
             confidence    = excluded.confidence,
             sample_size   = excluded.sample_size,
             cached_at     = excluded.cached_at",
        params![
            row.champion_id,
            row.opponent_id,
            row.position,
            row.games,
            row.wins,
            row.win_rate,
            row.source,
            row.patch_version,
            region,
            confidence,
            sample_size,
            now_secs(),
        ],
    )?;
    Ok(())
}

pub fn fetch_all_for_position(conn: &Connection, position: &str) -> Result<Vec<MatchupRow>> {
    let mut stmt = conn.prepare(
        "SELECT champion_id, opponent_id, position, games, wins, win_rate, source, patch_version
         FROM champion_matchups
         WHERE position = ?1",
    )?;
    let rows = stmt.query_map(params![position], |r| {
        Ok(MatchupRow {
            champion_id: r.get(0)?,
            opponent_id: r.get(1)?,
            position: r.get(2)?,
            games: r.get(3)?,
            wins: r.get(4)?,
            win_rate: r.get(5)?,
            source: r.get(6)?,
            patch_version: r.get(7)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}
