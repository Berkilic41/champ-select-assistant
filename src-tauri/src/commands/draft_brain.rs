use crate::errors::AppError;
use crate::recommendation::draft_brain::{
    local_rules_model_pack, local_seed_data_pack, DataPack, ModelPack, DRAFT_BRAIN_RULES_VERSION,
};
use crate::recommendation::draft_brain_data::build_local_data_pack;
use crate::AppState;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;
use ts_rs::TS;

const DEFAULT_PACK_TTL_SECS: i64 = 24 * 60 * 60;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationFeedbackInput {
    pub champion_id: u32,
    pub champion_key: String,
    pub feedback: String,
    pub session_hash: Option<String>,
    pub model_version: Option<String>,
    pub score: Option<f32>,
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct RecommendationFeedbackAck {
    pub stored: bool,
    pub synced: bool,
    pub feedback_id: i64,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct PackSyncStatus {
    pub kind: String,
    pub version: String,
    pub source: String,
    pub cached: bool,
    pub online: bool,
    pub confidence: Option<String>,
    pub generated_at: Option<u32>,
    pub message: String,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct DraftBrainQualityReport {
    pub feedback_total: i64,
    pub feedback_unsynced: i64,
    pub model_pack_version: Option<String>,
    pub data_pack_version: Option<String>,
    pub data_pack_confidence: Option<String>,
    pub data_pack_generated_at: Option<u32>,
    pub data_pack_fresh: Option<bool>,
    pub local_rules_version: String,
    pub cloud_configured: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackMetadata {
    confidence: Option<String>,
    generated_at: Option<u32>,
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn count(conn: &rusqlite::Connection, sql: &str) -> Result<i64, AppError> {
    conn.query_row(sql, [], |r| r.get::<_, i64>(0))
        .map_err(AppError::from)
}

fn extract_version(payload: &str, fallback: &str) -> String {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(|v| {
            v.get("version")
                .and_then(|s| s.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| fallback.to_string())
}

fn pack_metadata(payload: &str) -> PackMetadata {
    let parsed = serde_json::from_str::<serde_json::Value>(payload).ok();
    let confidence = parsed
        .as_ref()
        .and_then(|v| v.get("confidence"))
        .and_then(|v| v.as_str())
        .filter(|v| !v.trim().is_empty())
        .map(str::to_string);
    let generated_at = parsed
        .as_ref()
        .and_then(|v| v.get("generated_at"))
        .and_then(|v| v.as_u64())
        .and_then(|v| u32::try_from(v).ok());

    PackMetadata {
        confidence,
        generated_at,
    }
}

fn upsert_pack(
    conn: &rusqlite::Connection,
    kind: &str,
    version: &str,
    payload_json: &str,
    source: &str,
) -> Result<(), AppError> {
    let now = now_secs();
    conn.execute(
        "INSERT INTO draft_brain_packs
             (kind, version, payload_json, source, fetched_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(kind) DO UPDATE SET
             version      = excluded.version,
             payload_json = excluded.payload_json,
             source       = excluded.source,
             fetched_at   = excluded.fetched_at,
             expires_at   = excluded.expires_at",
        params![
            kind,
            version,
            payload_json,
            source,
            now,
            now + DEFAULT_PACK_TTL_SECS,
        ],
    )?;
    Ok(())
}

fn pack_version(conn: &rusqlite::Connection, kind: &str) -> Result<Option<String>, AppError> {
    conn.query_row(
        "SELECT version FROM draft_brain_packs WHERE kind = ?1",
        params![kind],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map_err(AppError::from)
}

fn pack_payload(conn: &rusqlite::Connection, kind: &str) -> Result<Option<String>, AppError> {
    conn.query_row(
        "SELECT payload_json FROM draft_brain_packs WHERE kind = ?1",
        params![kind],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map_err(AppError::from)
}

fn draft_brain_quality_report_from_conn(
    db: &rusqlite::Connection,
    cloud_configured: bool,
    now: i64,
) -> Result<DraftBrainQualityReport, AppError> {
    let feedback_total = count(db, "SELECT COUNT(*) FROM recommendation_feedback")?;
    let feedback_unsynced = count(
        db,
        "SELECT COUNT(*) FROM recommendation_feedback WHERE synced_at IS NULL",
    )?;
    let model_pack_version = pack_version(db, "model_pack")?;
    let data_pack_version = pack_version(db, "data_pack")?;
    let data_pack_metadata = pack_payload(db, "data_pack")?
        .as_deref()
        .map(pack_metadata)
        .unwrap_or(PackMetadata {
            confidence: None,
            generated_at: None,
        });
    let data_pack_fresh = data_pack_metadata
        .generated_at
        .map(|generated_at| now - generated_at as i64 <= DEFAULT_PACK_TTL_SECS);

    let mut notes = Vec::new();
    if !cloud_configured {
        notes.push("DRAFT_BRAIN_API_BASE yok; local rules/data fallback aktif".to_string());
    }
    if model_pack_version.is_none() {
        notes.push(
            "Model pack cache boş; runtime local rules kullanır, sync_model_pack önerilir"
                .to_string(),
        );
    }
    if data_pack_version.is_none() {
        notes.push(
            "Data pack cache boş; runtime local seed kullanır, sync_data_pack önerilir".to_string(),
        );
    }
    if data_pack_version.is_some() && data_pack_metadata.confidence.is_none() {
        notes.push(
            "Data pack confidence yok; eski backend veya eksik pack metadata olabilir".to_string(),
        );
    }
    if data_pack_fresh == Some(false) {
        notes.push("Data pack generated_at 24 saatten eski; sync_data_pack önerilir".to_string());
    }
    if data_pack_metadata.confidence.as_deref() == Some("low") {
        notes.push("Data pack confidence low; local/seed fallback sinyali baskın".to_string());
    }
    if feedback_unsynced > 0 {
        notes.push(format!("{feedback_unsynced} feedback cloud sync bekliyor"));
    }

    Ok(DraftBrainQualityReport {
        feedback_total,
        feedback_unsynced,
        model_pack_version,
        data_pack_version,
        data_pack_confidence: data_pack_metadata.confidence,
        data_pack_generated_at: data_pack_metadata.generated_at,
        data_pack_fresh,
        local_rules_version: DRAFT_BRAIN_RULES_VERSION.to_string(),
        cloud_configured,
        notes,
    })
}

pub(crate) fn load_cached_model_pack(
    conn: &rusqlite::Connection,
) -> Result<Option<ModelPack>, AppError> {
    let Some(payload) = pack_payload(conn, "model_pack")? else {
        return Ok(None);
    };
    Ok(ModelPack::from_json(&payload).ok())
}

pub(crate) fn load_cached_data_pack(
    conn: &rusqlite::Connection,
) -> Result<Option<DataPack>, AppError> {
    let Some(payload) = pack_payload(conn, "data_pack")? else {
        return Ok(None);
    };
    Ok(DataPack::from_json(&payload).ok())
}

pub(crate) fn active_model_pack(conn: &rusqlite::Connection) -> Result<ModelPack, AppError> {
    Ok(load_cached_model_pack(conn)?.unwrap_or_else(local_rules_model_pack))
}

pub(crate) fn active_data_pack(conn: &rusqlite::Connection) -> Result<DataPack, AppError> {
    if let Some(pack) = load_cached_data_pack(conn)? {
        return Ok(pack);
    }

    let Ok((coverage, _fallback_active, _stale)) = super::data_quality::gather_coverage(conn)
    else {
        return Ok(local_seed_data_pack());
    };
    Ok(build_local_data_pack(&coverage, None, None))
}

async fn fetch_cloud_pack(endpoint: &str) -> Result<Option<(String, String)>, AppError> {
    let base = match std::env::var("DRAFT_BRAIN_API_BASE") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => return Ok(None),
    };
    let url = format!(
        "{}/{}",
        base.trim_end_matches('/'),
        endpoint.trim_start_matches('/')
    );
    let client = reqwest::Client::builder().use_rustls_tls().build()?;
    let mut req = client.get(url);
    if let Ok(token) = std::env::var("DRAFT_BRAIN_API_TOKEN") {
        if !token.trim().is_empty() {
            req = req.bearer_auth(token);
        }
    }
    let text = req.send().await?.error_for_status()?.text().await?;
    let version = extract_version(&text, "cloud-unknown");
    Ok(Some((version, text)))
}

#[tauri::command]
pub async fn submit_recommendation_feedback(
    feedback: RecommendationFeedbackInput,
    state: State<'_, AppState>,
) -> Result<RecommendationFeedbackAck, AppError> {
    let payload = feedback.payload.unwrap_or_else(|| serde_json::json!({}));
    let payload_json = serde_json::to_string(&payload)?;
    let model_version = feedback
        .model_version
        .unwrap_or_else(|| DRAFT_BRAIN_RULES_VERSION.to_string());
    let now = now_secs();
    let db = state.db.lock().await;
    db.execute(
        "INSERT INTO recommendation_feedback
             (champion_id, champion_key, feedback, session_hash, model_version,
              score, payload_json, synced_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8)",
        params![
            feedback.champion_id,
            feedback.champion_key,
            feedback.feedback,
            feedback.session_hash,
            model_version,
            feedback.score.unwrap_or(0.0),
            payload_json,
            now,
        ],
    )?;
    Ok(RecommendationFeedbackAck {
        stored: true,
        synced: false,
        feedback_id: db.last_insert_rowid(),
    })
}

#[tauri::command]
pub async fn sync_model_pack(state: State<'_, AppState>) -> Result<PackSyncStatus, AppError> {
    let kind = "model_pack";
    let cloud_result = fetch_cloud_pack("/v1/model-pack/latest").await;
    let (version, payload, source, online, message) = match cloud_result {
        Ok(Some((version, payload))) => {
            let version = ModelPack::from_json(&payload)
                .map(|p| p.version)
                .unwrap_or(version);
            (
                version,
                payload,
                "cloud".to_string(),
                true,
                "Model pack cloud kaynaktan cache'lendi".to_string(),
            )
        }
        Ok(None) => {
            let pack = local_rules_model_pack();
            (
                pack.version.clone(),
                serde_json::to_string(&pack)?,
                "local_rules".to_string(),
                false,
                "Cloud ayarı yok; local rules model pack cache'lendi".to_string(),
            )
        }
        Err(err) => {
            let pack = local_rules_model_pack();
            (
                pack.version.clone(),
                serde_json::to_string(&pack)?,
                "local_rules".to_string(),
                false,
                format!("Cloud model pack alınamadı; local fallback aktif: {err}"),
            )
        }
    };
    let db = state.db.lock().await;
    upsert_pack(&db, kind, &version, &payload, &source)?;
    let metadata = pack_metadata(&payload);
    Ok(PackSyncStatus {
        kind: kind.to_string(),
        version,
        source,
        cached: true,
        online,
        confidence: metadata.confidence,
        generated_at: metadata.generated_at,
        message,
    })
}

#[tauri::command]
pub async fn sync_data_pack(
    patch: Option<String>,
    region: Option<String>,
    state: State<'_, AppState>,
) -> Result<PackSyncStatus, AppError> {
    let kind = "data_pack";
    let query = match (patch.as_deref(), region.as_deref()) {
        (Some(p), Some(r)) => format!("?patch={p}&region={r}"),
        (Some(p), None) => format!("?patch={p}"),
        (None, Some(r)) => format!("?region={r}"),
        (None, None) => String::new(),
    };
    let endpoint = format!("/v1/data-pack/latest{query}");
    let cloud_result = fetch_cloud_pack(&endpoint).await;
    let (version, payload, source, online, message) = match cloud_result {
        Ok(Some((version, payload))) => (
            version,
            payload,
            "cloud".to_string(),
            true,
            "Data pack cloud kaynaktan cache'lendi".to_string(),
        ),
        Ok(None) => {
            let pack = local_seed_data_pack();
            (
                pack.version.clone(),
                serde_json::to_string(&pack)?,
                "local_seed".to_string(),
                false,
                "Cloud ayarı yok; local seed data pack cache'lendi".to_string(),
            )
        }
        Err(err) => {
            let pack = local_seed_data_pack();
            (
                pack.version.clone(),
                serde_json::to_string(&pack)?,
                "local_seed".to_string(),
                false,
                format!("Cloud data pack alınamadı; local fallback aktif: {err}"),
            )
        }
    };
    let db = state.db.lock().await;
    upsert_pack(&db, kind, &version, &payload, &source)?;
    let metadata = pack_metadata(&payload);
    Ok(PackSyncStatus {
        kind: kind.to_string(),
        version,
        source,
        cached: true,
        online,
        confidence: metadata.confidence,
        generated_at: metadata.generated_at,
        message,
    })
}

#[tauri::command]
pub async fn get_draft_brain_quality_report(
    state: State<'_, AppState>,
) -> Result<DraftBrainQualityReport, AppError> {
    let db = state.db.lock().await;
    let cloud_configured = std::env::var("DRAFT_BRAIN_API_BASE")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    draft_brain_quality_report_from_conn(&db, cloud_configured, now_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{open_db, run_migrations};
    use crate::recommendation::draft_brain::LOCAL_SEED_DATA_PACK_VERSION;
    use tempfile::tempdir;

    fn draft_brain_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("memory db");
        conn.execute_batch(include_str!(
            "../../migrations/V012__draft_brain_feedback.sql"
        ))
        .expect("migration");
        conn
    }

    fn cloud_data_pack_payload(confidence: Option<&str>, generated_at: u32) -> String {
        let mut payload = serde_json::json!({
            "version": "cloud-data-v1",
            "patch": "16.10",
            "region": "tr1",
            "sources": ["cloud_postgres", "riot_match_v5"],
            "quality": {
                "rates": 172,
                "matchups": 1200,
                "builds": 172,
                "feedback": 42,
                "draft_samples": 84
            },
            "fallback": false,
            "generated_at": generated_at
        });
        if let Some(confidence) = confidence {
            payload["confidence"] = serde_json::Value::String(confidence.to_string());
        }
        payload.to_string()
    }

    #[test]
    fn pack_metadata_extracts_confidence_and_generated_at() {
        let payload = serde_json::json!({
            "version": "data-pack-test",
            "confidence": "medium",
            "generated_at": 1_780_358_400u32
        })
        .to_string();

        assert_eq!(
            pack_metadata(&payload),
            PackMetadata {
                confidence: Some("medium".to_string()),
                generated_at: Some(1_780_358_400),
            }
        );
    }

    #[test]
    fn quality_report_marks_data_pack_freshness_boundary() {
        let conn = draft_brain_conn();
        let now = 1_800_000_000i64;
        let fresh_generated_at = (now - 23 * 60 * 60) as u32;
        upsert_pack(
            &conn,
            "data_pack",
            "cloud-data-v1",
            &cloud_data_pack_payload(Some("high"), fresh_generated_at),
            "cloud",
        )
        .expect("fresh data pack insert");

        let fresh_report =
            draft_brain_quality_report_from_conn(&conn, true, now).expect("fresh report");
        assert_eq!(fresh_report.data_pack_fresh, Some(true));

        let stale_generated_at = (now - 25 * 60 * 60) as u32;
        upsert_pack(
            &conn,
            "data_pack",
            "cloud-data-v1",
            &cloud_data_pack_payload(Some("high"), stale_generated_at),
            "cloud",
        )
        .expect("stale data pack insert");

        let stale_report =
            draft_brain_quality_report_from_conn(&conn, true, now).expect("stale report");
        assert_eq!(stale_report.data_pack_fresh, Some(false));
        assert!(stale_report
            .notes
            .iter()
            .any(|note| note.contains("24 saatten eski")));
    }

    #[test]
    fn quality_report_passes_data_pack_confidence_through() {
        let conn = draft_brain_conn();
        let now = 1_800_000_000i64;
        upsert_pack(
            &conn,
            "data_pack",
            "cloud-data-v1",
            &cloud_data_pack_payload(Some("high"), now as u32),
            "cloud",
        )
        .expect("data pack insert");

        let report =
            draft_brain_quality_report_from_conn(&conn, true, now).expect("quality report");

        assert_eq!(report.data_pack_confidence.as_deref(), Some("high"));
        assert_eq!(report.data_pack_generated_at, Some(now as u32));
    }

    #[test]
    fn quality_report_allows_legacy_cloud_pack_without_confidence() {
        let conn = draft_brain_conn();
        let now = 1_800_000_000i64;
        upsert_pack(
            &conn,
            "data_pack",
            "legacy-cloud-data-v1",
            &cloud_data_pack_payload(None, now as u32),
            "cloud",
        )
        .expect("legacy data pack insert");

        let report =
            draft_brain_quality_report_from_conn(&conn, true, now).expect("quality report");

        assert_eq!(report.data_pack_confidence, None);
        assert_eq!(report.data_pack_fresh, Some(true));
        assert!(report
            .notes
            .iter()
            .any(|note| note.contains("confidence yok")));
    }

    #[test]
    fn quality_report_marks_local_fallback_low_confidence() {
        let conn = draft_brain_conn();
        let now = 1_800_000_000i64;
        let pack = local_seed_data_pack();
        upsert_pack(
            &conn,
            "data_pack",
            &pack.version,
            &serde_json::to_string(&pack).expect("local pack json"),
            "local_seed",
        )
        .expect("local data pack insert");

        let report =
            draft_brain_quality_report_from_conn(&conn, false, now).expect("quality report");

        assert_eq!(report.data_pack_confidence.as_deref(), Some("low"));
        assert_eq!(report.data_pack_fresh, None);
        assert!(report
            .notes
            .iter()
            .any(|note| note.contains("confidence low")));
    }

    #[test]
    fn cached_pack_loaders_parse_valid_payloads() {
        let conn = rusqlite::Connection::open_in_memory().expect("memory db");
        conn.execute_batch(include_str!(
            "../../migrations/V012__draft_brain_feedback.sql"
        ))
        .expect("migration");

        let model = local_rules_model_pack();
        let data = local_seed_data_pack();
        upsert_pack(
            &conn,
            "model_pack",
            &model.version,
            &serde_json::to_string(&model).expect("model json"),
            "test",
        )
        .expect("model insert");
        upsert_pack(
            &conn,
            "data_pack",
            &data.version,
            &serde_json::to_string(&data).expect("data json"),
            "test",
        )
        .expect("data insert");

        assert_eq!(
            load_cached_model_pack(&conn)
                .expect("model load")
                .expect("model")
                .version,
            DRAFT_BRAIN_RULES_VERSION
        );
        assert_eq!(
            load_cached_data_pack(&conn)
                .expect("data load")
                .expect("data")
                .version,
            LOCAL_SEED_DATA_PACK_VERSION
        );
    }

    #[test]
    fn active_packs_fallback_when_cache_empty() {
        let conn = rusqlite::Connection::open_in_memory().expect("memory db");
        conn.execute_batch(include_str!(
            "../../migrations/V012__draft_brain_feedback.sql"
        ))
        .expect("migration");

        assert_eq!(
            active_model_pack(&conn).expect("model").version,
            DRAFT_BRAIN_RULES_VERSION
        );
        assert_eq!(
            active_data_pack(&conn).expect("data").version,
            LOCAL_SEED_DATA_PACK_VERSION
        );
    }

    #[test]
    fn active_data_pack_uses_local_coverage_when_cache_empty() {
        let dir = tempdir().expect("tempdir");
        let mut conn = open_db(&dir.path().join("coverage.db")).expect("db");
        run_migrations(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO champions (champion_id, key, name, title, cached_at)
             VALUES (1, 'Annie', 'Annie', 'the Dark Child', 1)",
            [],
        )
        .expect("champion");
        conn.execute(
            "INSERT INTO builds
                 (champion_id, position, patch_version, item_ids, rune_ids,
                  win_rate, pick_rate, source, cached_at)
             VALUES (1, 'middle', '16.10', '1,2,3', '4,5,6', 0.52, 0.05, 'test', 1)",
            [],
        )
        .expect("build");
        conn.execute(
            "INSERT INTO champion_matchups
                 (champion_id, opponent_id, position, games, wins, win_rate, source, patch_version, cached_at)
             VALUES (1, 2, 'middle', 100, 55, 0.55, 'test', '16.10', 1)",
            [],
        )
        .expect("matchup");

        let pack = active_data_pack(&conn).expect("pack");

        assert_eq!(pack.quality.builds, 1);
        assert_eq!(pack.quality.matchups, 1);
        assert!(pack.fallback);
    }
}
