use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::time::{interval, Duration};
use tokio_util::sync::CancellationToken;

use super::client::LcuClient;
use super::session::parse_session;

/// Polls `/lol-champ-select/v1/session` every second.
/// Emits `champ-select-session` with the parsed state, or `null` when the
/// endpoint returns an error (i.e. the player is not in champ-select).
/// Max ardışık hata sayısı — bu kadar 404/hata alınca LoL client kapanmış demektir.
const MAX_CONSECUTIVE_ERRORS: u32 = 5;

/// Polls `/lol-gameflow/v1/gameflow-phase` every 2 seconds.
/// Emits `gameflow-phase` event when the phase changes. Stops on cancel (new
/// connection replaces it) or after too many consecutive errors (League closed),
/// so watchers never accumulate.
pub async fn start_gameflow_watcher(
    app: AppHandle,
    client: Arc<LcuClient>,
    cancel: CancellationToken,
) {
    let mut ticker = interval(Duration::from_millis(2_000));
    let mut last_phase = String::new();
    let mut consecutive_errors = 0u32;
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                tracing::info!("Gameflow watcher: iptal sinyali alındı, duruluyor");
                break;
            }
            _ = ticker.tick() => {}
        }
        match client.get_raw("/lol-gameflow/v1/gameflow-phase").await {
            Ok(json) => {
                consecutive_errors = 0;
                let phase = json.as_str().unwrap_or("").to_string();
                if phase != last_phase {
                    last_phase = phase.clone();
                    let _ = app.emit("gameflow-phase", &phase);
                    tracing::info!("Gameflow phase: {}", phase);
                }
            }
            Err(_) => {
                consecutive_errors += 1;
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    tracing::info!("Gameflow watcher: LoL client kapalı, duruluyor");
                    break;
                }
            }
        }
    }
}

pub async fn start_session_poller(
    app: AppHandle,
    client: Arc<LcuClient>,
    cancel: CancellationToken,
) {
    let mut ticker = interval(Duration::from_millis(1_000));
    let mut consecutive_errors = 0u32;
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                tracing::info!("Poller: iptal sinyali alındı, duruluyor");
                break;
            }
            _ = ticker.tick() => {}
        }
        match client.get_raw("/lol-champ-select/v1/session").await {
            Ok(json) => {
                consecutive_errors = 0;
                if let Some(state) = parse_session(&json) {
                    let _ = app.emit("champ-select-session", &state);
                } else {
                    let _ = app.emit("champ-select-session", serde_json::Value::Null);
                }
            }
            Err(err) => {
                consecutive_errors += 1;
                let _ = app.emit("champ-select-session", serde_json::Value::Null);
                // Distinct, observable error signal: lets the UI tell a real LCU
                // failure apart from "not in champ-select" (both clear the session).
                tracing::warn!("Poller LCU hatası ({consecutive_errors}): {err}");
                let _ = app.emit("lcu-error", consecutive_errors);
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    tracing::info!(
                        "Poller: {} ardışık hata — LoL client kapanmış, duruluyor",
                        MAX_CONSECUTIVE_ERRORS
                    );
                    break;
                }
            }
        }
    }
}
