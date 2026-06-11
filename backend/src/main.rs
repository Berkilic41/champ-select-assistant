use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{
    env,
    net::SocketAddr,
    time::{SystemTime, UNIX_EPOCH},
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db: PgPool,
}

#[derive(Debug, Deserialize)]
struct PackQuery {
    patch: Option<String>,
    region: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DraftSampleInput {
    user_hash: String,
    patch: String,
    region: String,
    queue_id: i32,
    payload: Value,
}

#[derive(Debug, Deserialize)]
struct FeedbackInput {
    user_hash: String,
    champion_id: i32,
    champion_key: String,
    feedback: String,
    model_version: String,
    score: f32,
    payload: Value,
    /// Stable dedup key from the client (`feedback_sync::idempotency_key`). Optional
    /// for backward-compat; when present a duplicate POST is a no-op (unique index).
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MatchOutcomeInput {
    user_hash: String,
    match_id_hash: String,
    champion_id: i32,
    position: String,
    win: bool,
    payload: Value,
}

#[derive(Debug, Serialize)]
struct Ack {
    ok: bool,
    id: Uuid,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let database_url = env::var("DATABASE_URL")?;
    let db = PgPoolOptions::new()
        .max_connections(8)
        .connect(&database_url)
        .await?;

    let app = Router::new()
        .route("/v1/health", get(health))
        .route("/v1/model-pack/latest", get(model_pack_latest))
        .route("/v1/data-pack/latest", get(data_pack_latest))
        .route("/v1/draft-samples", post(draft_sample))
        .route("/v1/recommendation-feedback", post(recommendation_feedback))
        .route("/v1/match-outcomes", post(match_outcome))
        .route("/v1/data-quality", get(data_quality))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(AppState { db });

    let addr: SocketAddr = env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
        .parse()?;
    tracing::info!("draft brain backend listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<Value> {
    Json(json!({ "ok": true }))
}

async fn model_pack_latest(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let row = sqlx::query_scalar::<_, Value>(
        "SELECT payload FROM model_packs WHERE active = true ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(row.unwrap_or_else(|| {
        json!({
            "version": "draft-brain-rules-v1",
            "weights": {
                "comfort": 0.20,
                "matchup": 0.25,
                "team_counter": 0.15,
                "synergy": 0.10,
                "meta": 0.15,
                "role_fit": 0.10,
                "risk": 0.05
            },
            "fallback": true
        })
    })))
}

async fn data_pack_latest(
    State(state): State<AppState>,
    Query(q): Query<PackQuery>,
) -> Result<Json<Value>, StatusCode> {
    let patch = q.patch.unwrap_or_else(|| "latest".to_string());
    let region = q.region.unwrap_or_else(|| "global".to_string());
    let quality = quality_counts(&state.db).await?;
    let matchups = quality["matchups"].as_i64().unwrap_or(0);
    // Carry `confidence` on the pack itself so the client `DataPack.confidence`
    // (and the recommendation badge) reflects cloud quality, not just the local pack.
    let build_champions = scalar_i64(
        &state.db,
        "SELECT COUNT(DISTINCT champion_id) FROM champion_builds",
    )
    .await?;
    let confidence = confidence_band(
        quality["rates"].as_i64().unwrap_or(0),
        matchups,
        quality["builds"].as_i64().unwrap_or(0),
        build_champions,
    );
    Ok(Json(json!({
        "version": format!("data-pack-{patch}-{region}"),
        "patch": patch,
        "region": region,
        "sources": ["cloud_postgres", "riot_match_v5", "manual_seed"],
        "quality": quality,
        "fallback": matchups == 0,
        "confidence": confidence,
        "generated_at": now_epoch_secs()
    })))
}

async fn draft_sample(
    State(state): State<AppState>,
    Json(input): Json<DraftSampleInput>,
) -> Result<Json<Ack>, StatusCode> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO draft_samples (id, user_hash, patch, region, queue_id, payload)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(input.user_hash)
    .bind(input.patch)
    .bind(input.region)
    .bind(input.queue_id)
    .bind(input.payload)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(Ack { ok: true, id }))
}

/// Canonical verdict vocabulary, shared with the client parser + frontend union
/// (src/types/feedback-vocabulary.json) so ingestion can never drift from them.
const FEEDBACK_VOCABULARY: &str = include_str!("../../src/types/feedback-vocabulary.json");

/// True when `verdict` is one of the canonical tokens.
fn verdict_is_canonical(verdict: &str) -> bool {
    serde_json::from_str::<Value>(FEEDBACK_VOCABULARY)
        .ok()
        .and_then(|v| {
            v["verdicts"]
                .as_array()
                .map(|arr| arr.iter().any(|x| x.as_str() == Some(verdict)))
        })
        .unwrap_or(false)
}

/// Validate an ingestion payload: canonical verdict, a hashed (not raw) user_hash,
/// a well-formed champion, and a finite score. Privacy policy: the server must only
/// ever receive a hashed user/session id — a raw PUUID / summoner name is rejected
/// by the length floor (a SHA-256 hex is 64 chars; raw names are short).
fn validate_feedback(input: &FeedbackInput) -> Result<(), &'static str> {
    if input.user_hash.trim().len() < 16 {
        return Err("user_hash must be a hash (>= 16 chars), never a raw identity");
    }
    if input.champion_id <= 0 {
        return Err("champion_id must be positive");
    }
    if input.champion_key.trim().is_empty() {
        return Err("champion_key is required");
    }
    if !verdict_is_canonical(&input.feedback) {
        return Err("feedback verdict is not in the canonical vocabulary");
    }
    if !input.score.is_finite() {
        return Err("score must be finite");
    }
    Ok(())
}

async fn recommendation_feedback(
    State(state): State<AppState>,
    Json(input): Json<FeedbackInput>,
) -> Result<Json<Ack>, StatusCode> {
    validate_feedback(&input).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
    let id = Uuid::new_v4();
    // ON CONFLICT on the dedup key makes a re-sent row a no-op (NULL keys never
    // conflict, so legacy clients without a key still insert normally).
    sqlx::query(
        "INSERT INTO recommendation_feedback
             (id, user_hash, champion_id, champion_key, feedback, model_version, score, payload, idempotency_key)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (idempotency_key) DO NOTHING",
    )
    .bind(id)
    .bind(input.user_hash)
    .bind(input.champion_id)
    .bind(input.champion_key)
    .bind(input.feedback)
    .bind(input.model_version)
    .bind(input.score)
    .bind(input.payload)
    .bind(input.idempotency_key)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(Ack { ok: true, id }))
}

async fn match_outcome(
    State(state): State<AppState>,
    Json(input): Json<MatchOutcomeInput>,
) -> Result<Json<Ack>, StatusCode> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO match_outcomes
             (id, user_hash, match_id_hash, champion_id, position, win, payload)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id)
    .bind(input.user_hash)
    .bind(input.match_id_hash)
    .bind(input.champion_id)
    .bind(input.position)
    .bind(input.win)
    .bind(input.payload)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(Ack { ok: true, id }))
}

async fn data_quality(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    // Target roster size (shared with the client's coverage ratios).
    const TARGET_CHAMPIONS: f64 = 172.0;

    let counts = quality_counts(&state.db).await?;
    let rates = counts["rates"].as_i64().unwrap_or(0);
    let matchups = counts["matchups"].as_i64().unwrap_or(0);
    let builds = counts["builds"].as_i64().unwrap_or(0);

    let build_champions = scalar_i64(
        &state.db,
        "SELECT COUNT(DISTINCT champion_id) FROM champion_builds",
    )
    .await?;
    let rate_champions = scalar_i64(
        &state.db,
        "SELECT COUNT(DISTINCT champion_id) FROM champion_rates",
    )
    .await?;
    let high_risk_sources = string_list(
        &state.db,
        "SELECT DISTINCT source FROM source_fetch_log WHERE risk_level = 'high'",
    )
    .await?;
    let stale_sources = string_list(
        &state.db,
        "SELECT source FROM source_fetch_log GROUP BY source \
         HAVING MAX(created_at) < now() - interval '24 hours'",
    )
    .await?;

    // Overall confidence — single shared formula (`confidence_band`) so the
    // pack endpoint and the quality endpoint can never drift apart.
    let build_coverage = (build_champions as f64 / TARGET_CHAMPIONS).min(1.0);
    let fallback_active = matchups == 0 || builds == 0;
    let confidence = confidence_band(rates, matchups, builds, build_champions);

    Ok(Json(json!({
        // ── Data Supremacy v1 enriched signals ──────────────────────────────
        "champion_rates_count": rates,
        "matchup_count": matchups,
        "build_count": builds,
        "primary_role_build_coverage": build_coverage,
        "meta_role_coverage": (rate_champions as f64 / TARGET_CHAMPIONS).min(1.0),
        "exact_matchup_coverage": matchups,
        "stale_sources": stale_sources,
        "high_risk_sources": high_risk_sources,
        "fallback_active": fallback_active,
        "confidence": confidence,
        "generated_at": now_epoch_secs(),
        // ── Backward-compatible flat counts (existing consumers) ─────────────
        "rates": rates,
        "matchups": matchups,
        "builds": builds,
        "feedback": counts["feedback"],
        "draft_samples": counts["draft_samples"]
    })))
}

fn now_epoch_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Overall data confidence band. MUST stay in sync with the client
/// `recommendation/draft_brain_data::data_confidence` — same thresholds
/// (build coverage ≥ 0.95, matchups ≥ 1000, roster target 172). Shared by both
/// `/v1/data-pack/latest` and `/v1/data-quality` so they can never diverge.
fn confidence_band(rates: i64, matchups: i64, builds: i64, build_champions: i64) -> &'static str {
    const TARGET_CHAMPIONS: f64 = 172.0;
    let build_cov = (build_champions as f64 / TARGET_CHAMPIONS).min(1.0);
    let fallback_active = matchups == 0 || builds == 0;
    if matchups == 0 && builds == 0 && rates == 0 {
        "low"
    } else if build_cov >= 0.95 && matchups >= 1_000 && rates > 0 && !fallback_active {
        "high"
    } else {
        "medium"
    }
}

async fn scalar_i64(db: &PgPool, sql: &str) -> Result<i64, StatusCode> {
    sqlx::query_scalar::<_, i64>(sql)
        .fetch_one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn string_list(db: &PgPool, sql: &str) -> Result<Vec<String>, StatusCode> {
    sqlx::query_scalar::<_, String>(sql)
        .fetch_all(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn quality_counts(db: &PgPool) -> Result<Value, StatusCode> {
    let rates = table_count(db, "champion_rates").await?;
    let matchups = table_count(db, "champion_matchups").await?;
    let builds = table_count(db, "champion_builds").await?;
    let feedback = table_count(db, "recommendation_feedback").await?;
    let samples = table_count(db, "draft_samples").await?;
    Ok(json!({
        "rates": rates,
        "matchups": matchups,
        "builds": builds,
        "feedback": feedback,
        "draft_samples": samples
    }))
}

async fn table_count(db: &PgPool, table: &str) -> Result<i64, StatusCode> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    sqlx::query_scalar::<_, i64>(&sql)
        .fetch_one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[cfg(test)]
mod feedback_validation_tests {
    use super::*;

    fn base() -> FeedbackInput {
        FeedbackInput {
            user_hash: "0123456789abcdef0123".to_string(),
            champion_id: 238,
            champion_key: "Zed".to_string(),
            feedback: "helpful".to_string(),
            model_version: "rules-v1".to_string(),
            score: 0.8,
            payload: Value::Null,
            idempotency_key: Some("abc123def456".to_string()),
        }
    }

    #[test]
    fn accepts_canonical_payload() {
        assert!(validate_feedback(&base()).is_ok());
    }

    #[test]
    fn every_canonical_verdict_is_accepted() {
        for v in ["helpful", "not_helpful", "picked", "skipped"] {
            let mut input = base();
            input.feedback = v.to_string();
            assert!(validate_feedback(&input).is_ok(), "{v} should pass");
        }
    }

    #[test]
    fn rejects_non_canonical_verdict() {
        let mut input = base();
        input.feedback = "love".to_string();
        assert!(validate_feedback(&input).is_err());
    }

    #[test]
    fn rejects_raw_user_hash() {
        let mut input = base();
        input.user_hash = "Faker".to_string();
        assert!(validate_feedback(&input).is_err());
    }
}
