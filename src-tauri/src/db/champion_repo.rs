use anyhow::Result;
use rusqlite::{params, Connection};
use std::time::{SystemTime, UNIX_EPOCH};

// ChampionRecord is now a shared pure DTO in `csa-core`. Re-exported so
// `crate::db::champion_repo::ChampionRecord` paths keep working; the repo fns below
// still construct it via struct literals.
pub use csa_core::types::ChampionRecord;

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub fn upsert_champion(
    conn: &Connection,
    champion_id: i64,
    key: &str,
    name: &str,
    title: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO champions (champion_id, key, name, title, cached_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(champion_id) DO UPDATE SET
             key       = excluded.key,
             name      = excluded.name,
             title     = excluded.title,
             cached_at = excluded.cached_at",
        params![champion_id, key, name, title, now_secs()],
    )?;
    Ok(())
}

pub fn get_champion_by_id(conn: &Connection, champion_id: i64) -> Result<Option<ChampionRecord>> {
    let result = conn.query_row(
        "SELECT champion_id, key, name, title FROM champions WHERE champion_id = ?1",
        params![champion_id],
        |r| {
            Ok(ChampionRecord {
                champion_id: r.get(0)?,
                key: r.get(1)?,
                name: r.get(2)?,
                title: r.get(3)?,
            })
        },
    );
    match result {
        Ok(rec) => Ok(Some(rec)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn list_all(conn: &Connection) -> Result<Vec<ChampionRecord>> {
    let mut stmt =
        conn.prepare("SELECT champion_id, key, name, title FROM champions ORDER BY name ASC")?;
    let rows = stmt.query_map([], |r| {
        Ok(ChampionRecord {
            champion_id: r.get(0)?,
            key: r.get(1)?,
            name: r.get(2)?,
            title: r.get(3)?,
        })
    })?;
    rows.map(|r| r.map_err(Into::into)).collect()
}
