use crate::db::{builds_repo, champion_rates_repo, champion_repo, matchup_repo};
use crate::errors::AppError;
use crate::meta::meraki::{build_rate_rows, fetch_meraki_rates};
use crate::AppState;
use serde::Deserialize;
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
    const SEED: &str = include_str!("../../resources/builds_seed.json");
    let entries: Vec<BuildSeedEntry> = serde_json::from_str(SEED)
        .map_err(|e| AppError::Other(format!("builds_seed.json parse hatası: {e}")))?;

    let count = entries.len();
    let db = state.db.lock().await;
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
        builds_repo::upsert_build(&db, &row)?;
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
    const SEED: &str = include_str!("../../resources/meta/matchup_seed.json");
    let entries: Vec<MatchupSeedEntry> = serde_json::from_str(SEED)
        .map_err(|e| AppError::Other(format!("matchup_seed.json parse hatası: {e}")))?;

    let count = entries.len();
    let db = state.db.lock().await;
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
        matchup_repo::upsert_matchup(&db, &row)?;
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
