use anyhow::Result;
use rusqlite::{params, Connection};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct BuildRow {
    pub champion_id: i64,
    pub position: String,
    pub patch_version: String,
    /// JSON-serialised Vec<u32>
    pub item_ids: String,
    /// JSON-serialised Vec<u32> — [keystone_id, primary_tree_id]
    pub rune_ids: String,
    pub win_rate: f64,
    pub source: String,
    /// Archetype string of the expected lane opponent (e.g. "juggernaut").
    /// NULL = default build used when no matchup-specific variant exists.
    pub opponent_archetype: Option<String>,
    /// Skill-order display text (e.g. "Q→W→E" = max Q first, W second, E last).
    pub skill_order: Option<String>,
    /// JSON-serialised Vec<u32> — [spell1_id, spell2_id]
    pub summoner_spells: Option<String>,
    /// JSON-serialised Vec<u32> — [secondary_tree_id, rune1_id, rune2_id]
    pub secondary_runes: Option<String>,
    /// JSON-serialised Vec<u32> — [offense, flex, defense]
    pub stat_shards: Option<String>,
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn row_from_query(r: &rusqlite::Row<'_>) -> rusqlite::Result<BuildRow> {
    Ok(BuildRow {
        champion_id: r.get(0)?,
        position: r.get(1)?,
        patch_version: r.get(2)?,
        item_ids: r.get(3)?,
        rune_ids: r.get(4)?,
        win_rate: r.get(5)?,
        source: r.get(6)?,
        opponent_archetype: r.get(7)?,
        skill_order: r.get(8)?,
        summoner_spells: r.get(9)?,
        secondary_runes: r.get(10)?,
        stat_shards: r.get(11)?,
    })
}

const SELECT_COLS: &str = "champion_id, position, patch_version, item_ids, rune_ids, win_rate,
                source, opponent_archetype, skill_order, summoner_spells,
                secondary_runes, stat_shards";

/// Default build — no matchup awareness (falls back to NULL opponent_archetype).
pub fn get_build(conn: &Connection, champion_id: i64, position: &str) -> Result<Option<BuildRow>> {
    let sql = format!(
        "SELECT {SELECT_COLS}
         FROM builds
         WHERE champion_id = ?1 AND position = ?2 AND opponent_archetype IS NULL
         ORDER BY cached_at DESC
         LIMIT 1"
    );
    let result = conn.query_row(&sql, params![champion_id, position], row_from_query);
    match result {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Matchup-aware lookup: tries `(champion, position, opponent_archetype)` first,
/// then falls back to the default NULL-archetype build.
pub fn get_build_for_matchup(
    conn: &Connection,
    champion_id: i64,
    position: &str,
    opponent_archetype: &str,
) -> Result<Option<BuildRow>> {
    let sql = format!(
        "SELECT {SELECT_COLS}
         FROM builds
         WHERE champion_id = ?1 AND position = ?2 AND opponent_archetype = ?3
         ORDER BY cached_at DESC
         LIMIT 1"
    );
    let result = conn.query_row(
        &sql,
        params![champion_id, position, opponent_archetype],
        row_from_query,
    );
    match result {
        Ok(row) => return Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => {}
        Err(e) => return Err(e.into()),
    }
    // Fall back to default
    get_build(conn, champion_id, position)
}

pub fn upsert_build(conn: &Connection, row: &BuildRow) -> Result<()> {
    conn.execute(
        "INSERT INTO builds
             (champion_id, position, patch_version, item_ids, rune_ids, win_rate,
              pick_rate, source, opponent_archetype, skill_order, summoner_spells,
              secondary_runes, stat_shards, cached_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0.0, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(champion_id, position, patch_version, source,
                     COALESCE(opponent_archetype, '')) DO UPDATE SET
             item_ids           = excluded.item_ids,
             rune_ids           = excluded.rune_ids,
             win_rate           = excluded.win_rate,
             opponent_archetype = excluded.opponent_archetype,
             skill_order        = excluded.skill_order,
             summoner_spells    = excluded.summoner_spells,
             secondary_runes    = excluded.secondary_runes,
             stat_shards        = excluded.stat_shards,
             cached_at          = excluded.cached_at",
        params![
            row.champion_id,
            row.position,
            row.patch_version,
            row.item_ids,
            row.rune_ids,
            row.win_rate,
            row.source,
            row.opponent_archetype,
            row.skill_order,
            row.summoner_spells,
            row.secondary_runes,
            row.stat_shards,
            now_secs(),
        ],
    )?;
    Ok(())
}
