//! Post-game / performance-trends coach command (Faz 5 v1). Reads the player's
//! recent matches from the local `matches` table (read-only), resolves champion
//! keys, and runs the pure trends builder. No LCU/network here; works on data the
//! Riot/LCU sync already stored. Touches no Codex-owned file.

use crate::db::{champion_repo, mastery_repo};
use crate::errors::AppError;
use crate::recommendation::postgame::{build_performance_report, MatchRow, PerformanceReport};
use crate::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;
use ts_rs::TS;

/// How many recent matches feed the trends read.
const RECENT_LIMIT: i64 = 20;

/// One champion's mastery progress for the training-mode "you improved" panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct MasteryProgressEntry {
    pub champion_id: u32,
    pub champion_key: String,
    pub points_gained: u32,
    pub current_points: u32,
    pub current_level: u32,
}

/// Top champions by mastery points gained in the last `days` (training-mode progress).
/// Empty until ≥2 snapshots straddle real play — honest, never fabricated.
#[tauri::command]
pub async fn get_mastery_progress(
    puuid: String,
    days: u32,
    state: State<'_, AppState>,
) -> Result<Vec<MasteryProgressEntry>, AppError> {
    let db = state.db.lock().await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let since = now - (days.max(1) as i64) * 86_400;

    let champ_keys: HashMap<i64, String> = champion_repo::list_all(&db)?
        .into_iter()
        .map(|c| (c.champion_id, c.key))
        .collect();

    let progress = mastery_repo::mastery_progress(&db, &puuid, since)?;
    Ok(progress
        .into_iter()
        .map(|p| MasteryProgressEntry {
            champion_id: u32::try_from(p.champion_id).unwrap_or(0),
            champion_key: champ_keys.get(&p.champion_id).cloned().unwrap_or_default(),
            points_gained: u32::try_from(p.points_gained).unwrap_or(0),
            current_points: u32::try_from(p.current_points).unwrap_or(0),
            current_level: u32::try_from(p.current_level).unwrap_or(0),
        })
        .collect())
}

/// Build a performance-trends report (one main lesson + form / tilt / pool read)
/// from the player's recent stored matches. Honest low-data partial when thin.
#[tauri::command]
pub async fn get_performance_report(
    puuid: String,
    state: State<'_, AppState>,
) -> Result<PerformanceReport, AppError> {
    let db = state.db.lock().await;

    let champ_keys: HashMap<u32, String> = champion_repo::list_all(&db)?
        .into_iter()
        .map(|c| (c.champion_id as u32, c.key))
        .collect();

    // Most-recent-first so streak/form are correct in the pure builder.
    let mut stmt = db.prepare(
        "SELECT champion_id, position, win, kills, deaths, assists, played_at, duration_secs, cs
         FROM matches
         WHERE puuid = ?1
         ORDER BY played_at DESC
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![puuid, RECENT_LIMIT], |r| {
            let champion_id = r.get::<_, i64>(0)? as u32;
            Ok(MatchRow {
                champion_id,
                champion_key: champ_keys.get(&champion_id).cloned().unwrap_or_default(),
                position: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                win: r.get::<_, i64>(2)? != 0,
                kills: r.get::<_, i64>(3)?.max(0) as u32,
                deaths: r.get::<_, i64>(4)?.max(0) as u32,
                assists: r.get::<_, i64>(5)?.max(0) as u32,
                played_at: r.get::<_, i64>(6)?,
                duration_secs: r.get::<_, i64>(7)?.max(0) as u32,
                cs: r.get::<_, Option<i64>>(8)?.map(|v| v.max(0) as u32),
            })
        })?
        .collect::<rusqlite::Result<Vec<MatchRow>>>()?;

    Ok(build_performance_report(&rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{match_repo, open_db, run_migrations};
    use tempfile::tempdir;

    fn memory_db() -> rusqlite::Connection {
        let dir = tempdir().unwrap();
        let mut conn = open_db(&dir.path().join("pg.db")).unwrap();
        run_migrations(&mut conn).unwrap();
        // We exercise the matches→report path, not referential integrity; skip the
        // summoners/champions FK parents the `matches` table would otherwise require.
        conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
        std::mem::forget(dir);
        conn
    }

    /// Mirrors the command's query + pure-build path without a Tauri State.
    fn report_for(conn: &rusqlite::Connection, puuid: &str) -> PerformanceReport {
        let champ_keys: HashMap<u32, String> = champion_repo::list_all(conn)
            .unwrap()
            .into_iter()
            .map(|c| (c.champion_id as u32, c.key))
            .collect();
        let mut stmt = conn
            .prepare(
                "SELECT champion_id, position, win, kills, deaths, assists, played_at, duration_secs, cs
                 FROM matches WHERE puuid = ?1 ORDER BY played_at DESC LIMIT ?2",
            )
            .unwrap();
        let rows = stmt
            .query_map(params![puuid, RECENT_LIMIT], |r| {
                let id = r.get::<_, i64>(0)? as u32;
                Ok(MatchRow {
                    champion_id: id,
                    champion_key: champ_keys.get(&id).cloned().unwrap_or_default(),
                    position: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    win: r.get::<_, i64>(2)? != 0,
                    kills: r.get::<_, i64>(3)?.max(0) as u32,
                    deaths: r.get::<_, i64>(4)?.max(0) as u32,
                    assists: r.get::<_, i64>(5)?.max(0) as u32,
                    played_at: r.get::<_, i64>(6)?,
                    duration_secs: r.get::<_, i64>(7)?.max(0) as u32,
                    cs: r.get::<_, Option<i64>>(8)?.map(|v| v.max(0) as u32),
                })
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<MatchRow>>>()
            .unwrap();
        build_performance_report(&rows)
    }

    #[test]
    fn empty_db_yields_partial_report() {
        let conn = memory_db();
        let r = report_for(&conn, "nobody");
        assert!(r.partial);
        assert_eq!(r.games, 0);
    }

    #[test]
    fn recent_matches_drive_streak_and_winrate() {
        let conn = memory_db();
        // Insert 4 matches; most recent (played_at 400) is a loss, then loss, then 2 wins.
        match_repo::insert_match(
            &conn,
            "m1",
            "p",
            99,
            Some("middle"),
            true,
            5,
            2,
            8,
            1800,
            420,
            100,
            Some(240), // 240 CS / 30 min = 8.0
        )
        .unwrap();
        match_repo::insert_match(
            &conn,
            "m2",
            "p",
            99,
            Some("middle"),
            true,
            6,
            3,
            7,
            1800,
            420,
            200,
            Some(270), // 9.0
        )
        .unwrap();
        match_repo::insert_match(
            &conn,
            "m3",
            "p",
            238,
            Some("middle"),
            false,
            2,
            7,
            3,
            1800,
            420,
            300,
            Some(180), // 6.0
        )
        .unwrap();
        match_repo::insert_match(
            &conn,
            "m4",
            "p",
            238,
            Some("middle"),
            false,
            1,
            8,
            2,
            1800,
            420,
            400,
            Some(150), // 5.0
        )
        .unwrap();

        let r = report_for(&conn, "p");
        assert_eq!(r.games, 4);
        assert_eq!(r.wins, 2);
        assert_eq!(r.loss_streak, 2, "two most-recent are losses");
        assert!(r.main_role == "middle");
        assert!(!r.top_champions.is_empty());
        // End-to-end CS path: (8.0 + 9.0 + 6.0 + 5.0) / 4 = 7.0.
        assert_eq!(r.avg_cs_per_min, Some(7.0));
    }
}
