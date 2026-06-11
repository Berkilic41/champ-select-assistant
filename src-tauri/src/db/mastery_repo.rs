use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
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

// MasteryRow is now a shared pure DTO in `csa-core`. Re-exported so existing paths
// keep working; `top_for_puuid` below still builds it via a struct literal.
pub use csa_core::types::MasteryRow;

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

/// Record a mastery snapshot for progress tracking — but ONLY when the points changed
/// since the most recent snapshot for this champion, so repeated syncs with no play
/// don't bloat the table. Returns `true` when a new row was written.
pub fn record_mastery_snapshot(
    conn: &Connection,
    puuid: &str,
    champion_id: i64,
    mastery_level: i64,
    mastery_points: i64,
    snapshot_at: i64,
) -> Result<bool> {
    let last: Option<i64> = conn
        .query_row(
            "SELECT mastery_points FROM mastery_snapshots
             WHERE puuid = ?1 AND champion_id = ?2
             ORDER BY snapshot_at DESC LIMIT 1",
            params![puuid, champion_id],
            |r| r.get(0),
        )
        .optional()?;
    if last == Some(mastery_points) {
        return Ok(false);
    }
    conn.execute(
        "INSERT INTO mastery_snapshots
             (puuid, champion_id, mastery_points, mastery_level, snapshot_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            puuid,
            champion_id,
            mastery_points,
            mastery_level,
            snapshot_at
        ],
    )?;
    Ok(true)
}

/// Per-champion mastery progress within a time window (training mode).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MasteryProgress {
    pub champion_id: i64,
    /// Points gained in the window (mastery is monotonic → MAX − MIN).
    pub points_gained: i64,
    pub current_points: i64,
    pub current_level: i64,
}

/// Top champions by mastery points gained since `since` (Unix seconds). Only champions
/// with a real gain (> 0) are returned, most progress first. Empty until ≥2 snapshots
/// straddle a gain — honest, never fabricated.
pub fn mastery_progress(
    conn: &Connection,
    puuid: &str,
    since: i64,
) -> Result<Vec<MasteryProgress>> {
    let mut stmt = conn.prepare(
        "SELECT champion_id,
                MAX(mastery_points) - MIN(mastery_points) AS gained,
                MAX(mastery_points) AS current_points,
                MAX(mastery_level)  AS current_level
         FROM mastery_snapshots
         WHERE puuid = ?1 AND snapshot_at >= ?2
         GROUP BY champion_id
         HAVING gained > 0
         ORDER BY gained DESC
         LIMIT 5",
    )?;
    let rows = stmt.query_map(params![puuid, since], |r| {
        Ok(MasteryProgress {
            champion_id: r.get(0)?,
            points_gained: r.get(1)?,
            current_points: r.get(2)?,
            current_level: r.get(3)?,
        })
    })?;
    rows.map(|r| r.map_err(Into::into)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{open_db, run_migrations};
    use tempfile::tempdir;

    fn db() -> Connection {
        let dir = tempdir().unwrap();
        let mut conn = open_db(&dir.path().join("m.db")).unwrap();
        run_migrations(&mut conn).unwrap();
        std::mem::forget(dir);
        conn
    }

    #[test]
    fn snapshot_skips_unchanged_points() {
        let conn = db();
        assert!(record_mastery_snapshot(&conn, "p", 1, 5, 1000, 100).unwrap());
        // Same points again → no new row.
        assert!(!record_mastery_snapshot(&conn, "p", 1, 5, 1000, 200).unwrap());
        // Changed points → new row.
        assert!(record_mastery_snapshot(&conn, "p", 1, 6, 1500, 300).unwrap());
    }

    #[test]
    fn progress_reports_points_gained_in_window() {
        let conn = db();
        // Champ 1: 1000 → 1800 (gained 800). Champ 2: single snapshot (no gain).
        record_mastery_snapshot(&conn, "p", 1, 5, 1000, 100).unwrap();
        record_mastery_snapshot(&conn, "p", 1, 6, 1800, 500).unwrap();
        record_mastery_snapshot(&conn, "p", 2, 4, 900, 100).unwrap();

        let prog = mastery_progress(&conn, "p", 0).unwrap();
        assert_eq!(prog.len(), 1, "only champ with a real gain");
        assert_eq!(prog[0].champion_id, 1);
        assert_eq!(prog[0].points_gained, 800);
        assert_eq!(prog[0].current_points, 1800);
        assert_eq!(prog[0].current_level, 6);

        // Window excluding the early snapshot → no gain visible.
        let recent = mastery_progress(&conn, "p", 400).unwrap();
        assert!(recent.is_empty(), "only one snapshot in window → no gain");
    }
}
