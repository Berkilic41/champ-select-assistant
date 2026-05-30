use crate::errors::AppError;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct AppSettings {
    pub weight_comfort: f32,
    /// Formerly weight_lane_counter — renamed in Sprint E.
    #[serde(alias = "weight_lane_counter")]
    pub weight_matchup: f32,
    pub weight_team_counter: f32,
    pub weight_synergy: f32,
    pub weight_meta: f32,
    pub weight_role_fit: f32,
    pub always_on_top: bool,
    pub window_size: String, // "compact"|"standard"|"wide"
    pub auto_hide_in_game: bool,
    pub sounds_enabled: bool,
    /// UI language: "tr" | "en". Added in Sprint G (i18n).
    #[serde(default = "default_language")]
    pub language: String,
    /// Platform region for LCU sync and Riot API routing (e.g. "tr1", "euw1").
    /// Not a Match-V5 routing value — routing is derived in the backend.
    #[serde(default = "default_platform_region")]
    pub platform_region: String,
}

fn default_language() -> String {
    "tr".to_string()
}

fn default_platform_region() -> String {
    "tr1".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            weight_comfort: 0.25, // sum = 1.00 (was 0.95)
            weight_matchup: 0.25,
            weight_team_counter: 0.15,
            weight_synergy: 0.10,
            weight_meta: 0.15,
            weight_role_fit: 0.10,
            always_on_top: true,
            window_size: "standard".to_string(),
            auto_hide_in_game: false,
            sounds_enabled: false,
            language: default_language(),
            platform_region: default_platform_region(),
        }
    }
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, AppError> {
    let db = state.db.lock().await;
    let json_str: Option<String> = match db.query_row(
        "SELECT value FROM app_config WHERE key = 'settings'",
        [],
        |r| r.get::<_, String>(0),
    ) {
        Ok(s) => Some(s),
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(e) => return Err(AppError::from(e)),
    };

    Ok(json_str
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default())
}

#[tauri::command]
pub async fn save_settings(
    settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let json = serde_json::to_string(&settings)?;
    let db = state.db.lock().await;
    db.execute(
        "INSERT OR REPLACE INTO app_config (key, value) VALUES ('settings', ?1)",
        rusqlite::params![json],
    )?;
    Ok(())
}

#[tauri::command]
pub async fn is_onboarding_complete(state: State<'_, AppState>) -> Result<bool, AppError> {
    let db = state.db.lock().await;
    let val: Option<String> = match db.query_row(
        "SELECT value FROM app_config WHERE key = 'onboarding_complete'",
        [],
        |r| r.get::<_, String>(0),
    ) {
        Ok(s) => Some(s),
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(e) => return Err(AppError::from(e)),
    };
    Ok(val.as_deref() == Some("true"))
}

#[tauri::command]
pub async fn complete_onboarding(state: State<'_, AppState>) -> Result<(), AppError> {
    let db = state.db.lock().await;
    db.execute(
        "INSERT OR REPLACE INTO app_config (key, value) VALUES ('onboarding_complete', 'true')",
        [],
    )?;
    Ok(())
}
