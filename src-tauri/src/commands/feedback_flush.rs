//! Feedback flush command (Feedback Loop — Sprint E, Claude).
//!
//! Pushes the local offline `recommendation_feedback` queue to the cloud using the
//! pure `feedback_sync` policy (idempotency, exponential backoff, terminal states).
//!
//! Correctness guarantees:
//!   * **Network failure never corrupts the queue** — a row is marked synced ONLY on
//!     success; a failure just bumps retry bookkeeping (`resolve_after_send`).
//!   * **PII-free** — rows without a session hash are skipped, never sent raw.
//!   * **Idempotent** — each POST carries a stable dedup key so a retried/duplicated
//!     send is a no-op on the backend (unique index).
//!
//! Triggered by Codex's sync UX (button / background) — NOT during champ-select.

use crate::errors::AppError;
use crate::recommendation::feedback_sync::{
    idempotency_key, is_due, resolve_after_send, FlushState, SendResult,
};
use crate::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;
use ts_rs::TS;

/// Minimum length for a session hash to be accepted (a real hash, not a raw id).
const MIN_HASH_LEN: usize = 16;

/// Outcome of a flush run, for the sync UX.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct FeedbackFlushSummary {
    /// No `DRAFT_BRAIN_API_BASE` configured — nothing was attempted.
    pub offline: bool,
    /// Rows a POST was attempted for.
    pub attempted: u32,
    /// Rows now marked synced.
    pub synced: u32,
    /// Rows whose POST failed (retry bookkeeping bumped, still queued).
    pub failed: u32,
    /// Due rows skipped because they carry no session hash (privacy policy).
    pub skipped_no_hash: u32,
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

struct QueueRow {
    id: i64,
    champion_id: i64,
    champion_key: String,
    feedback: String,
    session_hash: Option<String>,
    model_version: String,
    score: f64,
    payload_json: String,
    created_at: i64,
    state: FlushState,
}

async fn post_feedback(
    client: &reqwest::Client,
    base: &str,
    token: Option<&str>,
    body: &serde_json::Value,
) -> Result<(), String> {
    let url = format!("{}/v1/recommendation-feedback", base.trim_end_matches('/'));
    let mut req = client.post(url).json(body);
    if let Some(t) = token {
        req = req.bearer_auth(t);
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

/// Flush due, unsynced feedback rows to the cloud. No-op (and `offline: true`) when
/// the cloud base URL is not configured.
#[tauri::command]
pub async fn sync_recommendation_feedback(
    state: State<'_, AppState>,
) -> Result<FeedbackFlushSummary, AppError> {
    let base = match std::env::var("DRAFT_BRAIN_API_BASE") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            return Ok(FeedbackFlushSummary {
                offline: true,
                attempted: 0,
                synced: 0,
                failed: 0,
                skipped_no_hash: 0,
            })
        }
    };
    let token = std::env::var("DRAFT_BRAIN_API_TOKEN")
        .ok()
        .filter(|t| !t.trim().is_empty());
    let now = now_secs();

    // Read the queue, then release the lock before any network IO.
    let rows: Vec<QueueRow> = {
        let db = state.db.lock().await;
        let mut stmt = db.prepare(
            "SELECT rowid, champion_id, champion_key, feedback, session_hash, model_version,
                    score, payload_json, created_at, retry_count, next_retry_at
             FROM recommendation_feedback WHERE synced_at IS NULL",
        )?;
        let mapped = stmt.query_map([], |r| {
            Ok(QueueRow {
                id: r.get(0)?,
                champion_id: r.get(1)?,
                champion_key: r.get(2)?,
                feedback: r.get(3)?,
                session_hash: r.get(4)?,
                model_version: r.get(5)?,
                score: r.get(6)?,
                payload_json: r.get(7)?,
                created_at: r.get(8)?,
                state: FlushState {
                    synced_at: None,
                    retry_count: r.get::<_, i64>(9)?.max(0) as u32,
                    last_error: None,
                    next_retry_at: r.get(10)?,
                },
            })
        })?;
        mapped.collect::<Result<Vec<_>, _>>()?
    };

    let client = reqwest::Client::builder().use_rustls_tls().build()?;
    let mut summary = FeedbackFlushSummary {
        offline: false,
        attempted: 0,
        synced: 0,
        failed: 0,
        skipped_no_hash: 0,
    };
    let mut updates: Vec<(i64, FlushState)> = Vec::new();

    for row in rows {
        if !is_due(&row.state, now) {
            continue;
        }
        let user_hash = match row.session_hash.clone() {
            Some(h) if h.trim().len() >= MIN_HASH_LEN => h,
            _ => {
                summary.skipped_no_hash += 1;
                continue;
            }
        };
        summary.attempted += 1;

        let payload: serde_json::Value =
            serde_json::from_str(&row.payload_json).unwrap_or_else(|_| json!({}));
        let key = idempotency_key(
            &user_hash,
            row.champion_id.max(0) as u32,
            &row.feedback,
            row.created_at,
        );
        let body = json!({
            "user_hash": user_hash,
            "champion_id": row.champion_id,
            "champion_key": row.champion_key,
            "feedback": row.feedback,
            "model_version": row.model_version,
            "score": row.score,
            "payload": payload,
            "idempotency_key": key,
        });

        let result = match post_feedback(&client, &base, token.as_deref(), &body).await {
            Ok(()) => {
                summary.synced += 1;
                SendResult::Ok
            }
            Err(e) => {
                summary.failed += 1;
                SendResult::Failed(e)
            }
        };
        updates.push((row.id, resolve_after_send(&row.state, result, now)));
    }

    // Re-acquire the lock to persist the new sync states.
    if !updates.is_empty() {
        let db = state.db.lock().await;
        for (id, st) in &updates {
            db.execute(
                "UPDATE recommendation_feedback
                 SET synced_at = ?1, retry_count = ?2, last_error = ?3, next_retry_at = ?4
                 WHERE rowid = ?5",
                params![
                    st.synced_at,
                    st.retry_count,
                    st.last_error,
                    st.next_retry_at,
                    id
                ],
            )?;
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{open_db, run_migrations};
    use tempfile::tempdir;

    fn memory_db() -> rusqlite::Connection {
        let dir = tempdir().unwrap();
        let mut conn = open_db(&dir.path().join("fb.db")).unwrap();
        run_migrations(&mut conn).unwrap();
        std::mem::forget(dir);
        conn
    }

    fn insert(conn: &rusqlite::Connection, hash: Option<&str>) {
        conn.execute(
            "INSERT INTO recommendation_feedback
                 (champion_id, champion_key, feedback, session_hash, created_at)
             VALUES (238, 'Zed', 'helpful', ?1, 1000)",
            params![hash],
        )
        .unwrap();
    }

    /// The V013 columns round-trip: a resolved sync state persists and a synced row
    /// drops out of the unsynced queue. Mirrors the command's read/update plumbing
    /// (network step excluded — `resolve_after_send` is unit-tested separately).
    #[test]
    fn queue_read_and_state_update_round_trip() {
        let conn = memory_db();
        insert(&conn, Some("0123456789abcdef0123")); // syncable
        insert(&conn, None); // no hash → would be skipped by the command

        // Two rows queued.
        let unsynced: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM recommendation_feedback WHERE synced_at IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(unsynced, 2);

        // Apply a successful flush to the first row.
        let synced = resolve_after_send(
            &FlushState {
                synced_at: None,
                retry_count: 0,
                last_error: None,
                next_retry_at: None,
            },
            SendResult::Ok,
            5_000,
        );
        conn.execute(
            "UPDATE recommendation_feedback
             SET synced_at=?1, retry_count=?2, last_error=?3, next_retry_at=?4
             WHERE session_hash = '0123456789abcdef0123'",
            params![
                synced.synced_at,
                synced.retry_count,
                synced.last_error,
                synced.next_retry_at
            ],
        )
        .unwrap();

        let still_unsynced: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM recommendation_feedback WHERE synced_at IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(still_unsynced, 1, "synced row leaves the queue");
    }

    #[test]
    fn failed_state_persists_retry_metadata() {
        let conn = memory_db();
        insert(&conn, Some("0123456789abcdef0123"));
        let failed = resolve_after_send(
            &FlushState {
                synced_at: None,
                retry_count: 0,
                last_error: None,
                next_retry_at: None,
            },
            SendResult::Failed("HTTP 503".into()),
            5_000,
        );
        conn.execute(
            "UPDATE recommendation_feedback
             SET synced_at=?1, retry_count=?2, last_error=?3, next_retry_at=?4 WHERE rowid=1",
            params![
                failed.synced_at,
                failed.retry_count,
                failed.last_error,
                failed.next_retry_at
            ],
        )
        .unwrap();
        let (rc, err, nra, synced): (i64, Option<String>, Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT retry_count, last_error, next_retry_at, synced_at
                 FROM recommendation_feedback WHERE rowid=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(rc, 1);
        assert_eq!(err.as_deref(), Some("HTTP 503"));
        assert_eq!(nra, Some(5_030));
        assert_eq!(synced, None, "failed row stays queued");
    }
}
