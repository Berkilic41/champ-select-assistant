use crate::riot::client::ChampionStats;
use anyhow::Result;
use rusqlite::{params, Connection};

#[allow(clippy::too_many_arguments)]
pub fn insert_match(
    conn: &Connection,
    match_id: &str,
    puuid: &str,
    champion_id: i64,
    position: Option<&str>,
    win: bool,
    kills: i64,
    deaths: i64,
    assists: i64,
    duration_secs: i64,
    queue_id: i64,
    played_at: i64,
    // Total creep score (minions + neutral monsters); `None` when unavailable.
    cs: Option<i64>,
) -> Result<bool> {
    let rows = conn.execute(
        "INSERT OR IGNORE INTO matches
         (match_id, puuid, champion_id, position, win, kills, deaths, assists,
          duration_secs, queue_id, played_at, cs)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            match_id,
            puuid,
            champion_id,
            position,
            win as i64,
            kills,
            deaths,
            assists,
            duration_secs,
            queue_id,
            played_at,
            cs
        ],
    )?;
    Ok(rows > 0)
}

pub fn player_stats(conn: &Connection, puuid: &str) -> Result<Vec<ChampionStats>> {
    let mut stmt = conn.prepare(
        "SELECT champion_id,
                COUNT(*) AS games,
                SUM(win) AS wins,
                AVG(CAST(kills + assists AS REAL) / MAX(deaths, 1)) AS kda_avg
         FROM matches
         WHERE puuid = ?1
         GROUP BY champion_id
         ORDER BY games DESC
         LIMIT 20",
    )?;
    let rows = stmt.query_map(params![puuid], |r| {
        Ok(ChampionStats {
            champion_id: r.get(0)?,
            games: r.get(1)?,
            wins: r.get(2)?,
            kda_avg: r.get(3)?,
        })
    })?;
    rows.map(|r| r.map_err(Into::into)).collect()
}
