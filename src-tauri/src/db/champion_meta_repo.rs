use anyhow::Result;
use rusqlite::{params, Connection};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ChampionMetaRow {
    pub champion_id: u32,
    /// JSON-serialised Vec<String>, e.g. `["fighter","tank"]`
    pub roles: String,
    pub is_melee: bool,
    pub attack_range: i32,
    pub resource_type: Option<String>,
}

pub fn upsert_meta(conn: &Connection, row: &ChampionMetaRow) -> Result<()> {
    conn.execute(
        "INSERT INTO champion_meta (champion_id, roles, is_melee, attack_range, resource_type)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(champion_id) DO UPDATE SET
             roles         = excluded.roles,
             is_melee      = excluded.is_melee,
             attack_range  = excluded.attack_range,
             resource_type = excluded.resource_type",
        params![
            row.champion_id,
            row.roles,
            row.is_melee as i32,
            row.attack_range,
            row.resource_type,
        ],
    )?;
    Ok(())
}

pub fn get_all(conn: &Connection) -> Result<HashMap<u32, ChampionMetaRow>> {
    let mut stmt = conn.prepare(
        "SELECT champion_id, roles, is_melee, attack_range, resource_type FROM champion_meta",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ChampionMetaRow {
            champion_id: r.get::<_, i64>(0)? as u32,
            roles: r.get(1)?,
            is_melee: r.get::<_, i32>(2)? != 0,
            attack_range: r.get(3)?,
            resource_type: r.get(4)?,
        })
    })?;

    let mut map = HashMap::new();
    for row in rows {
        let row = row?;
        map.insert(row.champion_id, row);
    }
    Ok(map)
}

/// Convenience: extract the roles list from JSON string stored in the row.
pub fn parse_roles(roles_json: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(roles_json).unwrap_or_default()
}
