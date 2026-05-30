use anyhow::Result;
use rusqlite::{params, Connection};
use serde::Serialize;

pub fn upsert_mastery(
    conn: &Connection,
    puuid: &str,
    champion_id: i64,
    mastery_level: i64,
    mastery_points: i64,
    last_play_time: Option<i64>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO mastery (puuid, champion_id, mastery_level, mastery_points, last_play_time)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(puuid, champion_id) DO UPDATE SET
             mastery_level  = excluded.mastery_level,
             mastery_points = excluded.mastery_points,
             last_play_time = excluded.last_play_time",
        params![
            puuid,
            champion_id,
            mastery_level,
            mastery_points,
            last_play_time
        ],
    )?;
    Ok(())
}

/// A single mastery row for a summoner + champion pair.
#[derive(Debug, Serialize)]
pub struct MasteryRow {
    pub champion_id: i64,
    #[serde(rename = "mastery_level")]
    pub level: i64,
    #[serde(rename = "mastery_points")]
    pub points: i64,
    pub last_play_time: Option<i64>,
}

/// Return the top-N mastery entries for a summoner, ordered by mastery_points descending.
pub fn top_for_puuid(conn: &Connection, puuid: &str, limit: i64) -> Result<Vec<MasteryRow>> {
    let mut stmt = conn.prepare(
        "SELECT champion_id, mastery_level, mastery_points, last_play_time
         FROM mastery
         WHERE puuid = ?1
         ORDER BY mastery_points DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![puuid, limit], |r| {
        Ok(MasteryRow {
            champion_id: r.get(0)?,
            level: r.get(1)?,
            points: r.get(2)?,
            last_play_time: r.get(3)?,
        })
    })?;
    rows.map(|r| r.map_err(Into::into)).collect()
}

/// Returns top-N mastery entries as (champion_id, mastery_level, mastery_points) tuples.
#[allow(dead_code)]
pub fn top_mastery_for_puuid(
    conn: &Connection,
    puuid: &str,
    n: i64,
) -> Result<Vec<(i64, i64, i64)>> {
    let rows = top_for_puuid(conn, puuid, n)?;
    Ok(rows
        .into_iter()
        .map(|r| (r.champion_id, r.level, r.points))
        .collect())
}
