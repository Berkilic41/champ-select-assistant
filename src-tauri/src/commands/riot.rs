use crate::db::{champion_repo, mastery_repo, match_repo, summoner_repo};
use crate::errors::AppError;
use crate::riot::client::{
    routing_for_region, runtime_client_from_env, ChampionStats, SummonerInfo, SyncResult,
};
use crate::riot::endpoints::{mastery as mastery_ep, matches as match_ep, summoner as summoner_ep};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn sync_riot_player(
    game_name: String,
    tag_line: String,
    region: String,
    state: State<'_, AppState>,
) -> Result<SummonerInfo, AppError> {
    let riot = runtime_client_from_env()
        .ok_or_else(|| AppError::NotConfigured("Riot API key yapılandırılmamış".to_string()))?;

    let info = summoner_ep::get_by_riot_id(riot.as_ref(), &game_name, &tag_line, &region).await?;

    let db = state.db.lock().await;
    summoner_repo::upsert_summoner(&db, &info)?;

    tracing::info!(
        "Summoner synced: {}#{} ({})",
        info.game_name,
        info.tag_line,
        info.puuid
    );
    Ok(info)
}

#[tauri::command]
pub async fn sync_match_history(
    puuid: String,
    count: u8,
    state: State<'_, AppState>,
) -> Result<SyncResult, AppError> {
    let riot = runtime_client_from_env()
        .ok_or_else(|| AppError::NotConfigured("Riot API key yapılandırılmamış".to_string()))?;

    let routing = {
        let db = state.db.lock().await;
        let info = summoner_repo::get_summoner_by_puuid(&db, &puuid)?;
        let region = info.map(|i| i.region).unwrap_or_else(|| "euw1".to_string());
        routing_for_region(&region).to_string()
    }; // DB lock released here

    let ids = match_ep::list_ids(
        riot.as_ref(),
        &puuid,
        &routing,
        Some("ranked"),
        Some(420),
        count.min(20),
    )
    .await?;

    // Phase 1: Fetch all match details without holding the DB lock.
    struct ParsedMatch {
        match_id: String,
        champion_id: i64,
        position: Option<String>,
        win: bool,
        kills: i64,
        deaths: i64,
        assists: i64,
        duration_secs: i64,
        queue_id: i64,
        played_at: i64,
        cs: i64,
    }

    let mut parsed: Vec<ParsedMatch> = Vec::with_capacity(ids.len());
    let mut errors = 0u32;
    let mut skipped = 0u32;

    for id in &ids {
        let detail = match match_ep::get_detail(riot.as_ref(), id, &routing).await {
            Ok(d) => d,
            Err(_) => {
                errors += 1;
                continue;
            }
        };

        let participants = detail["info"]["participants"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        let p = match participants
            .iter()
            .find(|p| p["puuid"].as_str().unwrap_or("") == puuid)
        {
            Some(p) => p.clone(),
            None => {
                skipped += 1;
                continue;
            }
        };

        parsed.push(ParsedMatch {
            match_id: detail["metadata"]["matchId"]
                .as_str()
                .unwrap_or(id)
                .to_string(),
            champion_id: p["championId"].as_i64().unwrap_or(0),
            position: p["teamPosition"].as_str().map(String::from),
            win: p["win"].as_bool().unwrap_or(false),
            kills: p["kills"].as_i64().unwrap_or(0),
            deaths: p["deaths"].as_i64().unwrap_or(0),
            assists: p["assists"].as_i64().unwrap_or(0),
            duration_secs: detail["info"]["gameDuration"].as_i64().unwrap_or(0),
            queue_id: detail["info"]["queueId"].as_i64().unwrap_or(0),
            played_at: detail["info"]["gameStartTimestamp"].as_i64().unwrap_or(0) / 1000,
            // CS = lane minions + neutral monsters (standard Match-V5 participant fields).
            cs: p["totalMinionsKilled"].as_i64().unwrap_or(0)
                + p["neutralMinionsKilled"].as_i64().unwrap_or(0),
        });
    }

    // Phase 2: Single DB lock for all inserts.
    let mut synced = 0u32;
    let db = state.db.lock().await;

    for m in &parsed {
        if champion_repo::get_champion_by_id(&db, m.champion_id)?.is_none() {
            champion_repo::upsert_champion(
                &db,
                m.champion_id,
                &m.champion_id.to_string(),
                &format!("Champion {}", m.champion_id),
                "",
            )?;
        }

        let inserted = match_repo::insert_match(
            &db,
            &m.match_id,
            &puuid,
            m.champion_id,
            m.position.as_deref(),
            m.win,
            m.kills,
            m.deaths,
            m.assists,
            m.duration_secs,
            m.queue_id,
            m.played_at,
            Some(m.cs),
        )?;

        if inserted {
            synced += 1;
        } else {
            skipped += 1;
        }
    }

    Ok(SyncResult {
        synced,
        skipped,
        errors,
    })
}

#[tauri::command]
pub async fn sync_masteries(
    puuid: String,
    state: State<'_, AppState>,
) -> Result<SyncResult, AppError> {
    let riot = runtime_client_from_env()
        .ok_or_else(|| AppError::NotConfigured("Riot API key yapılandırılmamış".to_string()))?;

    let platform = {
        let db = state.db.lock().await;
        summoner_repo::get_summoner_by_puuid(&db, &puuid)?
            .map(|i| i.region)
            .unwrap_or_else(|| "euw1".to_string())
    }; // DB lock released

    let entries = mastery_ep::top_by_puuid(riot.as_ref(), &puuid, &platform, 20).await?;

    let db = state.db.lock().await;
    let mut synced = 0u32;
    let mut errors = 0u32;

    for entry in &entries {
        let champion_id = entry["championId"].as_i64().unwrap_or(0);
        let level = entry["championLevel"].as_i64().unwrap_or(0);
        let points = entry["championPoints"].as_i64().unwrap_or(0);
        let last_play = entry["lastPlayTime"].as_i64();

        if champion_repo::get_champion_by_id(&db, champion_id)?.is_none() {
            champion_repo::upsert_champion(
                &db,
                champion_id,
                &champion_id.to_string(),
                &format!("Champion {champion_id}"),
                "",
            )?;
        }

        match mastery_repo::upsert_mastery(&db, &puuid, champion_id, level, points, last_play) {
            Ok(()) => synced += 1,
            Err(_) => errors += 1,
        }
        // Progress tracking: snapshot when points changed (training mode).
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let _ = mastery_repo::record_mastery_snapshot(&db, &puuid, champion_id, level, points, now);
    }

    Ok(SyncResult {
        synced,
        skipped: 0,
        errors,
    })
}

#[tauri::command]
pub async fn get_player_stats(
    puuid: String,
    state: State<'_, AppState>,
) -> Result<Vec<ChampionStats>, AppError> {
    let db = state.db.lock().await;
    match_repo::player_stats(&db, &puuid).map_err(AppError::from)
}

#[tauri::command]
pub async fn get_masteries(
    puuid: String,
    state: State<'_, AppState>,
) -> Result<Vec<mastery_repo::MasteryRow>, AppError> {
    let db = state.db.lock().await;
    mastery_repo::top_for_puuid(&db, &puuid, 20).map_err(AppError::from)
}

/// Returns the puuid of the most recently synced summoner.
/// Used by champ-select UI to find the active player without lifting
/// summoner state up through React tree.
#[tauri::command]
pub async fn get_active_summoner_puuid(
    state: State<'_, AppState>,
) -> Result<Option<String>, AppError> {
    let db = state.db.lock().await;
    summoner_repo::get_active_puuid(&db).map_err(AppError::from)
}
