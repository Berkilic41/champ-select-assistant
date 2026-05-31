mod commands;
mod db;
mod ddragon;
mod errors;
mod lcu;
mod meta;
mod recommendation;
mod riot;

use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct AppState {
    pub lcu_client: Mutex<Option<lcu::LcuClient>>,
    /// Session HTTP poller handle — `None` when poller is not running.
    pub session_poller_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Cancellation token for the session HTTP poller.
    pub session_poller_cancel: Mutex<Option<tokio_util::sync::CancellationToken>>,
    /// WebSocket champ-select watcher handle.
    pub lcu_ws_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Cancellation token for the active WebSocket champ-select watcher.
    pub lcu_ws_cancel: Mutex<Option<tokio_util::sync::CancellationToken>>,
    pub db: Arc<Mutex<rusqlite::Connection>>,
    pub ddragon: Mutex<ddragon::DdragonCache>,
    pub riot: Option<Arc<riot::RiotClient>>,
    pub items_cache: Mutex<Vec<crate::ddragon::cdragon::ItemData>>,
    pub rune_trees_cache: Mutex<Vec<crate::ddragon::cdragon::RuneTree>>,
    pub draft_iq: Arc<recommendation::draft_iq::DraftKnowledgeBase>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Install a process-default rustls CryptoProvider BEFORE any TLS use.
    // Both `aws-lc-rs` and `ring` are in the dependency tree (aws-lc-rs via our
    // LCU websocket, ring transitively), so rustls 0.23 cannot auto-pick one;
    // without this, reqwest's `use_rustls_tls()` fails on the first HTTPS request
    // (Meraki / DDragon / CDragon) at runtime. Idempotent / harmless if already set.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {}))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Load .env file if present (dev convenience)
            let _ = dotenvy::dotenv();

            let app_data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("./data"));

            std::fs::create_dir_all(&app_data_dir)
                .map_err(|e| format!("App data dizini oluşturulamadı: {e}"))?;

            let db_path = app_data_dir.join("champ-select.db");
            let mut conn =
                db::open_db(&db_path).map_err(|e| format!("Veritabanı açılamadı: {e}"))?;
            db::run_migrations(&mut conn)
                .map_err(|e| format!("Migrationlar çalıştırılamadı: {e}"))?;

            tracing::info!("Veritabanı hazır: {:?}", db_path);

            let draft_iq = Arc::new(
                recommendation::draft_iq::DraftKnowledgeBase::load()
                    .map_err(|e| format!("Draft IQ KB yüklenemedi: {e}"))?,
            );
            tracing::info!(
                "Draft IQ KB yüklendi: {} şampiyon, {} combo",
                draft_iq.archetypes.len(),
                draft_iq.combos.len()
            );

            let riot = if let Ok(proxy_url) = std::env::var("PROXY_URL") {
                tracing::info!("Proxy modu aktif: {proxy_url}");
                Some(Arc::new(riot::RiotClient::new_with_proxy(proxy_url)))
            } else if let Ok(key) = std::env::var("RIOT_API_KEY") {
                tracing::info!("Riot API key yüklendi (doğrudan mod)");
                Some(Arc::new(riot::RiotClient::new(key)))
            } else {
                tracing::warn!("RIOT_API_KEY ve PROXY_URL bulunamadı — Riot komutları devre dışı");
                None
            };

            let state = AppState {
                lcu_client: Mutex::new(None),
                session_poller_handle: Mutex::new(None),
                session_poller_cancel: Mutex::new(None),
                lcu_ws_handle: Mutex::new(None),
                lcu_ws_cancel: Mutex::new(None),
                db: Arc::new(Mutex::new(conn)),
                ddragon: Mutex::new(ddragon::DdragonCache::new()),
                riot,
                items_cache: Mutex::new(Vec::new()),
                rune_trees_cache: Mutex::new(Vec::new()),
                draft_iq,
            };

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connect_lcu,
            commands::get_lcu_status,
            commands::sync_lcu_player_data,
            commands::get_ddragon_version,
            commands::sync_ddragon_champions,
            commands::get_champions,
            commands::sync_riot_player,
            commands::sync_match_history,
            commands::sync_masteries,
            commands::get_player_stats,
            commands::get_masteries,
            commands::get_active_summoner_puuid,
            commands::start_ws_listener,
            commands::get_recommendations,
            commands::get_ban_suggestions,
            commands::sync_cdragon_meta,
            commands::sync_meraki_rates,
            commands::import_builds,
            commands::import_matchups,
            commands::set_always_on_top,
            commands::set_window_size,
            commands::hide_window,
            commands::show_window,
            commands::get_champion_personal_stats,
            commands::hover_champion,
            commands::get_settings,
            commands::save_settings,
            commands::set_window_opacity,
            commands::is_onboarding_complete,
            commands::complete_onboarding,
            commands::get_enemy_champion_pools,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri uygulaması başlatılamadı");
}
