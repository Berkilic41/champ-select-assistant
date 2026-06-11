use crate::db::{builds_repo, champion_rates_repo, champion_repo, matchup_repo};
use crate::errors::AppError;
use crate::meta::meraki::{build_rate_rows, fetch_meraki_rates};
use crate::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Deserialize)]
struct BuildSeedEntry {
    champion_id: i64,
    position: String,
    patch_version: String,
    item_ids: Vec<u32>,
    rune_ids: Vec<u32>,
    win_rate: f64,
    #[allow(dead_code)]
    pick_rate: f64,
    source: String,
    opponent_archetype: Option<String>,
    #[serde(default)]
    skill_order: Option<String>,
    #[serde(default)]
    summoner_spells: Option<Vec<u32>>,
    #[serde(default)]
    secondary_runes: Option<Vec<u32>>,
    #[serde(default)]
    stat_shards: Option<Vec<u32>>,
}

/// Import the bundled seed builds into the `builds` table (V004).
/// Safe to call multiple times — upserts on conflict.
#[tauri::command]
pub async fn import_builds(state: State<'_, AppState>) -> Result<usize, AppError> {
    let db = state.db.lock().await;
    import_builds_seed(&db)
}

pub(crate) fn import_builds_seed(db: &Connection) -> Result<usize, AppError> {
    const SEED: &str = include_str!("../../resources/builds_seed.json");
    let entries: Vec<BuildSeedEntry> = serde_json::from_str(SEED)
        .map_err(|e| AppError::Other(format!("builds_seed.json parse hatası: {e}")))?;

    let count = entries.len();
    for entry in &entries {
        let row = builds_repo::BuildRow {
            champion_id: entry.champion_id,
            position: entry.position.clone(),
            patch_version: entry.patch_version.clone(),
            item_ids: serde_json::to_string(&entry.item_ids).unwrap_or_default(),
            rune_ids: serde_json::to_string(&entry.rune_ids).unwrap_or_default(),
            win_rate: entry.win_rate,
            source: entry.source.clone(),
            opponent_archetype: entry.opponent_archetype.clone(),
            skill_order: entry.skill_order.clone(),
            summoner_spells: entry
                .summoner_spells
                .as_ref()
                .map(|v| serde_json::to_string(v).unwrap_or_default()),
            secondary_runes: entry
                .secondary_runes
                .as_ref()
                .map(|v| serde_json::to_string(v).unwrap_or_default()),
            stat_shards: entry
                .stat_shards
                .as_ref()
                .map(|v| serde_json::to_string(v).unwrap_or_default()),
        };
        builds_repo::upsert_build(db, &row)?;
    }

    tracing::info!("Build seed içe aktarıldı: {} satır", count);
    Ok(count)
}

#[derive(Deserialize)]
struct MatchupSeedEntry {
    champion_id: i64,
    opponent_id: i64,
    position: String,
    games: i64,
    wins: i64,
    win_rate: f64,
    patch_version: String,
    source: String,
}

/// Import the bundled matchup seed into `champion_matchups` (V006).
/// Safe to call multiple times — upserts on conflict.
#[tauri::command]
pub async fn import_matchups(state: State<'_, AppState>) -> Result<usize, AppError> {
    let db = state.db.lock().await;
    import_matchups_seed(&db)
}

pub(crate) fn import_matchups_seed(db: &Connection) -> Result<usize, AppError> {
    const SEED: &str = include_str!("../../resources/meta/matchup_seed.json");
    let entries: Vec<MatchupSeedEntry> = serde_json::from_str(SEED)
        .map_err(|e| AppError::Other(format!("matchup_seed.json parse hatası: {e}")))?;

    let count = entries.len();
    for entry in &entries {
        let row = matchup_repo::MatchupRow {
            champion_id: entry.champion_id,
            opponent_id: entry.opponent_id,
            position: entry.position.clone(),
            games: entry.games,
            wins: entry.wins,
            win_rate: entry.win_rate,
            source: entry.source.clone(),
            patch_version: entry.patch_version.clone(),
        };
        matchup_repo::upsert_matchup(db, &row)?;
    }

    tracing::info!("Matchup seed içe aktarıldı: {} satır", count);
    Ok(count)
}

/// Fetch champion win/pick/ban rates from Meraki Analytics and persist to the
/// `champion_rates` table (V008).  The HTTP fetch happens without holding the
/// DB lock to avoid starving other commands during the network round-trip.
/// Returns the number of rows written.
#[tauri::command]
pub async fn sync_meraki_rates(state: State<'_, AppState>) -> Result<usize, AppError> {
    sync_meraki_rates_inner(&state).await
}

pub(crate) async fn sync_meraki_rates_inner(state: &AppState) -> Result<usize, AppError> {
    // Phase 1: read champion list (short DB lock)
    let all_champions = {
        let db = state.db.lock().await;
        champion_repo::list_all(&db)?
    };

    // Phase 2: HTTP fetch + transform (no DB lock held)
    let meraki = fetch_meraki_rates().await?;
    let rows = build_rate_rows(&meraki, &all_champions);

    // Phase 3: batch upsert (short DB lock)
    let count = rows.len();
    {
        let db = state.db.lock().await;
        for row in &rows {
            champion_rates_repo::upsert_rate(&db, row)?;
        }
    }

    tracing::info!(
        "Meraki meta sync tamamlandı: patch={}, {} satır",
        meraki.patch,
        count
    );
    Ok(count)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DataQualityPositionCount {
    pub position: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DataQualityTargets {
    pub champion_target: usize,
    pub matchup_target: usize,
    pub build_primary_role_target: usize,
    pub meta_role_coverage_target: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DataQualityReport {
    pub champions: usize,
    pub draft_iq_champions: usize,
    pub combos: usize,
    pub builds: usize,
    pub matchups: usize,
    pub meta_rates: usize,
    pub build_positions: Vec<DataQualityPositionCount>,
    pub matchup_positions: Vec<DataQualityPositionCount>,
    pub meta_positions: Vec<DataQualityPositionCount>,
    pub targets: DataQualityTargets,
    pub notes: Vec<String>,
}

fn count_table(conn: &rusqlite::Connection, table: &str) -> Result<usize, AppError> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    let count = conn.query_row(&sql, [], |r| r.get::<_, i64>(0))?;
    Ok(count.max(0) as usize)
}

fn count_by_position(
    conn: &rusqlite::Connection,
    table: &str,
) -> Result<Vec<DataQualityPositionCount>, AppError> {
    let sql = format!("SELECT position, COUNT(*) FROM {table} GROUP BY position ORDER BY position");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |r| {
        Ok(DataQualityPositionCount {
            position: r.get::<_, String>(0)?,
            count: r.get::<_, i64>(1)?.max(0) as usize,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Read-only coverage snapshot for the iTero-killer data layer: static Draft IQ,
/// seeded builds/matchups, and live meta rows. This is intentionally conservative
/// and never fetches network data; `refresh` commands populate the tables.
#[tauri::command]
pub async fn get_data_quality_report(
    state: State<'_, AppState>,
) -> Result<DataQualityReport, AppError> {
    let db = state.db.lock().await;
    let champions = count_table(&db, "champions")?;
    let builds = count_table(&db, "builds")?;
    let matchups = count_table(&db, "champion_matchups")?;
    let meta_rates = count_table(&db, "champion_rates")?;
    let build_positions = count_by_position(&db, "builds")?;
    let matchup_positions = count_by_position(&db, "champion_matchups")?;
    let meta_positions = count_by_position(&db, "champion_rates")?;

    let draft_iq = state.draft_iq.clone();
    let mut notes = Vec::new();
    if matchups < 1_000 {
        notes.push(format!("Matchup coverage hedefin altında: {matchups}/1000"));
    }
    if builds < 172 {
        notes.push(format!(
            "Build coverage hedefin altında: {builds}/172 primary role"
        ));
    }
    if meta_rates == 0 {
        notes.push("Canlı meta tablosu boş; Meraki/source sync çalıştırılmalı".to_string());
    }
    if notes.is_empty() {
        notes.push("Veri katmanı hedefleri karşılıyor".to_string());
    }

    Ok(DataQualityReport {
        champions,
        draft_iq_champions: draft_iq.archetypes.len(),
        combos: draft_iq.combos.len(),
        builds,
        matchups,
        meta_rates,
        build_positions,
        matchup_positions,
        meta_positions,
        targets: DataQualityTargets {
            champion_target: 172,
            matchup_target: 1_000,
            build_primary_role_target: 172,
            meta_role_coverage_target: 0.95,
        },
        notes,
    })
}
