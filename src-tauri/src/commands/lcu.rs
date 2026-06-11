use crate::db::{champion_repo, mastery_repo, match_repo, summoner_repo};
use crate::errors::AppError;
use crate::lcu::{
    find_lockfile, parse_lcu_mastery, parse_lcu_match_history, parse_lcu_summoner_name, LcuClient,
};
use crate::riot::client::SummonerInfo;
use crate::AppState;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Serialize)]
pub struct LcuStatus {
    pub connected: bool,
    pub summoner_name: Option<String>,
    pub region: Option<String>,
    pub port: Option<u16>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn connect_lcu(app: AppHandle, state: State<'_, AppState>) -> Result<LcuStatus, String> {
    let lockfile = match crate::lcu::find_lockfile() {
        Ok(lf) => lf,
        Err(e) => {
            return Ok(LcuStatus {
                connected: false,
                summoner_name: None,
                region: None,
                port: None,
                error: Some(e.to_string()),
            });
        }
    };

    let port = lockfile.port;
    let client = match crate::lcu::LcuClient::new(&lockfile) {
        Ok(c) => c,
        Err(e) => {
            return Ok(LcuStatus {
                connected: false,
                summoner_name: None,
                region: None,
                port: Some(port),
                error: Some(format!("Client oluşturulamadı: {}", e)),
            });
        }
    };

    let summoner: serde_json::Value =
        match client.get_raw("/lol-summoner/v1/current-summoner").await {
            Ok(v) => v,
            Err(e) => {
                return Ok(LcuStatus {
                    connected: false,
                    summoner_name: None,
                    region: None,
                    port: Some(port),
                    error: Some(format!("LCU isteği başarısız: {}", e)),
                });
            }
        };

    let game_name = summoner["gameName"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or_else(|| summoner["displayName"].as_str().filter(|s| !s.is_empty()))
        .unwrap_or("Summoner")
        .to_string();
    let tag_line = summoner["tagLine"].as_str().unwrap_or("").to_string();
    // "ELVEDA#SNB" formatında birleştir — frontend parseSummoner ile ayırır
    let summoner_name = if tag_line.is_empty() {
        Some(game_name)
    } else {
        Some(format!("{}#{}", game_name, tag_line))
    };
    // region Sprint 2'de Riot API'den alınacak; şimdilik None
    let region: Option<String> = None;

    *state.lcu_client.lock().await = Some(client.clone());

    let client_arc = Arc::new(client);

    // Stop any running WebSocket watcher before starting the poller.
    {
        let mut cancel_guard = state.lcu_ws_cancel.lock().await;
        if let Some(old) = cancel_guard.take() {
            old.cancel();
        }
    }
    {
        let mut handle_guard = state.lcu_ws_handle.lock().await;
        if let Some(old) = handle_guard.take() {
            old.abort();
        }
    }

    // Cancel and replace any existing poller.
    {
        let mut cancel_guard = state.session_poller_cancel.lock().await;
        if let Some(old) = cancel_guard.take() {
            old.cancel();
        }
    }
    {
        let mut handle_guard = state.session_poller_handle.lock().await;
        if let Some(old) = handle_guard.take() {
            old.abort();
        }
    }

    let poller_cancel = CancellationToken::new();
    let poller_handle = tokio::spawn(crate::lcu::poller::start_session_poller(
        app.clone(),
        client_arc.clone(),
        poller_cancel.clone(),
    ));
    *state.session_poller_cancel.lock().await = Some(poller_cancel);
    *state.session_poller_handle.lock().await = Some(poller_handle);

    // Replace any previous gameflow watcher so they don't accumulate on reconnect.
    {
        let mut cancel_guard = state.gameflow_watcher_cancel.lock().await;
        if let Some(old) = cancel_guard.take() {
            old.cancel();
        }
    }
    let gameflow_cancel = CancellationToken::new();
    tokio::spawn(crate::lcu::poller::start_gameflow_watcher(
        app.clone(),
        client_arc,
        gameflow_cancel.clone(),
    ));
    *state.gameflow_watcher_cancel.lock().await = Some(gameflow_cancel);

    tracing::info!(
        "LCU bağlantısı kuruldu ve poller başlatıldı: {:?}",
        summoner_name
    );

    Ok(LcuStatus {
        connected: true,
        summoner_name,
        region,
        port: Some(port),
        error: None,
    })
}

/// Result of a full LCU-based player data sync.
/// No ts-rs derive — frontend type is defined manually in src/types/riot.ts.
#[derive(Debug, Serialize)]
pub struct LcuPlayerDataSyncResult {
    pub summoner: SummonerInfo,
    pub matches_synced: u32,
    pub matches_skipped: u32,
    pub masteries_synced: u32,
    pub errors: u32,
}

/// Sync match history, mastery, and summoner info from the running League Client
/// without requiring a Riot developer API key.
///
/// If `state.lcu_client` is not yet connected, automatically tries to find the
/// lockfile and create a client.  Returns `AppError::NotConnected` if the
/// League client is not running.
#[tauri::command]
pub async fn sync_lcu_player_data(
    // Platform region from the frontend ("tr1", "euw1", "na1" …).
    // Never pass a Riot Match-V5 routing region ("europe") here.
    region: Option<String>,
    state: State<'_, AppState>,
) -> Result<LcuPlayerDataSyncResult, AppError> {
    // ── 1. Get or auto-create LcuClient ────────────────────────────────────
    let client = {
        let guard = state.lcu_client.lock().await;
        match guard.as_ref() {
            Some(c) => c.clone(),
            None => {
                // Drop the lock before the potentially-blocking lockfile search
                drop(guard);
                let lf = find_lockfile().map_err(|_| AppError::NotConnected)?;
                LcuClient::new(&lf).map_err(|e| AppError::Lcu(e.to_string()))?
            }
        }
    };

    // ── 2. Current summoner ─────────────────────────────────────────────────
    let summoner_json = client
        .get_raw("/lol-summoner/v1/current-summoner")
        .await
        .map_err(|e| AppError::Lcu(format!("current-summoner hatası: {e}")))?;

    let puuid = summoner_json["puuid"]
        .as_str()
        .ok_or_else(|| AppError::Lcu("LCU current-summoner yanıtında puuid yok".to_string()))?
        .to_string();

    let (game_name, tag_line) = parse_lcu_summoner_name(&summoner_json);
    let platform_region = region.filter(|r| !r.is_empty()).unwrap_or_else(|| {
        tracing::warn!("sync_lcu_player_data: region parametresi eksik, 'euw1' kullanılıyor");
        "euw1".to_string()
    });

    let summoner = SummonerInfo {
        puuid: puuid.clone(),
        game_name,
        tag_line,
        region: platform_region,
    };

    // ── 3. Upsert summoner ─────────────────────────────────────────────────
    {
        let db = state.db.lock().await;
        summoner_repo::upsert_summoner(&db, &summoner).map_err(AppError::from)?;
    }

    // ── 4. Match history ───────────────────────────────────────────────────
    let history_path = format!(
        "/lol-match-history/v1/products/lol/{}/matches?begIndex=0&endIndex=20",
        puuid
    );
    let history_json = client
        .get_raw(&history_path)
        .await
        .unwrap_or(serde_json::Value::Null); // non-fatal: skip on error

    let parsed_matches = parse_lcu_match_history(&history_json, &puuid);

    let mut matches_synced = 0u32;
    let mut matches_skipped = 0u32;
    let mut errors = 0u32;

    {
        let db = state.db.lock().await;
        for m in &parsed_matches {
            // Ensure champion row exists (DB FK constraint)
            if champion_repo::get_champion_by_id(&db, m.champion_id)
                .unwrap_or(None)
                .is_none()
            {
                let _ = champion_repo::upsert_champion(
                    &db,
                    m.champion_id,
                    &m.champion_id.to_string(),
                    &format!("Champion {}", m.champion_id),
                    "",
                );
            }

            match match_repo::insert_match(
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
            ) {
                Ok(true) => matches_synced += 1,
                Ok(false) => matches_skipped += 1,
                Err(_) => errors += 1,
            }
        }
    }

    // ── 5. Mastery ─────────────────────────────────────────────────────────
    // Try the local-player endpoint first; fall back to summonerId path on 404.
    let mastery_json = {
        let primary = client
            .get_raw("/lol-champion-mastery/v1/local-player/champion-mastery")
            .await;
        if let Ok(j) = primary {
            j
        } else {
            let summoner_id = summoner_json["summonerId"]
                .as_str()
                .or_else(|| summoner_json["summonerId"].as_i64().map(|_| ""))
                .unwrap_or("");
            if summoner_id.is_empty() {
                serde_json::Value::Null
            } else {
                let path = format!("/lol-champion-mastery/v1/{}/champion-mastery", summoner_id);
                client
                    .get_raw(&path)
                    .await
                    .unwrap_or(serde_json::Value::Null)
            }
        }
    };

    let parsed_mastery = parse_lcu_mastery(&mastery_json);
    let mut masteries_synced = 0u32;

    {
        let db = state.db.lock().await;
        for m in &parsed_mastery {
            if champion_repo::get_champion_by_id(&db, m.champion_id)
                .unwrap_or(None)
                .is_none()
            {
                let _ = champion_repo::upsert_champion(
                    &db,
                    m.champion_id,
                    &m.champion_id.to_string(),
                    &format!("Champion {}", m.champion_id),
                    "",
                );
            }
            match mastery_repo::upsert_mastery(
                &db,
                &puuid,
                m.champion_id,
                m.level,
                m.points,
                m.last_play_time,
            ) {
                Ok(()) => masteries_synced += 1,
                Err(_) => errors += 1,
            }
            // Progress tracking: snapshot when points changed (training mode).
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let _ = mastery_repo::record_mastery_snapshot(
                &db,
                &puuid,
                m.champion_id,
                m.level,
                m.points,
                now,
            );
        }
    }

    tracing::info!(
        "LCU sync tamamlandı: {} maç, {} mastery, {} hata",
        matches_synced,
        masteries_synced,
        errors
    );

    Ok(LcuPlayerDataSyncResult {
        summoner,
        matches_synced,
        matches_skipped,
        masteries_synced,
        errors,
    })
}

#[tauri::command]
pub async fn get_lcu_status(state: State<'_, AppState>) -> Result<LcuStatus, String> {
    let connected = state.lcu_client.lock().await.is_some();
    Ok(LcuStatus {
        connected,
        summoner_name: None,
        region: None,
        port: None,
        error: if connected {
            None
        } else {
            Some("Bağlı değil".to_string())
        },
    })
}
