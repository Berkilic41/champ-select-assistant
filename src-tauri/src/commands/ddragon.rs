use crate::db::champion_repo::ChampionRecord;
use crate::ddragon::cdragon::{fetch_items, fetch_runes};
use crate::errors::AppError;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_ddragon_version(state: State<'_, AppState>) -> Result<String, AppError> {
    let cache = state.ddragon.lock().await;
    Ok(cache.current_version().to_string())
}

#[tauri::command]
pub async fn sync_ddragon_champions(state: State<'_, AppState>) -> Result<usize, AppError> {
    sync_ddragon_champions_inner(&state).await
}

pub(crate) async fn sync_ddragon_champions_inner(state: &AppState) -> Result<usize, AppError> {
    // Fetch without holding DB lock
    let (version, champions) = crate::ddragon::fetch_champion_list().await?;
    let count = champions.len();

    state.ddragon.lock().await.version = Some(version.clone());
    tracing::info!("DDragon versiyonu güncellendi: {}", version);

    let db = state.db.lock().await;
    for (id, key, name, title) in &champions {
        crate::db::champion_repo::upsert_champion(&db, *id, key, name, title)?;
    }
    drop(db);

    // Populate item + rune cache in parallel (no DB needed)
    let (items_res, runes_res) = tokio::join!(fetch_items(&version), fetch_runes(&version),);
    match items_res {
        Ok(items) => {
            tracing::info!("Item cache güncellendi: {} item", items.len());
            *state.items_cache.lock().await = items;
        }
        Err(e) => tracing::warn!("Item cache güncellenemedi: {}", e),
    }
    match runes_res {
        Ok(runes) => {
            tracing::info!("Rune cache güncellendi: {} tree", runes.len());
            *state.rune_trees_cache.lock().await = runes;
        }
        Err(e) => tracing::warn!("Rune cache güncellenemedi: {}", e),
    }

    Ok(count)
}

#[tauri::command]
pub async fn get_champions(state: State<'_, AppState>) -> Result<Vec<ChampionRecord>, AppError> {
    let db = state.db.lock().await;
    crate::db::champion_repo::list_all(&db).map_err(AppError::from)
}
