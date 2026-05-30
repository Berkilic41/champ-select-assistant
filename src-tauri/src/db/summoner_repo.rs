use crate::riot::client::SummonerInfo;
use anyhow::Result;
use rusqlite::{params, Connection};
use std::time::{SystemTime, UNIX_EPOCH};

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub fn upsert_summoner(conn: &Connection, info: &SummonerInfo) -> Result<()> {
    conn.execute(
        "INSERT INTO summoners (puuid, game_name, tag_line, region, cached_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(puuid) DO UPDATE SET
             game_name = excluded.game_name,
             tag_line  = excluded.tag_line,
             region    = excluded.region,
             cached_at = excluded.cached_at",
        params![
            info.puuid,
            info.game_name,
            info.tag_line,
            info.region,
            now_secs()
        ],
    )?;
    Ok(())
}

pub fn get_summoner_by_puuid(conn: &Connection, puuid: &str) -> Result<Option<SummonerInfo>> {
    let result = conn.query_row(
        "SELECT puuid, game_name, tag_line, region FROM summoners WHERE puuid = ?1",
        params![puuid],
        |r| {
            Ok(SummonerInfo {
                puuid: r.get(0)?,
                game_name: r.get(1)?,
                tag_line: r.get(2)?,
                region: r.get(3)?,
            })
        },
    );
    match result {
        Ok(info) => Ok(Some(info)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Returns the most recently cached summoner's puuid, if any.
/// Used by champ-select to pick the active player without prop-drilling
/// from the lobby flow.
pub fn get_active_puuid(conn: &Connection) -> Result<Option<String>> {
    let result = conn.query_row(
        "SELECT puuid FROM summoners ORDER BY cached_at DESC LIMIT 1",
        [],
        |r| r.get::<_, String>(0),
    );
    match result {
        Ok(p) => Ok(Some(p)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}
