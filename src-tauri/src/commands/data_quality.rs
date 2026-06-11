//! Data Supremacy v1 — command layer for the data-source registry + local
//! data-pack builder. Read/writes the local SQLite DB; the scoring engine stays
//! pure (logic lives in `recommendation::draft_brain_data`).
//!
//! Intentionally does NOT redefine `get_data_quality_report` (Codex owns that in
//! `commands/meta_sync.rs`). These commands add the source registry + the ability
//! to cache a *real-quality* local data pack so recommendation badges reflect
//! actual local coverage when the cloud is unavailable.

use crate::commands::ddragon::sync_ddragon_champions_inner;
use crate::commands::meta_sync::{
    import_builds_seed, import_matchups_seed, sync_meraki_rates_inner,
};
use crate::db::{builds_repo, champion_rates_repo, champion_repo, matchup_repo, summoner_repo};
use crate::errors::AppError;
use crate::recommendation::coverage_expansion_policy::{
    plan_coverage_expansion, CoverageExpansionInput, FrontierSample,
};
use crate::recommendation::coverage_ramp::{
    classify_data_trajectory, evaluate_coverage_ramp, CoverageRampInput, CoverageRampReport,
    RampSnapshot,
};
use crate::recommendation::data_pipeline_quality::{
    evaluate_pipeline_quality, PipelineQualityInput, PipelineQualityReport, PipelineSource,
};
use crate::recommendation::draft_brain::DataPack;
use crate::recommendation::draft_brain_data::{
    build_local_data_pack, compute_registry_report, DataSourceEntry, DataSourceKind,
    DataSourceRegistryReport, LocalCoverage,
};
use crate::recommendation::feedback_analytics::{
    analyze_feedback, FeedbackAnalytics, FeedbackEvent,
};
use crate::recommendation::feedback_observability::{
    personalization_status, summarize_observability, FeedbackObservability,
    FeedbackPersonalizationStatus,
};
use crate::recommendation::feedback_signal::FeedbackInput;
use crate::recommendation::ingestion_contract::{
    decide_cache_promotion, to_canonical_rows, CandidateQuality, CanonicalRowSet,
};
use crate::recommendation::match_discovery_planner::{
    plan_match_discovery, CrawledPlayerRecord, DiscoveredMatchCandidate, DiscoverySeed,
    KnownMatchRecord, MatchDiscoveryInput,
};
use crate::recommendation::match_fetch_planner::{
    plan_match_fetch, CoverageGap, FetchedMatchRecord, MatchCandidate, MatchFetchPlannerInput,
};
use crate::recommendation::match_v5_aggregator::aggregate_matches;
use crate::recommendation::match_v5_mapper::{match_v5_from_detail, normalize_patch};
use crate::recommendation::pipeline_scheduler_policy::{
    compute_rate_budget, plan_refresh, summarize_fetch_logs, FetchLogEntry, FetchLogSummary,
    RateLimitBudget, RateLimitInput, RefreshPlan, RefreshPolicyInput, RefreshSourceInput,
};
use crate::riot::client::{routing_for_region, runtime_client_from_env, runtime_riot_configured};
use crate::riot::endpoints::{matches as match_ep, summoner as summoner_ep};
use crate::AppState;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};
use tokio::time::{sleep, Duration};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

const PACK_TTL_SECS: i64 = 24 * 60 * 60;
/// u.gg aggregate-stats refresh cadence (open CDN; daily-moving stats).
const UGG_TTL_SECS: i64 = 6 * 60 * 60;
const TARGET_CHAMPIONS: u32 = 172;
const SCHEDULER_INITIAL_DELAY_SECS: u64 = 30;
// Aggressive growth cadence: tick every 3 min while warming up. We were using
// ~0.1% of the Riot personal-key budget (100 req/2min); the binding safety is
// the per-tick volume cap (FETCH_BATCH_LIMIT + CRAWL_BUDGET) plus the rolling
// 2-min request budget in riot::rate_limiter, NOT a slow tick.
const SCHEDULER_INTERVAL_SECS: u64 = 3 * 60;
const RATE_WINDOW_SECS: i64 = 60 * 60;
// Source-refresh gate (which sources may refresh per window) — raised so the
// gate no longer throttles aggressive Match-V5 collection. Actual Riot-call
// safety lives in the rolling 2-min budget (RIOT_BUDGET_*) + governor (20/s).
const RATE_MAX_REQUESTS: u32 = 60;
const MATCH_V5_SOURCE: &str = "riot_match_v5";
// Keep collecting Match-V5 batches well beyond the first "healthy" threshold.
// The quality engine treats ~1k matchups as usable; we keep growing real-game
// coverage aggressively until this larger collection target is reached.
const MATCH_V5_TARGET_MATCHUPS: u32 = 25_000;
const MATCH_V5_WARMUP_TTL_SECS: i64 = 3 * 60;
const MATCH_V5_STABLE_TTL_SECS: i64 = 60 * 60;
const MATCH_V5_RANKED_QUEUE: u32 = 420;
const MATCH_V5_CANDIDATE_COUNT: u8 = 20;
const MATCH_V5_FETCH_BATCH_LIMIT: u32 = 50;
const MATCH_V5_ROLE_TARGET_SAMPLES: u32 = 1000;
const MATCH_V5_ROLES: [&str; 5] = ["top", "jungle", "middle", "bottom", "utility"];
const MATCH_DISCOVERY_CRAWL_BUDGET: u32 = 15;
const MATCH_DISCOVERY_MAX_BREADTH: u32 = 15;
const MATCH_DISCOVERY_PER_PLAYER_MATCH_CAP: u32 = 5;
const MATCH_DISCOVERY_MATCH_LIST_COUNT: u8 = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct FeedbackObservabilityReport {
    pub counters: FeedbackObservability,
    pub status: FeedbackPersonalizationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct DataPipelineRefreshSummary {
    pub before_status: String,
    pub after_status: String,
    pub actions: Vec<String>,
    pub ddragon_champions: u32,
    pub meraki_rates: u32,
    pub builds_imported: u32,
    pub matchups_imported: u32,
    pub match_v5_matches: u32,
    pub match_v5_rates: u32,
    pub match_v5_matchups: u32,
    pub match_v5_builds: u32,
    pub match_v5_errors: u32,
    pub data_pack_cached: bool,
    pub cache_action: String,
    pub cache_promoted: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct PipelineSchedulerStatus {
    pub champ_select_active: bool,
    pub rate_limit: RateLimitBudget,
    pub fetch_logs: FetchLogSummary,
    pub plan: RefreshPlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct CoverageRampSnapshotView {
    pub taken_at: u32,
    pub champion_rate_rows: u32,
    pub matchup_rows: u32,
    pub build_rows: u32,
    pub discovered_matches: u32,
    pub fetched_matches: u32,
    pub processed_matches: u32,
    pub failed_matches: u32,
    pub crawled_players: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct LiveCoverageRampReport {
    pub before: CoverageRampSnapshotView,
    pub after: CoverageRampSnapshotView,
    pub ramp: CoverageRampReport,
    pub refresh: DataPipelineRefreshSummary,
    pub champ_select_active: bool,
    pub crawl_budget: u32,
}

/// In-memory record of the background scheduler's most recent ramp verdict. Lets the
/// status surface show a *trajectory* (quality + recent motion) without running a
/// network refresh on the UI path. Reset on app restart (not persisted).
#[derive(Debug, Clone)]
pub struct LastCoverageRamp {
    pub ramp_state: String,
    pub data_growing: bool,
    pub measured_at: i64,
}

/// Fused, user-facing data trajectory: point-in-time quality status + the scheduler's
/// most recent ramp motion (`coverage_ramp::classify_data_trajectory`). `trajectory`
/// is `unknown` until the first background tick has measured a ramp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct DataTrajectoryView {
    pub trajectory: String,
    pub quality_status: String,
    pub ramp_state: String,
    pub data_growing: bool,
    pub measured_at: Option<u32>,
    /// Whether a Riot production key is configured at runtime. When false, Match-V5
    /// live-match ingestion never runs — the UI shows an honest "no key" badge
    /// instead of implying live data.
    pub riot_key_present: bool,
    /// Whether Match-V5 ingestion is actually active (key present AND a synced
    /// active summoner).
    pub match_v5_enabled: bool,
    /// Epoch seconds of the last successful Match-V5 fetch, when any.
    pub match_v5_last_success_at: Option<u32>,
    /// Age in seconds of the last successful Match-V5 fetch (now − last_success).
    pub match_v5_age_secs: Option<u32>,
}

#[derive(Debug, Clone, Default)]
struct MatchV5IngestionOutcome {
    fetched_matches: u32,
    detail_errors: u32,
    rates: u32,
    matchups: u32,
    builds: u32,
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn count(conn: &rusqlite::Connection, sql: &str) -> Result<u32, AppError> {
    let n = conn.query_row(sql, [], |r| r.get::<_, i64>(0))?;
    Ok(n.max(0) as u32)
}

fn ramp_snapshot(conn: &rusqlite::Connection, taken_at: i64) -> Result<RampSnapshot, AppError> {
    Ok(RampSnapshot {
        taken_at,
        champion_rate_rows: count(conn, "SELECT COUNT(*) FROM champion_rates")?,
        matchup_rows: count(conn, "SELECT COUNT(*) FROM champion_matchups")?,
        build_rows: count(conn, "SELECT COUNT(*) FROM builds")?,
        discovered_matches: count(
            conn,
            "SELECT COUNT(*) FROM match_v5_fetch_history WHERE status = 'discovered'",
        )?,
        fetched_matches: count(
            conn,
            "SELECT COUNT(*) FROM match_v5_fetch_history WHERE status IN ('fetched', 'parsed')",
        )?,
        processed_matches: count(
            conn,
            "SELECT COUNT(*) FROM match_v5_fetch_history WHERE status = 'processed'",
        )?,
        failed_matches: count(
            conn,
            "SELECT COUNT(*) FROM match_v5_fetch_history WHERE status = 'failed'",
        )?,
        crawled_players: count(conn, "SELECT COUNT(*) FROM match_discovery_players")?,
    })
}

fn ramp_snapshot_view(snapshot: &RampSnapshot) -> CoverageRampSnapshotView {
    CoverageRampSnapshotView {
        taken_at: snapshot.taken_at.max(0) as u32,
        champion_rate_rows: snapshot.champion_rate_rows,
        matchup_rows: snapshot.matchup_rows,
        build_rows: snapshot.build_rows,
        discovered_matches: snapshot.discovered_matches,
        fetched_matches: snapshot.fetched_matches,
        processed_matches: snapshot.processed_matches,
        failed_matches: snapshot.failed_matches,
        crawled_players: snapshot.crawled_players,
    }
}

fn optional_string(conn: &rusqlite::Connection, sql: &str) -> Result<Option<String>, AppError> {
    Ok(conn
        .query_row(sql, [], |r| r.get::<_, Option<String>>(0))
        .optional()?
        .flatten()
        .filter(|value| !value.is_empty() && value != "unknown"))
}

fn pack_exists(conn: &rusqlite::Connection) -> Result<bool, AppError> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM draft_brain_packs WHERE kind = 'data_pack' LIMIT 1",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    Ok(exists)
}

fn source_risk(source: &str) -> String {
    let source = source.to_lowercase();
    if source.contains("scraper")
        || source.contains("lolalytics")
        || source.contains("op_gg")
        || source.contains("mobalytics")
    {
        "high"
    } else if source.contains("cloud")
        || source.contains("riot")
        || source.contains("meraki")
        || source.contains("u_gg")
        || source.contains("leaguepedia")
        || source.contains("postgres")
    {
        "medium"
    } else {
        "low"
    }
    .to_string()
}

fn push_source_rows(
    conn: &rusqlite::Connection,
    sources: &mut Vec<PipelineSource>,
    sql: &str,
) -> Result<(), AppError> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| {
        let source: String = r.get(0)?;
        let updated_at: i64 = r.get(1)?;
        Ok((source, updated_at))
    })?;
    for row in rows {
        let (source, updated_at) = row?;
        sources.push(PipelineSource {
            risk_level: source_risk(&source),
            source,
            updated_at,
        });
    }
    Ok(())
}

fn pipeline_sources(
    conn: &rusqlite::Connection,
    now: i64,
) -> Result<Vec<PipelineSource>, AppError> {
    let mut sources = Vec::new();
    push_source_rows(
        conn,
        &mut sources,
        "SELECT 'rates:' || source, MAX(cached_at) FROM champion_rates GROUP BY source",
    )?;
    push_source_rows(
        conn,
        &mut sources,
        "SELECT 'builds:' || source, MAX(cached_at) FROM builds GROUP BY source",
    )?;
    push_source_rows(
        conn,
        &mut sources,
        "SELECT 'matchups:' || source, MAX(cached_at) FROM champion_matchups GROUP BY source",
    )?;
    push_source_rows(
        conn,
        &mut sources,
        "SELECT 'pack:' || COALESCE(source, 'unknown'), fetched_at
         FROM draft_brain_packs
         WHERE kind = 'data_pack'",
    )?;

    if sources.is_empty() {
        sources.push(PipelineSource {
            source: "local_seed".to_string(),
            updated_at: now,
            risk_level: "low".to_string(),
        });
    }
    Ok(sources)
}

fn dominant_data_patch(conn: &rusqlite::Connection) -> Result<Option<String>, AppError> {
    optional_string(
        conn,
        "SELECT patch FROM (
             SELECT patch AS patch FROM champion_rates WHERE patch != 'unknown'
             UNION ALL
             SELECT patch_version AS patch FROM builds WHERE patch_version != 'unknown'
             UNION ALL
             SELECT patch_version AS patch FROM champion_matchups WHERE patch_version != 'unknown'
         )
         GROUP BY patch
         ORDER BY COUNT(*) DESC
         LIMIT 1",
    )
}

fn build_pipeline_quality_input(
    conn: &rusqlite::Connection,
    current_patch: String,
    now: i64,
) -> Result<PipelineQualityInput, AppError> {
    let (coverage, fallback_active, _stale) = gather_coverage(conn)?;
    let data_patch = dominant_data_patch(conn)?;
    let current_patch = if current_patch == "unknown" || current_patch.is_empty() {
        data_patch.clone().unwrap_or_else(|| "unknown".to_string())
    } else {
        current_patch
    };

    Ok(PipelineQualityInput {
        now,
        current_patch,
        data_patch,
        target_champions: coverage.total_champions.max(TARGET_CHAMPIONS),
        champion_rate_count: coverage.meta_role_champions,
        matchup_count: coverage.matchup_count,
        build_champion_count: coverage.build_champions,
        meta_role_count: coverage.meta_role_champions,
        sources: pipeline_sources(conn, now)?,
        fallback_available: fallback_active
            || coverage
                .sources
                .iter()
                .any(|source| source.source == "local_seed" || source.source == "manual_seed"),
        last_good_cache_available: pack_exists(conn)?,
    })
}

fn cache_local_data_pack(
    conn: &rusqlite::Connection,
    patch: Option<String>,
    region: Option<String>,
    now: i64,
) -> Result<DataPack, AppError> {
    let (coverage, _fallback, _stale) = gather_coverage(conn)?;
    let mut pack = build_local_data_pack(&coverage, patch, region);
    pack.generated_at = Some(now.clamp(0, u32::MAX as i64) as u32);

    let payload = serde_json::to_string(&pack)?;
    conn.execute(
        "INSERT INTO draft_brain_packs
             (kind, version, payload_json, source, fetched_at, expires_at)
         VALUES ('data_pack', ?1, ?2, 'local_builder', ?3, ?4)
         ON CONFLICT(kind) DO UPDATE SET
             version      = excluded.version,
             payload_json = excluded.payload_json,
             source       = excluded.source,
             fetched_at   = excluded.fetched_at,
             expires_at   = excluded.expires_at",
        params![pack.version, payload, now, now + PACK_TTL_SECS],
    )?;
    Ok(pack)
}

fn coverage_score_from_counts(rates: u32, matchups: u32, build_champions: u32) -> f32 {
    let rate_cov = (rates as f32 / TARGET_CHAMPIONS as f32).min(1.0);
    let matchup_cov = (matchups as f32 / 1_000.0).min(1.0);
    let build_cov = (build_champions as f32 / TARGET_CHAMPIONS as f32).min(1.0);
    ((rate_cov + matchup_cov + build_cov) / 3.0).clamp(0.0, 1.0)
}

fn candidate_quality_from_db(
    conn: &rusqlite::Connection,
    source: &str,
) -> Result<CandidateQuality, AppError> {
    let (coverage, _fallback, _stale) = gather_coverage(conn)?;
    let high_risk = coverage
        .sources
        .iter()
        .any(|entry| entry.risk_level == "high");
    Ok(CandidateQuality {
        source: source.to_string(),
        risk_level: if high_risk { "high" } else { "medium" }.to_string(),
        coverage_score: coverage_score_from_counts(
            coverage.champion_rates_count,
            coverage.matchup_count,
            coverage.build_champions,
        ),
        sample_size: coverage
            .champion_rates_count
            .saturating_add(coverage.matchup_count)
            .saturating_add(coverage.build_count),
        fresh: true,
    })
}

fn current_cache_quality(
    conn: &rusqlite::Connection,
    now: i64,
) -> Result<Option<CandidateQuality>, AppError> {
    let cached: Option<(String, String, Option<i64>)> = conn
        .query_row(
            "SELECT source, payload_json, expires_at
             FROM draft_brain_packs
             WHERE kind = 'data_pack'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;

    let Some((source, payload, expires_at)) = cached else {
        return Ok(None);
    };
    let Ok(pack) = serde_json::from_str::<DataPack>(&payload) else {
        return Ok(None);
    };

    let build_champions = count(
        conn,
        "SELECT COUNT(DISTINCT champion_id) FROM builds WHERE source != 'unknown'",
    )?;
    let rates = pack.quality.champion_rates;
    let matchups = pack.quality.matchups;
    let builds = pack.quality.builds;
    Ok(Some(CandidateQuality {
        source: source.clone(),
        risk_level: source_risk(&source),
        coverage_score: coverage_score_from_counts(rates, matchups, build_champions),
        sample_size: rates.saturating_add(matchups).saturating_add(builds),
        fresh: expires_at.map(|e| e >= now).unwrap_or(false),
    }))
}

fn action_keys(report: &PipelineQualityReport) -> Vec<String> {
    let mut keys = Vec::new();
    for action in &report.actions {
        if !keys.contains(&action.action) {
            keys.push(action.action.clone());
        }
    }
    keys
}

async fn current_pipeline_report(state: &AppState) -> Result<PipelineQualityReport, AppError> {
    let current_patch = state.ddragon.lock().await.current_version().to_string();
    let db = state.db.lock().await;
    let input = build_pipeline_quality_input(&db, current_patch, now_secs())?;
    Ok(evaluate_pipeline_quality(&input))
}

fn record_fetch_log(
    conn: &rusqlite::Connection,
    source: &str,
    status: &str,
    decision: &str,
    message: &str,
    started_at: i64,
    finished_at: i64,
) -> Result<(), AppError> {
    let duration_ms = finished_at.saturating_sub(started_at).saturating_mul(1000);
    conn.execute(
        "INSERT INTO source_fetch_log
             (source, status, decision, message, started_at, finished_at, duration_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            source,
            status,
            decision,
            message,
            started_at,
            finished_at,
            duration_ms,
        ],
    )?;
    Ok(())
}

async fn record_fetch_log_state(
    state: &AppState,
    source: &str,
    status: &str,
    decision: &str,
    message: &str,
    started_at: i64,
    finished_at: i64,
) {
    let db = state.db.lock().await;
    if let Err(err) = record_fetch_log(
        &db,
        source,
        status,
        decision,
        message,
        started_at,
        finished_at,
    ) {
        tracing::warn!("source_fetch_log yazılamadı ({source}): {err}");
    }
}

fn read_fetch_logs(
    conn: &rusqlite::Connection,
    since: i64,
) -> Result<Vec<FetchLogEntry>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT source, status, finished_at
         FROM source_fetch_log
         WHERE finished_at >= ?1
         ORDER BY finished_at ASC",
    )?;
    let rows = stmt.query_map(params![since], |r| {
        Ok(FetchLogEntry {
            source: r.get(0)?,
            status: r.get(1)?,
            at: r.get(2)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn recent_request_timestamps(
    conn: &rusqlite::Connection,
    since: i64,
) -> Result<Vec<i64>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT finished_at
         FROM source_fetch_log
         WHERE finished_at >= ?1
           AND decision = 'refresh'
           AND status IN ('success', 'failed', 'rate_limited')",
    )?;
    let rows = stmt.query_map(params![since], |r| r.get::<_, i64>(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn last_success_at(logs: &[FetchLogEntry], source: &str) -> Option<i64> {
    logs.iter()
        .filter(|entry| entry.source == source && entry.status == "success")
        .map(|entry| entry.at)
        .max()
}

fn source_health(summary: &FetchLogSummary, source: &str) -> String {
    summary
        .sources
        .iter()
        .find(|entry| entry.source == source)
        .map(|entry| entry.health.clone())
        .unwrap_or_else(|| "insufficient".to_string())
}

fn match_v5_scheduler_ttl(conn: &rusqlite::Connection) -> Result<i64, AppError> {
    let (coverage, _fallback, _stale) = gather_coverage(conn)?;
    if coverage.matchup_count < MATCH_V5_TARGET_MATCHUPS {
        Ok(MATCH_V5_WARMUP_TTL_SECS)
    } else {
        Ok(MATCH_V5_STABLE_TTL_SECS)
    }
}

async fn has_active_summoner(state: &AppState) -> Result<bool, AppError> {
    let db = state.db.lock().await;
    Ok(summoner_repo::get_active_puuid(&db)?.is_some())
}

async fn is_champ_select_active(state: &AppState) -> bool {
    let client = state.lcu_client.lock().await.clone();
    let Some(client) = client else {
        return false;
    };
    match client.get_raw("/lol-gameflow/v1/gameflow-phase").await {
        Ok(value) => value.as_str() == Some("ChampSelect"),
        Err(_) => false,
    }
}

async fn build_pipeline_scheduler_status(
    state: &AppState,
) -> Result<PipelineSchedulerStatus, AppError> {
    let now = now_secs();
    let since = now - 30 * 24 * 60 * 60;
    let champ_select_active = is_champ_select_active(state).await;
    let riot_enabled = runtime_riot_configured() && has_active_summoner(state).await?;
    let (logs, request_timestamps, match_v5_ttl_secs) = {
        let db = state.db.lock().await;
        (
            read_fetch_logs(&db, since)?,
            recent_request_timestamps(&db, now - RATE_WINDOW_SECS)?,
            match_v5_scheduler_ttl(&db)?,
        )
    };
    let fetch_logs = summarize_fetch_logs(&logs, now);
    let rate_limit = compute_rate_budget(&RateLimitInput {
        now,
        window_secs: RATE_WINDOW_SECS,
        max_requests: RATE_MAX_REQUESTS,
        request_timestamps,
    });
    let source = |key: &str, enabled: bool, ttl_secs: i64| RefreshSourceInput {
        source: key.to_string(),
        enabled,
        last_fetch_at: last_success_at(&logs, key),
        ttl_secs,
        health: source_health(&fetch_logs, key),
        next_allowed_at: None,
    };
    let plan = plan_refresh(&RefreshPolicyInput {
        now,
        champ_select_active,
        remaining_budget: rate_limit.remaining,
        sources: vec![
            source("ddragon", true, PACK_TTL_SECS),
            source("meraki", true, PACK_TTL_SECS),
            // u.gg open CDN: rates + builds across the full roster, 6 h cadence.
            source("u_gg", true, UGG_TTL_SECS),
            // Leaguepedia pro presence — one polite Cargo request per day.
            source("leaguepedia", true, PACK_TTL_SECS),
            source("match_v5", riot_enabled, match_v5_ttl_secs),
        ],
    });

    Ok(PipelineSchedulerStatus {
        champ_select_active,
        rate_limit,
        fetch_logs,
        plan,
    })
}

async fn run_scheduled_source(state: &AppState, source: &str) -> Result<String, AppError> {
    match source {
        "ddragon" => {
            let count = sync_ddragon_champions_inner(state).await?;
            Ok(format!("{count} champions"))
        }
        "meraki" => {
            let count = sync_meraki_rates_inner(state).await?;
            Ok(format!("{count} rates"))
        }
        "u_gg" => {
            let (rates, builds, matchups) = sync_ugg_inner(state).await?;
            Ok(format!(
                "{rates} rates, {builds} builds, {matchups} matchups"
            ))
        }
        "leaguepedia" => {
            let count = sync_leaguepedia_inner(state).await?;
            Ok(format!("{count} pro champions"))
        }
        "match_v5" => {
            let outcome = sync_match_v5_ingestion(state).await?;
            Ok(format!(
                "{} matches, {} rates, {} matchups, {} builds, {} errors",
                outcome.fetched_matches,
                outcome.rates,
                outcome.matchups,
                outcome.builds,
                outcome.detail_errors
            ))
        }
        other => Err(AppError::Other(format!(
            "Bilinmeyen pipeline source: {other}"
        ))),
    }
}

async fn promote_data_pack_if_quality_allows(
    state: &AppState,
    current_good: Option<CandidateQuality>,
) -> Result<(), AppError> {
    let current_patch = state.ddragon.lock().await.current_version().to_string();
    let started = now_secs();
    let (status, decision_key, message) = {
        let db = state.db.lock().await;
        let candidate = candidate_quality_from_db(&db, "local_builder")?;
        let decision = decide_cache_promotion(&candidate, current_good.as_ref());
        if decision.promoted {
            cache_local_data_pack(&db, Some(current_patch), None, now_secs())?;
            ("success", decision.action, decision.reason)
        } else {
            ("skipped", decision.action, decision.reason)
        }
    };
    record_fetch_log_state(
        state,
        "data_pack_cache",
        status,
        &decision_key,
        &message,
        started,
        now_secs(),
    )
    .await;
    Ok(())
}

async fn run_scheduler_tick(state: &AppState) -> Result<(), AppError> {
    let _refresh_guard = state.data_pipeline_refresh_lock.lock().await;
    let current_good = {
        let db = state.db.lock().await;
        current_cache_quality(&db, now_secs())?
    };
    // Best-effort ramp baseline before this tick's refreshes (read-only counts).
    let ramp_before = {
        let db = state.db.lock().await;
        ramp_snapshot(&db, now_secs()).ok()
    };
    let status = build_pipeline_scheduler_status(state).await?;
    let mut any_success = false;

    for decision in &status.plan.decisions {
        let started = now_secs();
        if decision.decision != "refresh" {
            let status_token = if decision.decision == "skip_rate_limited" {
                "rate_limited"
            } else {
                "skipped"
            };
            record_fetch_log_state(
                state,
                &decision.source,
                status_token,
                &decision.decision,
                &decision.reason,
                started,
                now_secs(),
            )
            .await;
            continue;
        }

        match run_scheduled_source(state, &decision.source).await {
            Ok(message) => {
                any_success = true;
                record_fetch_log_state(
                    state,
                    &decision.source,
                    "success",
                    &decision.decision,
                    &message,
                    started,
                    now_secs(),
                )
                .await;
            }
            Err(err) => {
                record_fetch_log_state(
                    state,
                    &decision.source,
                    "failed",
                    &decision.decision,
                    &err.to_string(),
                    started,
                    now_secs(),
                )
                .await;
            }
        }
    }

    if any_success {
        promote_data_pack_if_quality_allows(state, current_good).await?;
    }

    // Best-effort: record this tick's ramp motion for the trajectory surface. A failure
    // here must never abort the tick (its job is the refresh above).
    if let Some(before) = ramp_before {
        record_tick_ramp(state, before, status.champ_select_active).await;
    }

    Ok(())
}

/// Evaluate the ramp across this tick (before snapshot → now) and store the verdict in
/// `AppState` for `get_data_trajectory`. Non-fatal; swallows errors.
async fn record_tick_ramp(state: &AppState, before: RampSnapshot, champ_select_active: bool) {
    let now = now_secs();
    let after = {
        let db = state.db.lock().await;
        match ramp_snapshot(&db, now) {
            Ok(s) => s,
            Err(_) => return,
        }
    };
    let crawl_budget = if champ_select_active {
        0
    } else {
        MATCH_DISCOVERY_CRAWL_BUDGET
    };
    let ramp = evaluate_coverage_ramp(&CoverageRampInput {
        before,
        after,
        champ_select_active,
        crawl_budget,
    });
    *state.last_coverage_ramp.lock().await = Some(LastCoverageRamp {
        ramp_state: ramp.ramp_state,
        data_growing: ramp.data_growing,
        measured_at: now,
    });
}

pub(crate) async fn start_data_pipeline_scheduler(app: AppHandle) {
    let state = app.state::<AppState>();
    let mut handle_guard = state.data_pipeline_scheduler_handle.lock().await;
    if handle_guard.is_some() {
        return;
    }

    let cancel = CancellationToken::new();
    let task_cancel = cancel.clone();
    let task_app = app.clone();
    let handle = tokio::spawn(async move {
        tokio::select! {
            _ = task_cancel.cancelled() => return,
            _ = sleep(Duration::from_secs(SCHEDULER_INITIAL_DELAY_SECS)) => {}
        }

        loop {
            let state = task_app.state::<AppState>();
            if let Err(err) = run_scheduler_tick(&state).await {
                tracing::warn!("Data pipeline scheduler tick failed: {err}");
            }
            tokio::select! {
                _ = task_cancel.cancelled() => break,
                _ = sleep(Duration::from_secs(SCHEDULER_INTERVAL_SECS)) => {}
            }
        }
    });

    *state.data_pipeline_scheduler_cancel.lock().await = Some(cancel);
    *handle_guard = Some(handle);
    tracing::info!("Data pipeline scheduler started");
}

fn ensure_champions_for_rows(
    conn: &rusqlite::Connection,
    row_set: &CanonicalRowSet,
) -> Result<(), AppError> {
    let mut ids = HashSet::new();
    for row in &row_set.rates {
        ids.insert(row.champion_id);
    }
    for row in &row_set.matchups {
        ids.insert(row.champion_id);
        ids.insert(row.opponent_id);
    }
    for row in &row_set.builds {
        ids.insert(row.champion_id);
    }

    for id in ids {
        if id == 0 {
            continue;
        }
        if champion_repo::get_champion_by_id(conn, id as i64)?.is_none() {
            champion_repo::upsert_champion(
                conn,
                id as i64,
                &id.to_string(),
                &format!("Champion {id}"),
                "",
            )?;
        }
    }
    Ok(())
}

fn upsert_canonical_rows(
    conn: &rusqlite::Connection,
    row_set: &CanonicalRowSet,
) -> Result<(), AppError> {
    ensure_champions_for_rows(conn, row_set)?;

    for row in &row_set.rates {
        champion_rates_repo::upsert_rate_with_region(
            conn,
            &champion_rates_repo::ChampionRateRow {
                champion_id: row.champion_id,
                position: row.position.clone(),
                win_rate: row.win_rate,
                pick_rate: row.pick_rate,
                ban_rate: row.ban_rate,
                sample_size: row.sample_size,
                patch: row.patch.clone(),
                source: row.source.clone(),
                confidence: row.confidence.clone(),
            },
            &row.region,
        )?;
    }

    for row in &row_set.matchups {
        matchup_repo::upsert_matchup_with_metadata(
            conn,
            &matchup_repo::MatchupRow {
                champion_id: row.champion_id as i64,
                opponent_id: row.opponent_id as i64,
                position: row.position.clone(),
                games: row.games as i64,
                wins: row.wins as i64,
                win_rate: row.win_rate as f64,
                source: row.source.clone(),
                patch_version: row.patch.clone(),
            },
            &row.region,
            &row.confidence,
            row.sample_size as i64,
        )?;
    }

    for row in &row_set.builds {
        if row.item_ids.is_empty() && row.rune_ids.is_empty() {
            continue;
        }
        let build = builds_repo::BuildRow {
            champion_id: row.champion_id as i64,
            position: row.position.clone(),
            patch_version: row.patch.clone(),
            item_ids: serde_json::to_string(&row.item_ids)?,
            rune_ids: serde_json::to_string(&row.rune_ids)?,
            win_rate: row.win_rate as f64,
            source: row.source.clone(),
            opponent_archetype: None,
            skill_order: None,
            summoner_spells: Some(serde_json::to_string(&row.summoner_spells)?),
            secondary_runes: None,
            stat_shards: None,
        };
        builds_repo::upsert_build_with_metadata(
            conn,
            &build,
            row.pick_rate as f64,
            &row.region,
            row.games as i64,
            &row.confidence,
        )?;
    }
    Ok(())
}

/// Fetch the u.gg aggregate source (rates + builds across the full roster) via the
/// `AggregateSource` framework and upsert its canonical rows. u.gg is an open CDN
/// (no Riot key, no LCU), so it doesn't contend with the Riot budget or champ
/// select. Returns `(rate_rows, build_rows)`.
async fn sync_ugg_inner(state: &AppState) -> Result<(u32, u32, u32), AppError> {
    // Phase 1: champion roster + current patch (short locks).
    let champions = {
        let db = state.db.lock().await;
        champion_repo::list_all(&db)?
    };
    let patch = normalize_patch(state.ddragon.lock().await.current_version());

    let ctx = crate::meta::source::FetchCtx {
        patch,
        region: "all".to_string(),
        champions,
    };

    // Phase 2: HTTP fetch via the framework (no DB lock held).
    let row_set = crate::meta::source::AggregateSource::UGg
        .fetch(&ctx)
        .await
        .map_err(|e| AppError::Other(format!("u.gg fetch hatası: {e}")))?;
    let rates = row_set.rates.len() as u32;
    let builds = row_set.builds.len() as u32;
    let matchups = row_set.matchups.len() as u32;

    // Phase 3: batch upsert (short DB lock).
    {
        let db = state.db.lock().await;
        upsert_canonical_rows(&db, &row_set)?;
    }

    tracing::info!(
        "u.gg sync tamamlandı: {} rate, {} build, {} matchup",
        rates,
        builds,
        matchups
    );
    Ok((rates, builds, matchups))
}

/// Fetch recent pro-play drafts from Leaguepedia (one polite Cargo request) and
/// store champion-level **pro presence** (pick% + ban%) under a synthetic `"pro"`
/// position + `"leaguepedia"` source. Kept OUT of the ranked per-role blend (role
/// queries never ask for `"pro"`); read separately for the "pro heat" badge.
/// Returns the number of champions written.
async fn sync_leaguepedia_inner(state: &AppState) -> Result<u32, AppError> {
    use crate::meta::leaguepedia;

    // Phase 1: champion display-name → id map (short lock).
    let name_to_id: HashMap<String, u32> = {
        let db = state.db.lock().await;
        champion_repo::list_all(&db)?
            .into_iter()
            .filter_map(|c| {
                u32::try_from(c.champion_id)
                    .ok()
                    .map(|id| (leaguepedia::normalize_name(&c.name), id))
            })
            .collect()
    };
    let patch = normalize_patch(state.ddragon.lock().await.current_version());

    // Phase 2: one polite Cargo fetch (no DB lock); rate-limit → Err → fallback.
    let value = leaguepedia::fetch_draft_rows(0)
        .await
        .map_err(|e| AppError::Other(format!("Leaguepedia fetch hatası: {e}")))?;
    let games = leaguepedia::parse_draft_rows(&value);
    let presence = leaguepedia::aggregate_presence(&games, &name_to_id);

    // Phase 3: upsert as canonical rate rows (short DB lock).
    let rates: Vec<_> = presence
        .iter()
        .map(
            |p| crate::recommendation::ingestion_contract::CanonicalRateRow {
                region: "pro".to_string(),
                patch: patch.clone(),
                champion_id: p.champion_id,
                position: "pro".to_string(),
                win_rate: 0.0,
                pick_rate: p.pick_rate,
                ban_rate: p.ban_rate,
                sample_size: p.total_games,
                source: "leaguepedia".to_string(),
                confidence: if p.total_games >= 100 {
                    "high"
                } else if p.total_games >= 30 {
                    "medium"
                } else {
                    "low"
                }
                .to_string(),
            },
        )
        .collect();
    let count = rates.len() as u32;
    {
        let db = state.db.lock().await;
        upsert_canonical_rows(
            &db,
            &CanonicalRowSet {
                region: "pro".to_string(),
                rates,
                matchups: Vec::new(),
                builds: Vec::new(),
            },
        )?;
    }

    tracing::info!(
        "Leaguepedia pro sync tamamlandı: {} şampiyon ({} pro maç)",
        count,
        games.len()
    );
    Ok(count)
}

fn build_match_fetch_candidates(
    ids: &[String],
    region: &str,
    patch: &str,
    now: i64,
) -> Vec<MatchCandidate> {
    ids.iter()
        .enumerate()
        .map(|(idx, id)| MatchCandidate {
            match_id: id.clone(),
            region: region.to_string(),
            patch: patch.to_string(),
            queue_id: MATCH_V5_RANKED_QUEUE,
            role_hint: None,
            discovered_at: now.saturating_sub(idx as i64),
        })
        .collect()
}

fn record_match_fetch_candidates(
    conn: &rusqlite::Connection,
    candidates: &[MatchCandidate],
    now: i64,
) -> Result<(), AppError> {
    for c in candidates {
        if c.match_id.trim().is_empty() {
            continue;
        }
        conn.execute(
            "INSERT INTO match_v5_fetch_history
                 (match_id, region, patch, queue_id, status, discovered_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'discovered', ?5, ?6)
             ON CONFLICT(match_id) DO UPDATE SET
                 region = excluded.region,
                 patch = CASE
                     WHEN match_v5_fetch_history.status IN ('fetched', 'parsed', 'processed')
                     THEN match_v5_fetch_history.patch
                     ELSE excluded.patch
                 END,
                 queue_id = CASE
                     WHEN match_v5_fetch_history.queue_id = 0 THEN excluded.queue_id
                     ELSE match_v5_fetch_history.queue_id
                 END,
                 discovered_at = match_v5_fetch_history.discovered_at,
                 updated_at = excluded.updated_at",
            params![
                c.match_id.as_str(),
                c.region.as_str(),
                c.patch.as_str(),
                c.queue_id as i64,
                c.discovered_at,
                now,
            ],
        )?;
    }
    Ok(())
}

fn read_match_fetch_records(
    conn: &rusqlite::Connection,
    ids: &[String],
) -> Result<Vec<FetchedMatchRecord>, AppError> {
    let mut records = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT match_id, region, patch, status,
                COALESCE(fetched_at, processed_at, discovered_at, updated_at)
         FROM match_v5_fetch_history
         WHERE match_id = ?1",
    )?;
    for id in ids {
        if let Some(record) = stmt
            .query_row(params![id], |r| {
                Ok(FetchedMatchRecord {
                    match_id: r.get(0)?,
                    region: r.get(1)?,
                    patch: r.get(2)?,
                    status: r.get(3)?,
                    fetched_at: r.get(4)?,
                })
            })
            .optional()?
        {
            records.push(record);
        }
    }
    Ok(records)
}

fn read_pending_discovered_match_ids(
    conn: &rusqlite::Connection,
    region: &str,
    limit: u32,
) -> Result<Vec<String>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT match_id
         FROM match_v5_fetch_history
         WHERE region = ?1 AND status IN ('discovered', 'failed')
         ORDER BY discovered_at DESC, match_id ASC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![region, limit as i64], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn hash_puuid(puuid: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in puuid.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn is_lcu_uuid_puuid(puuid: &str) -> bool {
    let bytes = puuid.as_bytes();
    if bytes.len() != 36 {
        return false;
    }

    bytes.iter().enumerate().all(|(idx, byte)| {
        if matches!(idx, 8 | 13 | 18 | 23) {
            *byte == b'-'
        } else {
            byte.is_ascii_hexdigit()
        }
    })
}

fn participant_puuids_from_detail(detail: &serde_json::Value) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut puuids: Vec<String> = detail
        .get("info")
        .and_then(|info| info.get("participants"))
        .and_then(|participants| participants.as_array())
        .into_iter()
        .flatten()
        .filter_map(|participant| participant.get("puuid").and_then(|value| value.as_str()))
        .map(str::trim)
        .filter(|puuid| !puuid.is_empty())
        .filter_map(|puuid| {
            if seen.insert(puuid.to_string()) {
                Some(puuid.to_string())
            } else {
                None
            }
        })
        .collect();
    puuids.sort();
    puuids
}

fn discovery_seed(
    puuid_hash: String,
    region: &str,
    source: &str,
    now: i64,
    count: u32,
) -> DiscoverySeed {
    DiscoverySeed {
        puuid_hash,
        region: region.to_string(),
        source: source.to_string(),
        seen_at: now,
        contribution_count: count,
    }
}

fn upsert_discovery_seeds(
    conn: &rusqlite::Connection,
    seeds: &[DiscoverySeed],
    now: i64,
) -> Result<(), AppError> {
    for seed in seeds {
        if seed.puuid_hash.trim().is_empty() {
            continue;
        }
        conn.execute(
            "INSERT INTO match_discovery_players
                 (puuid_hash, region, source, contribution_count, first_seen_at, last_seen_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6)
             ON CONFLICT(puuid_hash) DO UPDATE SET
                 region = excluded.region,
                 source = excluded.source,
                 contribution_count = match_discovery_players.contribution_count
                     + excluded.contribution_count,
                 last_seen_at = MAX(match_discovery_players.last_seen_at, excluded.last_seen_at),
                 updated_at = excluded.updated_at",
            params![
                seed.puuid_hash.as_str(),
                seed.region.as_str(),
                seed.source.as_str(),
                seed.contribution_count as i64,
                seed.seen_at,
                now,
            ],
        )?;
    }
    Ok(())
}

fn mark_discovery_player_crawled(
    conn: &rusqlite::Connection,
    puuid_hash: &str,
    region: &str,
    now: i64,
) -> Result<(), AppError> {
    if puuid_hash.trim().is_empty() {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO match_discovery_players
             (puuid_hash, region, source, contribution_count, first_seen_at, last_seen_at,
              last_crawled_at, crawl_count, updated_at)
         VALUES (?1, ?2, 'manual_seed', 0, ?3, ?3, ?3, 1, ?3)
         ON CONFLICT(puuid_hash) DO UPDATE SET
             region = excluded.region,
             last_crawled_at = excluded.last_crawled_at,
             crawl_count = match_discovery_players.crawl_count + 1,
             updated_at = excluded.updated_at",
        params![puuid_hash, region, now],
    )?;
    Ok(())
}

fn read_crawled_player_records(
    conn: &rusqlite::Connection,
    region: &str,
) -> Result<Vec<CrawledPlayerRecord>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT puuid_hash, region, last_crawled_at, crawl_count
         FROM match_discovery_players
         WHERE region = ?1 AND last_crawled_at IS NOT NULL",
    )?;
    let rows = stmt.query_map(params![region], |r| {
        Ok(CrawledPlayerRecord {
            puuid_hash: r.get(0)?,
            region: r.get(1)?,
            last_crawled_at: r.get(2)?,
            crawl_count: r.get::<_, i64>(3)?.max(0) as u32,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn read_known_match_records(
    conn: &rusqlite::Connection,
    ids: &[String],
) -> Result<Vec<KnownMatchRecord>, AppError> {
    let mut records = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT match_id, region, status FROM match_v5_fetch_history WHERE match_id = ?1",
    )?;
    for id in ids {
        if let Some(record) = stmt
            .query_row(params![id], |r| {
                Ok(KnownMatchRecord {
                    match_id: r.get(0)?,
                    region: r.get(1)?,
                    status: r.get(2)?,
                })
            })
            .optional()?
        {
            records.push(record);
        }
    }
    Ok(records)
}

fn aggregate_participant_seeds(
    puuids: &[String],
    region: &str,
    now: i64,
) -> (Vec<DiscoverySeed>, HashMap<String, String>) {
    let mut counts: HashMap<String, u32> = HashMap::new();
    let mut raw_by_hash = HashMap::new();
    for puuid in puuids {
        if puuid.trim().is_empty() {
            continue;
        }
        let hash = hash_puuid(puuid);
        raw_by_hash
            .entry(hash.clone())
            .or_insert_with(|| puuid.clone());
        *counts.entry(hash).or_insert(0) += 1;
    }
    let mut seeds: Vec<DiscoverySeed> = counts
        .into_iter()
        .map(|(hash, count)| discovery_seed(hash, region, "match_participant", now, count))
        .collect();
    seeds.sort_by(|a, b| a.puuid_hash.cmp(&b.puuid_hash));
    (seeds, raw_by_hash)
}

fn discovered_match_candidates(
    ids: &[String],
    region: &str,
    source_hash: &str,
    now: i64,
) -> Vec<DiscoveredMatchCandidate> {
    ids.iter()
        .enumerate()
        .map(|(idx, id)| DiscoveredMatchCandidate {
            match_id: id.clone(),
            region: region.to_string(),
            source_puuid_hash: source_hash.to_string(),
            discovered_at: now.saturating_sub(idx as i64),
        })
        .collect()
}

fn role_samples(
    conn: &rusqlite::Connection,
    sql: &str,
    region: &str,
    patch: &str,
    role: &str,
) -> Result<u32, AppError> {
    let value = conn.query_row(sql, params![MATCH_V5_SOURCE, region, patch, role], |r| {
        r.get::<_, i64>(0)
    })?;
    Ok(value.max(0) as u32)
}

fn build_coverage_expansion_frontiers(
    conn: &rusqlite::Connection,
    region: &str,
    patch: &str,
) -> Result<Vec<FrontierSample>, AppError> {
    let mut frontiers = Vec::new();
    for role in MATCH_V5_ROLES {
        let rate_samples = role_samples(
            conn,
            "SELECT COALESCE(SUM(sample_size), 0)
             FROM champion_rates
             WHERE source = ?1 AND region = ?2 AND patch = ?3 AND position = ?4",
            region,
            patch,
            role,
        )?;
        let matchup_samples = role_samples(
            conn,
            "SELECT COALESCE(SUM(sample_size), SUM(games), 0)
             FROM champion_matchups
             WHERE source = ?1 AND region = ?2 AND patch_version = ?3 AND position = ?4",
            region,
            patch,
            role,
        )?;
        let build_samples = role_samples(
            conn,
            "SELECT COALESCE(SUM(games), 0)
             FROM builds
             WHERE source = ?1 AND region = ?2 AND patch_version = ?3 AND position = ?4",
            region,
            patch,
            role,
        )?;
        let current_samples = rate_samples.min(matchup_samples).min(build_samples);
        frontiers.push(FrontierSample {
            region: region.to_string(),
            patch: patch.to_string(),
            role: role.to_string(),
            champion_id: None,
            current_samples,
            target_samples: MATCH_V5_ROLE_TARGET_SAMPLES,
        });
    }
    Ok(frontiers)
}

fn build_match_fetch_coverage_gaps(
    conn: &rusqlite::Connection,
    region: &str,
    patch: &str,
    champ_select_active: bool,
) -> Result<Vec<CoverageGap>, AppError> {
    let frontiers = build_coverage_expansion_frontiers(conn, region, patch)?;
    let total_samples: u32 = frontiers.iter().map(|f| f.current_samples).sum();
    let expansion = plan_coverage_expansion(&CoverageExpansionInput {
        champ_select_active,
        frontiers,
        player_sample_counts: if total_samples == 0 {
            Vec::new()
        } else {
            vec![total_samples]
        },
        max_targets: MATCH_V5_ROLES.len() as u32,
    });
    Ok(expansion
        .targets
        .into_iter()
        .map(|target| CoverageGap {
            region: target.frontier.region,
            patch: target.frontier.patch,
            role: target.frontier.role,
            current_samples: target.frontier.current_samples,
            target_samples: target.frontier.target_samples,
            priority: target.priority,
        })
        .collect())
}

fn mark_match_fetch_failed(
    conn: &rusqlite::Connection,
    match_id: &str,
    region: &str,
    patch: &str,
    error: &str,
    now: i64,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO match_v5_fetch_history
             (match_id, region, patch, queue_id, status, discovered_at, fetched_at, error, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'failed', ?5, ?5, ?6, ?5)
         ON CONFLICT(match_id) DO UPDATE SET
             region = excluded.region,
             patch = excluded.patch,
             queue_id = excluded.queue_id,
             status = 'failed',
             fetched_at = excluded.fetched_at,
             error = excluded.error,
             updated_at = excluded.updated_at",
        params![
            match_id,
            region,
            patch,
            MATCH_V5_RANKED_QUEUE as i64,
            now,
            error,
        ],
    )?;
    Ok(())
}

fn mark_match_fetch_processed(
    conn: &rusqlite::Connection,
    match_id: &str,
    region: &str,
    patch: &str,
    queue_id: u32,
    now: i64,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO match_v5_fetch_history
             (match_id, region, patch, queue_id, status, discovered_at, fetched_at, processed_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'processed', ?5, ?5, ?5, ?5)
         ON CONFLICT(match_id) DO UPDATE SET
             region = excluded.region,
             patch = excluded.patch,
             queue_id = excluded.queue_id,
             status = 'processed',
             fetched_at = COALESCE(match_v5_fetch_history.fetched_at, excluded.fetched_at),
             processed_at = excluded.processed_at,
             error = NULL,
             updated_at = excluded.updated_at",
        params![match_id, region, patch, queue_id as i64, now],
    )?;
    Ok(())
}

async fn sync_match_v5_ingestion(state: &AppState) -> Result<MatchV5IngestionOutcome, AppError> {
    let Some(riot) = runtime_client_from_env() else {
        return Ok(MatchV5IngestionOutcome::default());
    };

    let Some((mut puuid, mut region, game_name, tag_line)) = ({
        let db = state.db.lock().await;
        let puuid = summoner_repo::get_active_puuid(&db)?;
        match puuid {
            Some(puuid) => {
                let info = summoner_repo::get_summoner_by_puuid(&db, &puuid)?;
                let (region, game_name, tag_line) = info
                    .map(|info| (info.region, info.game_name, info.tag_line))
                    .unwrap_or_else(|| ("euw1".to_string(), String::new(), String::new()));
                Some((puuid, region, game_name, tag_line))
            }
            None => None,
        }
    }) else {
        return Ok(MatchV5IngestionOutcome::default());
    };

    if is_lcu_uuid_puuid(&puuid) {
        if game_name.trim().is_empty() || tag_line.trim().is_empty() {
            return Err(AppError::Other(
                "Match-V5 aktif oyuncu PUUID'si LCU UUID formatında; Riot ID yok, account-v1 ile çözülemiyor"
                    .to_string(),
            ));
        }

        let resolved =
            summoner_ep::get_by_riot_id(riot.as_ref(), &game_name, &tag_line, &region).await?;
        if is_lcu_uuid_puuid(&resolved.puuid) {
            return Err(AppError::Other(
                "Riot account-v1 LCU UUID formatında PUUID döndürdü; Match-V5 çağrısı güvenli değil"
                    .to_string(),
            ));
        }

        puuid = resolved.puuid.clone();
        region = resolved.region.clone();
        let db = state.db.lock().await;
        summoner_repo::upsert_summoner(&db, &resolved)?;
    }

    let routing = routing_for_region(&region);
    let mut ids = match_ep::list_ids(
        riot.as_ref(),
        &puuid,
        routing,
        Some("ranked"),
        Some(420),
        MATCH_V5_CANDIDATE_COUNT,
    )
    .await?;

    let current_patch = normalize_patch(state.ddragon.lock().await.current_version());
    let now = now_secs();
    let active_hash = hash_puuid(&puuid);
    {
        let db = state.db.lock().await;
        let active_seed = discovery_seed(
            active_hash.clone(),
            &region,
            "active_player",
            now,
            ids.len() as u32,
        );
        upsert_discovery_seeds(&db, &[active_seed], now)?;
        mark_discovery_player_crawled(&db, &active_hash, &region, now)?;

        let pending =
            read_pending_discovered_match_ids(&db, &region, MATCH_V5_CANDIDATE_COUNT as u32)?;
        let mut seen: HashSet<String> = ids.iter().cloned().collect();
        for id in pending {
            if seen.insert(id.clone()) {
                ids.push(id);
            }
        }
    }

    let candidates = build_match_fetch_candidates(&ids, &region, &current_patch, now);
    let plan = {
        let db = state.db.lock().await;
        record_match_fetch_candidates(&db, &candidates, now)?;
        let fetched_records = read_match_fetch_records(&db, &ids)?;
        let coverage_gaps = build_match_fetch_coverage_gaps(&db, &region, &current_patch, false)?;
        plan_match_fetch(&MatchFetchPlannerInput {
            now,
            champ_select_active: false,
            rate_budget: MATCH_V5_FETCH_BATCH_LIMIT,
            batch_limit: MATCH_V5_FETCH_BATCH_LIMIT,
            candidates: candidates.clone(),
            fetched_records,
            coverage_gaps,
        })
    };

    let mut details = Vec::new();
    let mut participant_puuids = Vec::new();
    let mut detail_errors = 0u32;
    let mut failed = Vec::new();
    for id in &plan.to_fetch {
        // Rolling 2-min Riot budget: stop this tick early before we'd exceed
        // Riot's 100 req / 2 min ceiling (a 429 there risks key revocation).
        if !crate::riot::rate_limiter::riot_budget().try_acquire() {
            break;
        }
        match match_ep::get_detail(riot.as_ref(), id, routing).await {
            Ok(detail) => {
                participant_puuids.extend(participant_puuids_from_detail(&detail));
                if let Some(parsed) = match_v5_from_detail(&detail, id) {
                    details.push(parsed);
                } else {
                    detail_errors += 1;
                    failed.push((id.clone(), "parse_failed".to_string()));
                }
            }
            Err(_) => {
                detail_errors += 1;
                failed.push((id.clone(), "detail_fetch_failed".to_string()));
            }
        }
    }

    let (participant_seeds, raw_by_hash) =
        aggregate_participant_seeds(&participant_puuids, &region, now_secs());

    let aggregation = aggregate_matches(&details);
    let row_set = to_canonical_rows(&aggregation, &region);
    {
        let db = state.db.lock().await;
        for (id, error) in &failed {
            mark_match_fetch_failed(&db, id, &region, &current_patch, error, now_secs())?;
        }
        upsert_canonical_rows(&db, &row_set)?;
        let processed_at = now_secs();
        for detail in &details {
            mark_match_fetch_processed(
                &db,
                &detail.match_id,
                &region,
                &detail.patch,
                detail.queue_id,
                processed_at,
            )?;
        }
    }

    if !participant_seeds.is_empty() {
        let crawl_plan = {
            let db = state.db.lock().await;
            upsert_discovery_seeds(&db, &participant_seeds, now_secs())?;
            let crawled_players = read_crawled_player_records(&db, &region)?;
            plan_match_discovery(&MatchDiscoveryInput {
                now: now_secs(),
                champ_select_active: false,
                crawl_budget: MATCH_DISCOVERY_CRAWL_BUDGET,
                max_breadth: MATCH_DISCOVERY_MAX_BREADTH,
                per_player_match_cap: MATCH_DISCOVERY_PER_PLAYER_MATCH_CAP,
                seeds: participant_seeds.clone(),
                crawled_players,
                candidate_matches: Vec::new(),
                known_matches: Vec::new(),
            })
        };

        let mut discovered = Vec::new();
        for puuid_hash in &crawl_plan.to_crawl {
            let Some(raw_puuid) = raw_by_hash.get(puuid_hash) else {
                continue;
            };
            // Same rolling 2-min Riot budget guards the discovery crawl calls.
            if !crate::riot::rate_limiter::riot_budget().try_acquire() {
                break;
            }
            match match_ep::list_ids(
                riot.as_ref(),
                raw_puuid,
                routing,
                Some("ranked"),
                Some(MATCH_V5_RANKED_QUEUE),
                MATCH_DISCOVERY_MATCH_LIST_COUNT,
            )
            .await
            {
                Ok(match_ids) => {
                    let crawled_at = now_secs();
                    {
                        let db = state.db.lock().await;
                        mark_discovery_player_crawled(&db, puuid_hash, &region, crawled_at)?;
                    }
                    discovered.extend(discovered_match_candidates(
                        &match_ids, &region, puuid_hash, crawled_at,
                    ));
                }
                Err(_) => {
                    detail_errors += 1;
                }
            }
        }

        if !discovered.is_empty() {
            let discovered_ids: Vec<String> = discovered
                .iter()
                .map(|candidate| candidate.match_id.clone())
                .collect();
            let db = state.db.lock().await;
            let known_matches = read_known_match_records(&db, &discovered_ids)?;
            let intake_plan = plan_match_discovery(&MatchDiscoveryInput {
                now: now_secs(),
                champ_select_active: false,
                crawl_budget: 0,
                max_breadth: 0,
                per_player_match_cap: MATCH_DISCOVERY_PER_PLAYER_MATCH_CAP,
                seeds: Vec::new(),
                crawled_players: Vec::new(),
                candidate_matches: discovered,
                known_matches,
            });
            let new_candidates = build_match_fetch_candidates(
                &intake_plan.new_match_ids,
                &region,
                &current_patch,
                now_secs(),
            );
            record_match_fetch_candidates(&db, &new_candidates, now_secs())?;
        }
    }

    Ok(MatchV5IngestionOutcome {
        fetched_matches: aggregation.quality.match_count,
        detail_errors,
        rates: row_set.rates.len() as u32,
        matchups: row_set.matchups.len() as u32,
        builds: row_set.builds.len() as u32,
    })
}

/// Gather live coverage from the local DB and decide which source kinds are
/// contributing, whether we are on fallback, and which sources are stale.
pub(crate) fn gather_coverage(
    conn: &rusqlite::Connection,
) -> Result<(LocalCoverage, bool, Vec<String>), AppError> {
    let total_champions = count(conn, "SELECT COUNT(*) FROM champions")?;
    let champion_rates_count = count(conn, "SELECT COUNT(*) FROM champion_rates")?;
    let matchup_count = count(conn, "SELECT COUNT(*) FROM champion_matchups")?;
    let build_count = count(conn, "SELECT COUNT(*) FROM builds")?;
    let build_champions = count(conn, "SELECT COUNT(DISTINCT champion_id) FROM builds")?;
    let meta_role_champions = count(
        conn,
        "SELECT COUNT(DISTINCT champion_id) FROM champion_rates",
    )?;

    // Bundled seeds + DDragon are always present; rates imply an aggregator.
    let no_local_data = build_count == 0 && matchup_count == 0;
    let mut sources = vec![
        DataSourceEntry::from_kind(
            DataSourceKind::LocalSeed,
            Some(build_count.max(matchup_count)),
            no_local_data,
        ),
        DataSourceEntry::from_kind(DataSourceKind::ManualSeed, Some(matchup_count), false),
        DataSourceEntry::from_kind(DataSourceKind::Ddragon, None, false),
    ];
    if champion_rates_count > 0 {
        sources.push(DataSourceEntry::from_kind(
            DataSourceKind::Meraki,
            Some(champion_rates_count),
            false,
        ));
    }

    // Is a cloud data pack cached and fresh? Drives `fallback_active` + staleness.
    let cached: Option<(Option<String>, Option<i64>)> = conn
        .query_row(
            "SELECT source, expires_at FROM draft_brain_packs WHERE kind = 'data_pack'",
            [],
            |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<i64>>(1)?)),
        )
        .optional()?;

    let now = now_secs();
    let mut stale_sources = Vec::new();
    let fallback_active = match cached {
        Some((Some(ref source), expires)) if source == "cloud" => {
            sources.push(DataSourceEntry::from_kind(
                DataSourceKind::CloudPostgres,
                None,
                false,
            ));
            if expires.map(|e| e < now).unwrap_or(true) {
                stale_sources.push("data_pack".to_string());
            }
            false
        }
        Some((_, expires)) => {
            // A local/builder pack is cached — still a fallback.
            if expires.map(|e| e < now).unwrap_or(true) {
                stale_sources.push("data_pack".to_string());
            }
            true
        }
        None => {
            // Never synced → runtime uses the in-memory local seed pack.
            stale_sources.push("data_pack".to_string());
            true
        }
    };

    let coverage = LocalCoverage {
        total_champions,
        champion_rates_count,
        matchup_count,
        build_count,
        build_champions,
        meta_role_champions,
        sources,
    };
    Ok((coverage, fallback_active, stale_sources))
}

/// Read-only registry report: source kinds + risk posture + the derived signals
/// (`primary_role_build_coverage`, `exact_matchup_coverage`, `stale_sources`,
/// `high_risk_sources`, `fallback_active`). Never hits the network.
#[tauri::command]
pub async fn get_data_source_registry(
    state: State<'_, AppState>,
) -> Result<DataSourceRegistryReport, AppError> {
    let db = state.db.lock().await;
    let (coverage, fallback_active, stale) = gather_coverage(&db)?;
    Ok(compute_registry_report(
        &coverage,
        stale,
        fallback_active,
        now_secs() as u32,
    ))
}

/// Read-only production data-pipeline quality: coverage + freshness + concrete
/// refresh/cache actions. Local DB only; it never fetches network data.
#[tauri::command]
pub async fn get_pipeline_quality_report(
    state: State<'_, AppState>,
) -> Result<PipelineQualityReport, AppError> {
    current_pipeline_report(&state).await
}

/// Read-only scheduler status: current policy plan, rate-limit budget, and
/// fetch-log observability. It may query local LCU gameflow to enforce the
/// champ-select no-network guard, but it never performs external refreshes.
#[tauri::command]
pub async fn get_pipeline_scheduler_status(
    state: State<'_, AppState>,
) -> Result<PipelineSchedulerStatus, AppError> {
    build_pipeline_scheduler_status(&state).await
}

/// Read-only data trajectory: fuses the current quality status with the background
/// scheduler's most recent ramp motion into one honest user-facing token (healthy /
/// enriching / warming_up / stagnant / regressing / deferred). `unknown` until the
/// first background tick has measured a ramp. No network — local DB + in-memory state.
#[tauri::command]
pub async fn get_data_trajectory(
    state: State<'_, AppState>,
) -> Result<DataTrajectoryView, AppError> {
    let quality_status = current_pipeline_report(&state).await?.status;
    let last = state.last_coverage_ramp.lock().await.clone();
    let (ramp_state, data_growing, measured_at, trajectory) = match last {
        Some(r) => {
            let trajectory = classify_data_trajectory(&quality_status, &r.ramp_state);
            (
                r.ramp_state,
                r.data_growing,
                Some(r.measured_at.max(0) as u32),
                trajectory,
            )
        }
        None => ("unknown".to_string(), false, None, "unknown".to_string()),
    };
    // Honest Match-V5 / Riot-key status for the data badges (S1). No network — a
    // runtime-config check plus one local fetch-log read.
    let riot_key_present = runtime_riot_configured();
    let has_summoner = has_active_summoner(&state).await?;
    let match_v5_enabled = riot_key_present && has_summoner;
    let now = now_secs();
    let last_v5 = {
        let db = state.db.lock().await;
        let since = now - 30 * 24 * 60 * 60;
        last_success_at(&read_fetch_logs(&db, since)?, "match_v5")
    };
    Ok(DataTrajectoryView {
        trajectory,
        quality_status,
        ramp_state,
        data_growing,
        measured_at,
        riot_key_present,
        match_v5_enabled,
        match_v5_last_success_at: last_v5.map(|t| t.max(0) as u32),
        match_v5_age_secs: last_v5.map(|t| (now - t).max(0) as u32),
    })
}

async fn sync_data_pipeline_inner(
    state: &AppState,
) -> Result<DataPipelineRefreshSummary, AppError> {
    let before = current_pipeline_report(state).await?;
    if is_champ_select_active(state).await {
        let now = now_secs();
        for source in ["ddragon", "meraki", "match_v5"] {
            record_fetch_log_state(
                state,
                source,
                "skipped",
                "skip_champ_select",
                "Champ-select aktif; manual pipeline refresh ertelendi.",
                now,
                now,
            )
            .await;
        }
        let cached = {
            let db = state.db.lock().await;
            pack_exists(&db)?
        };
        return Ok(DataPipelineRefreshSummary {
            before_status: before.status.clone(),
            after_status: before.status,
            actions: vec!["skip_champ_select".to_string()],
            ddragon_champions: 0,
            meraki_rates: 0,
            builds_imported: 0,
            matchups_imported: 0,
            match_v5_matches: 0,
            match_v5_rates: 0,
            match_v5_matchups: 0,
            match_v5_builds: 0,
            match_v5_errors: 0,
            data_pack_cached: cached,
            cache_action: "keep_current".to_string(),
            cache_promoted: false,
            errors: vec!["champ_select_active".to_string()],
        });
    }
    let current_good = {
        let db = state.db.lock().await;
        current_cache_quality(&db, now_secs())?
    };
    let mut errors = Vec::new();

    let ddragon_started = now_secs();
    let ddragon_champions = match sync_ddragon_champions_inner(state).await {
        Ok(count) => {
            record_fetch_log_state(
                state,
                "ddragon",
                "success",
                "refresh",
                &format!("{count} champions"),
                ddragon_started,
                now_secs(),
            )
            .await;
            count as u32
        }
        Err(err) => {
            record_fetch_log_state(
                state,
                "ddragon",
                "failed",
                "refresh",
                &err.to_string(),
                ddragon_started,
                now_secs(),
            )
            .await;
            errors.push(format!("ddragon: {err}"));
            0
        }
    };

    let meraki_started = now_secs();
    let meraki_rates = match sync_meraki_rates_inner(state).await {
        Ok(count) => {
            record_fetch_log_state(
                state,
                "meraki",
                "success",
                "refresh",
                &format!("{count} rates"),
                meraki_started,
                now_secs(),
            )
            .await;
            count as u32
        }
        Err(err) => {
            record_fetch_log_state(
                state,
                "meraki",
                "failed",
                "refresh",
                &err.to_string(),
                meraki_started,
                now_secs(),
            )
            .await;
            errors.push(format!("meraki: {err}"));
            0
        }
    };

    let (builds_imported, matchups_imported) = {
        let db = state.db.lock().await;
        let builds_started = now_secs();
        let builds = match import_builds_seed(&db) {
            Ok(count) => {
                if let Err(err) = record_fetch_log(
                    &db,
                    "build_seed",
                    "success",
                    "refresh",
                    &format!("{count} builds"),
                    builds_started,
                    now_secs(),
                ) {
                    tracing::warn!("build_seed fetch log yazılamadı: {err}");
                }
                count as u32
            }
            Err(err) => {
                if let Err(log_err) = record_fetch_log(
                    &db,
                    "build_seed",
                    "failed",
                    "refresh",
                    &err.to_string(),
                    builds_started,
                    now_secs(),
                ) {
                    tracing::warn!("build_seed fetch log yazılamadı: {log_err}");
                }
                errors.push(format!("build_seed: {err}"));
                0
            }
        };
        let matchups_started = now_secs();
        let matchups = match import_matchups_seed(&db) {
            Ok(count) => {
                if let Err(err) = record_fetch_log(
                    &db,
                    "matchup_seed",
                    "success",
                    "refresh",
                    &format!("{count} matchups"),
                    matchups_started,
                    now_secs(),
                ) {
                    tracing::warn!("matchup_seed fetch log yazılamadı: {err}");
                }
                count as u32
            }
            Err(err) => {
                if let Err(log_err) = record_fetch_log(
                    &db,
                    "matchup_seed",
                    "failed",
                    "refresh",
                    &err.to_string(),
                    matchups_started,
                    now_secs(),
                ) {
                    tracing::warn!("matchup_seed fetch log yazılamadı: {log_err}");
                }
                errors.push(format!("matchup_seed: {err}"));
                0
            }
        };
        (builds, matchups)
    };

    let match_v5_started = now_secs();
    let match_v5 = match sync_match_v5_ingestion(state).await {
        Ok(outcome) => {
            record_fetch_log_state(
                state,
                "match_v5",
                "success",
                "refresh",
                &format!(
                    "{} matches, {} rates, {} matchups, {} builds, {} errors",
                    outcome.fetched_matches,
                    outcome.rates,
                    outcome.matchups,
                    outcome.builds,
                    outcome.detail_errors
                ),
                match_v5_started,
                now_secs(),
            )
            .await;
            outcome
        }
        Err(err) => {
            record_fetch_log_state(
                state,
                "match_v5",
                "failed",
                "refresh",
                &err.to_string(),
                match_v5_started,
                now_secs(),
            )
            .await;
            errors.push(format!("match_v5: {err}"));
            MatchV5IngestionOutcome::default()
        }
    };

    let current_patch = state.ddragon.lock().await.current_version().to_string();
    let (data_pack_cached, cache_action, cache_promoted) = {
        let db = state.db.lock().await;
        let cache_started = now_secs();
        let candidate = candidate_quality_from_db(&db, "local_builder")?;
        let decision = decide_cache_promotion(&candidate, current_good.as_ref());
        let cached = if decision.promoted {
            match cache_local_data_pack(&db, Some(current_patch), None, now_secs()) {
                Ok(_) => {
                    if let Err(err) = record_fetch_log(
                        &db,
                        "data_pack_cache",
                        "success",
                        &decision.action,
                        &decision.reason,
                        cache_started,
                        now_secs(),
                    ) {
                        tracing::warn!("data_pack_cache fetch log yazılamadı: {err}");
                    }
                    true
                }
                Err(err) => {
                    if let Err(log_err) = record_fetch_log(
                        &db,
                        "data_pack_cache",
                        "failed",
                        &decision.action,
                        &err.to_string(),
                        cache_started,
                        now_secs(),
                    ) {
                        tracing::warn!("data_pack_cache fetch log yazılamadı: {log_err}");
                    }
                    errors.push(format!("data_pack_cache: {err}"));
                    false
                }
            }
        } else {
            if let Err(err) = record_fetch_log(
                &db,
                "data_pack_cache",
                "skipped",
                &decision.action,
                &decision.reason,
                cache_started,
                now_secs(),
            ) {
                tracing::warn!("data_pack_cache fetch log yazılamadı: {err}");
            }
            pack_exists(&db)?
        };
        (cached, decision.action, decision.promoted)
    };

    let after = current_pipeline_report(state).await?;
    let actions = action_keys(&before);

    Ok(DataPipelineRefreshSummary {
        before_status: before.status,
        after_status: after.status,
        actions,
        ddragon_champions,
        meraki_rates,
        builds_imported,
        matchups_imported,
        match_v5_matches: match_v5.fetched_matches,
        match_v5_rates: match_v5.rates,
        match_v5_matchups: match_v5.matchups,
        match_v5_builds: match_v5.builds,
        match_v5_errors: match_v5.detail_errors,
        data_pack_cached,
        cache_action,
        cache_promoted,
        errors,
    })
}

/// Manual production-pipeline refresh. This is intentionally user-triggered:
/// champ-select recommendation latency never waits on network fetches.
#[tauri::command]
pub async fn sync_data_pipeline(
    state: State<'_, AppState>,
) -> Result<DataPipelineRefreshSummary, AppError> {
    let _refresh_guard = state.data_pipeline_refresh_lock.lock().await;
    sync_data_pipeline_inner(&state).await
}

/// Runs one manual refresh and evaluates whether live coverage actually moved.
/// This is a measurement wrapper around `sync_data_pipeline_inner`: it keeps the
/// same no-network-during-champ-select rule and adds before/after funnel counts.
#[tauri::command]
pub async fn measure_live_coverage_ramp(
    state: State<'_, AppState>,
) -> Result<LiveCoverageRampReport, AppError> {
    let _refresh_guard = state.data_pipeline_refresh_lock.lock().await;
    measure_live_coverage_ramp_inner(&state).await
}

async fn measure_live_coverage_ramp_inner(
    state: &AppState,
) -> Result<LiveCoverageRampReport, AppError> {
    let champ_select_active = is_champ_select_active(state).await;
    let riot_ready = runtime_riot_configured() && has_active_summoner(state).await?;
    let crawl_budget = if champ_select_active || !riot_ready {
        0
    } else {
        MATCH_DISCOVERY_CRAWL_BUDGET
    };
    let before = {
        let db = state.db.lock().await;
        ramp_snapshot(&db, now_secs())?
    };
    let refresh = sync_data_pipeline_inner(state).await?;
    let after = {
        let db = state.db.lock().await;
        ramp_snapshot(&db, now_secs())?
    };
    let ramp = evaluate_coverage_ramp(&CoverageRampInput {
        before: before.clone(),
        after: after.clone(),
        champ_select_active,
        crawl_budget,
    });

    Ok(LiveCoverageRampReport {
        before: ramp_snapshot_view(&before),
        after: ramp_snapshot_view(&after),
        ramp,
        refresh,
        champ_select_active,
        crawl_budget,
    })
}

/// Build a local data pack from live DB coverage (real quality, not zeros) and
/// cache it as the `data_pack` row so recommendation badges reflect it when the
/// cloud is unavailable. Returns the freshly built pack.
#[tauri::command]
pub async fn rebuild_local_data_pack(
    patch: Option<String>,
    region: Option<String>,
    state: State<'_, AppState>,
) -> Result<DataPack, AppError> {
    let db = state.db.lock().await;
    cache_local_data_pack(&db, patch, region, now_secs())
}

/// Read-only feedback observability summary for the data-quality surface: total /
/// polar / neutral feedback, how many champions carry a signal, and how many rows
/// await cloud sync. Never hits the network — local DB only.
#[tauri::command]
pub async fn get_feedback_observability(
    state: State<'_, AppState>,
) -> Result<FeedbackObservabilityReport, AppError> {
    let db = state.db.lock().await;
    let mut stmt =
        db.prepare("SELECT champion_id, feedback, synced_at FROM recommendation_feedback")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<i64>>(2)?,
        ))
    })?;

    let mut inputs = Vec::new();
    let mut pending_sync = 0u32;
    for row in rows {
        let (champion_id, verdict, synced_at) = row?;
        if synced_at.is_none() {
            pending_sync += 1;
        }
        inputs.push(FeedbackInput {
            champion_id: champion_id.max(0) as u32,
            verdict,
        });
    }
    Ok(build_feedback_observability_report(&inputs, pending_sync))
}

fn build_feedback_observability_report(
    inputs: &[FeedbackInput],
    pending_sync: u32,
) -> FeedbackObservabilityReport {
    let counters = summarize_observability(inputs, pending_sync);
    let status = personalization_status(&counters);
    FeedbackObservabilityReport { counters, status }
}

/// Read-only feedback analytics: per-champion sentiment trend, recent-window signal
/// count, and the "which recommendations is the user disliking?" list. `window_days`
/// defaults to 7. Local DB only — never hits the network.
#[tauri::command]
pub async fn get_feedback_analytics(
    window_days: Option<u32>,
    state: State<'_, AppState>,
) -> Result<FeedbackAnalytics, AppError> {
    let db = state.db.lock().await;
    let mut stmt = db.prepare(
        "SELECT champion_id, champion_key, feedback, created_at FROM recommendation_feedback",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(FeedbackEvent {
            champion_id: r.get::<_, i64>(0)?.max(0) as u32,
            champion_key: r.get::<_, String>(1)?,
            verdict: r.get::<_, String>(2)?,
            created_at: r.get::<_, i64>(3)?,
        })
    })?;
    let events: Vec<FeedbackEvent> = rows.collect::<Result<_, _>>()?;
    Ok(analyze_feedback(
        &events,
        now_secs(),
        window_days.unwrap_or(7),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{open_db, run_migrations};
    use serde_json::json;
    use std::{path::PathBuf, sync::Arc};
    use tempfile::tempdir;
    use tokio::sync::Mutex;

    fn memory_db() -> rusqlite::Connection {
        let dir = tempdir().unwrap();
        let mut conn = open_db(&dir.path().join("dq.db")).unwrap();
        run_migrations(&mut conn).unwrap();
        // Leak the tempdir so the file outlives the test connection.
        std::mem::forget(dir);
        conn
    }

    fn insert_matchup_rows(conn: &rusqlite::Connection, count: u32) {
        for idx in 0..count {
            let role = MATCH_V5_ROLES[(idx as usize) % MATCH_V5_ROLES.len()];
            let champion_id = 10_000 + idx as i64;
            let opponent_id = 20_000 + idx as i64;
            conn.execute(
                "INSERT INTO champion_matchups
                     (champion_id, opponent_id, position, games, wins, win_rate,
                      source, patch_version, cached_at, region, confidence, sample_size)
                 VALUES (?1, ?2, ?3, 1, 1, 1.0, ?4, '16.10', 1, 'euw1', 'low', 1)",
                params![champion_id, opponent_id, role, MATCH_V5_SOURCE],
            )
            .unwrap();
        }
    }

    #[test]
    fn match_v5_scheduler_ttl_stays_warm_until_matchup_target() {
        let conn = memory_db();
        insert_matchup_rows(&conn, MATCH_V5_TARGET_MATCHUPS - 1);

        assert_eq!(
            match_v5_scheduler_ttl(&conn).unwrap(),
            MATCH_V5_WARMUP_TTL_SECS,
            "below the quality target, Match-V5 should keep collecting small background batches"
        );
    }

    #[test]
    fn match_v5_scheduler_ttl_backs_off_after_matchup_target() {
        let conn = memory_db();
        insert_matchup_rows(&conn, MATCH_V5_TARGET_MATCHUPS);

        assert_eq!(
            match_v5_scheduler_ttl(&conn).unwrap(),
            MATCH_V5_STABLE_TTL_SECS,
            "after reaching the target, Match-V5 should back off to the stable TTL"
        );
    }

    fn live_db_path() -> PathBuf {
        std::env::var("CSA_LIVE_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let appdata = std::env::var("APPDATA").expect("APPDATA is required on Windows");
                PathBuf::from(appdata)
                    .join("com.aslan.champ-select-assistant")
                    .join("champ-select.db")
            })
    }

    fn live_riot_client() -> Option<Arc<crate::riot::RiotClient>> {
        crate::riot::client::runtime_client_from_env()
    }

    fn live_app_state(conn: rusqlite::Connection) -> AppState {
        AppState {
            lcu_client: Mutex::new(None),
            session_poller_handle: Mutex::new(None),
            session_poller_cancel: Mutex::new(None),
            lcu_ws_handle: Mutex::new(None),
            lcu_ws_cancel: Mutex::new(None),
            gameflow_watcher_cancel: Mutex::new(None),
            data_pipeline_scheduler_handle: Mutex::new(None),
            data_pipeline_scheduler_cancel: Mutex::new(None),
            data_pipeline_refresh_lock: Mutex::new(()),
            last_coverage_ramp: Mutex::new(None),
            db: Arc::new(Mutex::new(conn)),
            ddragon: Mutex::new(crate::ddragon::DdragonCache::new()),
            riot: live_riot_client(),
            items_cache: Mutex::new(Vec::new()),
            rune_trees_cache: Mutex::new(Vec::new()),
            draft_iq: Arc::new(
                crate::recommendation::draft_iq::DraftKnowledgeBase::load()
                    .expect("Draft IQ KB loads"),
            ),
        }
    }

    #[tokio::test]
    #[ignore = "manual live Riot/API smoke; mutates the local app DB"]
    async fn live_measure_coverage_ramp_smoke() {
        let _ = dotenvy::dotenv();
        assert!(
            crate::riot::client::runtime_riot_configured(),
            "RIOT_API_KEY or PROXY_URL must be set for live ramp smoke"
        );
        let path = live_db_path();
        let mut conn = open_db(&path).expect("open live app DB");
        run_migrations(&mut conn).expect("run live migrations");
        let state = live_app_state(conn);
        let report = measure_live_coverage_ramp_inner(&state)
            .await
            .expect("live coverage ramp measurement");
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    }

    #[test]
    fn gather_coverage_on_empty_db_is_fallback() {
        let conn = memory_db();
        let (coverage, fallback_active, stale) = gather_coverage(&conn).unwrap();
        assert!(fallback_active, "no cloud pack → fallback active");
        assert!(
            stale.contains(&"data_pack".to_string()),
            "never synced → stale"
        );
        // Local seed + manual seed + ddragon are always registered.
        assert!(coverage.sources.iter().any(|s| s.source == "local_seed"));
        assert_eq!(coverage.build_count, 0);
    }

    #[test]
    fn rebuilt_pack_is_cached_and_fallback() {
        let conn = memory_db();
        let (coverage, _f, _s) = gather_coverage(&conn).unwrap();
        let pack = build_local_data_pack(&coverage, Some("16.11".into()), None);
        assert!(pack.fallback);

        // Cache it (same SQL the command uses) and read it back.
        let now = now_secs();
        let payload = serde_json::to_string(&pack).unwrap();
        conn.execute(
            "INSERT INTO draft_brain_packs (kind, version, payload_json, source, fetched_at, expires_at)
             VALUES ('data_pack', ?1, ?2, 'local_builder', ?3, ?4)
             ON CONFLICT(kind) DO UPDATE SET version=excluded.version, payload_json=excluded.payload_json,
                 source=excluded.source, fetched_at=excluded.fetched_at, expires_at=excluded.expires_at",
            params![pack.version, payload, now, now + PACK_TTL_SECS],
        )
        .unwrap();

        let (_cov, fallback_active, _stale) = gather_coverage(&conn).unwrap();
        assert!(
            fallback_active,
            "a local_builder pack is still a fallback (not cloud)"
        );
    }

    #[test]
    fn feedback_observability_reads_rows_and_counts_pending() {
        let conn = memory_db();
        let now = now_secs();
        let seed: [(i64, &str, Option<i64>); 4] = [
            (238, "helpful", None),      // unsynced polar
            (238, "helpful", None),      // unsynced polar
            (238, "helpful", Some(now)), // synced polar → champ 238 has 3 polar
            (99, "skipped", None),       // unsynced neutral
        ];
        for (cid, verdict, synced) in seed {
            conn.execute(
                "INSERT INTO recommendation_feedback
                     (champion_id, champion_key, feedback, synced_at, created_at)
                 VALUES (?1, 'X', ?2, ?3, ?4)",
                params![cid, verdict, synced, now],
            )
            .unwrap();
        }

        // Mirror the command's read path against the local DB.
        let mut stmt = conn
            .prepare("SELECT champion_id, feedback, synced_at FROM recommendation_feedback")
            .unwrap();
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                ))
            })
            .unwrap();
        let mut inputs = Vec::new();
        let mut pending = 0u32;
        for row in rows {
            let (cid, verdict, synced) = row.unwrap();
            if synced.is_none() {
                pending += 1;
            }
            inputs.push(FeedbackInput {
                champion_id: cid.max(0) as u32,
                verdict,
            });
        }
        let s = summarize_observability(&inputs, pending);
        assert_eq!(s.total, 4);
        assert_eq!(s.polar, 3, "3 helpful are polar");
        assert_eq!(s.neutral, 1, "1 skipped is neutral");
        assert_eq!(s.pending_sync, 3, "3 rows have NULL synced_at");
        assert_eq!(
            s.active_champion_signals, 1,
            "only champ 238 carries a signal"
        );

        let report = build_feedback_observability_report(&inputs, pending);
        assert_eq!(report.counters, s);
        assert_eq!(
            report.status,
            FeedbackPersonalizationStatus::NeedsSync,
            "pending sync has top priority"
        );
    }

    #[test]
    fn canonical_upsert_persists_ingestion_metadata() {
        let conn = memory_db();
        let rows = CanonicalRowSet {
            region: "tr1".to_string(),
            rates: vec![
                crate::recommendation::ingestion_contract::CanonicalRateRow {
                    region: "tr1".to_string(),
                    patch: "16.10".to_string(),
                    champion_id: 238,
                    position: "middle".to_string(),
                    win_rate: 0.52,
                    pick_rate: 0.08,
                    ban_rate: 0.0,
                    sample_size: 120,
                    source: "riot_match_v5".to_string(),
                    confidence: "high".to_string(),
                },
            ],
            matchups: vec![
                crate::recommendation::ingestion_contract::CanonicalMatchupRow {
                    region: "tr1".to_string(),
                    patch: "16.10".to_string(),
                    champion_id: 238,
                    opponent_id: 61,
                    position: "middle".to_string(),
                    games: 34,
                    wins: 19,
                    win_rate: 19.0 / 34.0,
                    sample_size: 34,
                    source: "riot_match_v5".to_string(),
                    confidence: "medium".to_string(),
                },
            ],
            builds: vec![
                crate::recommendation::ingestion_contract::CanonicalBuildRow {
                    region: "tr1".to_string(),
                    patch: "16.10".to_string(),
                    champion_id: 238,
                    position: "middle".to_string(),
                    item_ids: vec![3142, 6691, 3814],
                    rune_ids: vec![8112, 8143],
                    summoner_spells: vec![4, 14],
                    games: 27,
                    win_rate: 0.55,
                    pick_rate: 0.08,
                    sample_size: 27,
                    source: "riot_match_v5".to_string(),
                    confidence: "medium".to_string(),
                },
            ],
        };

        upsert_canonical_rows(&conn, &rows).expect("canonical rows upsert");

        let rate: (String, String, i64) = conn
            .query_row(
                "SELECT region, confidence, sample_size
                 FROM champion_rates
                 WHERE champion_id = 238 AND position = 'middle' AND source = 'riot_match_v5'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(rate, ("tr1".to_string(), "high".to_string(), 120));

        let matchup: (String, String, i64) = conn
            .query_row(
                "SELECT region, confidence, sample_size
                 FROM champion_matchups
                 WHERE champion_id = 238 AND opponent_id = 61 AND source = 'riot_match_v5'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(matchup, ("tr1".to_string(), "medium".to_string(), 34));

        let build: (String, String, i64, String) = conn
            .query_row(
                "SELECT region, confidence, games, summoner_spells
                 FROM builds
                 WHERE champion_id = 238 AND position = 'middle' AND source = 'riot_match_v5'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            build,
            (
                "tr1".to_string(),
                "medium".to_string(),
                27,
                "[4,14]".to_string()
            )
        );
    }

    fn planner_decision<'a>(
        plan: &'a crate::recommendation::match_fetch_planner::MatchFetchPlan,
        match_id: &str,
    ) -> &'a crate::recommendation::match_fetch_planner::MatchFetchDecision {
        plan.decisions
            .iter()
            .find(|decision| decision.match_id == match_id)
            .expect("decision exists")
    }

    #[test]
    fn match_fetch_history_dedups_processed_and_retries_failed() {
        let conn = memory_db();
        let now = now_secs();
        let ids = vec!["TR1_A".to_string(), "TR1_B".to_string()];
        let candidates = build_match_fetch_candidates(&ids, "tr1", "16.10", now);
        record_match_fetch_candidates(&conn, &candidates, now).unwrap();
        mark_match_fetch_processed(&conn, "TR1_A", "tr1", "16.10", 420, now).unwrap();
        mark_match_fetch_failed(&conn, "TR1_B", "tr1", "16.10", "detail_fetch_failed", now)
            .unwrap();

        let records = read_match_fetch_records(&conn, &ids).unwrap();
        let plan = plan_match_fetch(&MatchFetchPlannerInput {
            now,
            champ_select_active: false,
            rate_budget: 10,
            batch_limit: 10,
            candidates,
            fetched_records: records,
            coverage_gaps: vec![CoverageGap {
                region: "tr1".to_string(),
                patch: "16.10".to_string(),
                role: "middle".to_string(),
                current_samples: 0,
                target_samples: MATCH_V5_ROLE_TARGET_SAMPLES,
                priority: 10,
            }],
        });

        assert_eq!(
            planner_decision(&plan, "TR1_A").decision,
            "skip_already_fetched",
            "processed match ids must be deduped"
        );
        assert_eq!(
            planner_decision(&plan, "TR1_B").decision,
            "fetch",
            "failed detail fetches remain retryable"
        );
        assert_eq!(plan.to_fetch, vec!["TR1_B".to_string()]);
    }

    #[test]
    fn match_discovery_players_store_hash_only_and_crawl_state() {
        let conn = memory_db();
        let now = now_secs();
        let raw_puuid = "raw-puuid-never-persisted";
        let puuid_hash = hash_puuid(raw_puuid);
        let seed = discovery_seed(puuid_hash.clone(), "tr1", "match_participant", now, 2);

        upsert_discovery_seeds(&conn, &[seed], now).unwrap();
        mark_discovery_player_crawled(&conn, &puuid_hash, "tr1", now + 1).unwrap();

        let stored_raw_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM match_discovery_players WHERE puuid_hash = ?1",
                params![raw_puuid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored_raw_count, 0, "raw PUUID must not be persisted");

        let row: (String, String, i64, i64, i64) = conn
            .query_row(
                "SELECT puuid_hash, source, contribution_count, last_crawled_at, crawl_count
                 FROM match_discovery_players
                 WHERE puuid_hash = ?1",
                params![puuid_hash.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(
            row,
            (
                puuid_hash.clone(),
                "match_participant".to_string(),
                2,
                now + 1,
                1
            )
        );

        let crawled = read_crawled_player_records(&conn, "tr1").unwrap();
        assert_eq!(crawled.len(), 1);
        assert_eq!(crawled[0].puuid_hash, puuid_hash);
    }

    #[test]
    fn lcu_uuid_puuid_is_detected_before_match_v5_calls() {
        assert!(
            is_lcu_uuid_puuid("fc2a1dbd-6a47-5e41-a65b-436ea68eb7f4"),
            "LCU UUID-style PUUID must be resolved through account-v1 before Match-V5"
        );
        assert!(
            !is_lcu_uuid_puuid(
                "ZRcMXoVY_this_is_a_long_riot_account_puuid_candidate_without_uuid_shape_123456"
            ),
            "long Riot account-v1 PUUID candidates should not be rejected by the UUID guard"
        );
        assert!(
            !is_lcu_uuid_puuid("raw-puuid-never-persisted"),
            "ordinary test strings are not UUID-like LCU PUUIDs"
        );
    }

    #[test]
    fn participant_extraction_dedups_and_hashes_without_raw_columns() {
        let detail = json!({
            "info": {
                "participants": [
                    { "puuid": "p1" },
                    { "puuid": "p2" },
                    { "puuid": "p1" },
                    { "puuid": "" },
                    { "championId": 99 }
                ]
            }
        });
        let puuids = participant_puuids_from_detail(&detail);
        assert_eq!(puuids, vec!["p1".to_string(), "p2".to_string()]);

        let (seeds, raw_by_hash) = aggregate_participant_seeds(&puuids, "tr1", 10);
        assert_eq!(seeds.len(), 2);
        assert!(seeds.iter().all(|seed| seed.source == "match_participant"));
        assert!(seeds
            .iter()
            .all(|seed| seed.puuid_hash != "p1" && seed.puuid_hash != "p2"));
        assert_eq!(raw_by_hash.len(), 2, "raw PUUID stays transient only");
    }

    #[test]
    fn discovery_intake_records_new_match_ids_for_future_fetch() {
        let conn = memory_db();
        let now = now_secs();
        let existing = vec!["TR1_EXISTING".to_string()];
        let existing_candidates = build_match_fetch_candidates(&existing, "tr1", "16.10", now);
        record_match_fetch_candidates(&conn, &existing_candidates, now).unwrap();
        mark_match_fetch_processed(&conn, "TR1_EXISTING", "tr1", "16.10", 420, now).unwrap();

        let discovered = vec![
            DiscoveredMatchCandidate {
                match_id: "TR1_EXISTING".to_string(),
                region: "tr1".to_string(),
                source_puuid_hash: "h1".to_string(),
                discovered_at: now,
            },
            DiscoveredMatchCandidate {
                match_id: "TR1_NEW".to_string(),
                region: "tr1".to_string(),
                source_puuid_hash: "h1".to_string(),
                discovered_at: now - 1,
            },
        ];
        let known =
            read_known_match_records(&conn, &["TR1_EXISTING".to_string(), "TR1_NEW".to_string()])
                .unwrap();
        let intake_plan = plan_match_discovery(&MatchDiscoveryInput {
            now,
            champ_select_active: false,
            crawl_budget: 0,
            max_breadth: 0,
            per_player_match_cap: 5,
            seeds: Vec::new(),
            crawled_players: Vec::new(),
            candidate_matches: discovered,
            known_matches: known,
        });
        assert_eq!(intake_plan.new_match_ids, vec!["TR1_NEW".to_string()]);

        let new_candidates =
            build_match_fetch_candidates(&intake_plan.new_match_ids, "tr1", "16.10", now);
        record_match_fetch_candidates(&conn, &new_candidates, now).unwrap();
        let pending = read_pending_discovered_match_ids(&conn, "tr1", 10).unwrap();
        assert_eq!(pending, vec!["TR1_NEW".to_string()]);
    }

    #[test]
    fn ramp_snapshot_counts_pipeline_rows_and_fetch_funnel() {
        let conn = memory_db();
        let now = now_secs();
        upsert_canonical_rows(
            &conn,
            &CanonicalRowSet {
                region: "tr1".to_string(),
                rates: vec![
                    crate::recommendation::ingestion_contract::CanonicalRateRow {
                        region: "tr1".to_string(),
                        patch: "16.10".to_string(),
                        champion_id: 238,
                        position: "middle".to_string(),
                        win_rate: 0.52,
                        pick_rate: 0.08,
                        ban_rate: 0.0,
                        sample_size: 120,
                        source: MATCH_V5_SOURCE.to_string(),
                        confidence: "high".to_string(),
                    },
                ],
                matchups: vec![
                    crate::recommendation::ingestion_contract::CanonicalMatchupRow {
                        region: "tr1".to_string(),
                        patch: "16.10".to_string(),
                        champion_id: 238,
                        opponent_id: 61,
                        position: "middle".to_string(),
                        games: 34,
                        wins: 19,
                        win_rate: 19.0 / 34.0,
                        sample_size: 34,
                        source: MATCH_V5_SOURCE.to_string(),
                        confidence: "medium".to_string(),
                    },
                ],
                builds: vec![
                    crate::recommendation::ingestion_contract::CanonicalBuildRow {
                        region: "tr1".to_string(),
                        patch: "16.10".to_string(),
                        champion_id: 238,
                        position: "middle".to_string(),
                        item_ids: vec![3142, 6691, 3814],
                        rune_ids: vec![8112, 8143],
                        summoner_spells: vec![4, 14],
                        games: 27,
                        win_rate: 0.55,
                        pick_rate: 0.08,
                        sample_size: 27,
                        source: MATCH_V5_SOURCE.to_string(),
                        confidence: "medium".to_string(),
                    },
                ],
            },
        )
        .unwrap();
        let discovered =
            build_match_fetch_candidates(&["TR1_DISC".to_string()], "tr1", "16.10", now);
        record_match_fetch_candidates(&conn, &discovered, now).unwrap();
        conn.execute(
            "INSERT INTO match_v5_fetch_history
                 (match_id, region, patch, queue_id, status, discovered_at, fetched_at, updated_at)
             VALUES ('TR1_FETCHED', 'tr1', '16.10', 420, 'fetched', ?1, ?1, ?1)",
            params![now],
        )
        .unwrap();
        mark_match_fetch_processed(&conn, "TR1_PROC", "tr1", "16.10", 420, now).unwrap();
        mark_match_fetch_failed(&conn, "TR1_FAIL", "tr1", "16.10", "detail", now).unwrap();
        upsert_discovery_seeds(
            &conn,
            &[
                discovery_seed("h1".to_string(), "tr1", "match_participant", now, 1),
                discovery_seed("h2".to_string(), "tr1", "match_participant", now, 1),
            ],
            now,
        )
        .unwrap();

        let snapshot = ramp_snapshot(&conn, now).unwrap();
        assert_eq!(snapshot.champion_rate_rows, 1);
        assert_eq!(snapshot.matchup_rows, 1);
        assert_eq!(snapshot.build_rows, 1);
        assert_eq!(snapshot.discovered_matches, 1);
        assert_eq!(snapshot.fetched_matches, 1);
        assert_eq!(snapshot.processed_matches, 1);
        assert_eq!(snapshot.failed_matches, 1);
        assert_eq!(snapshot.crawled_players, 2);

        let view = ramp_snapshot_view(&snapshot);
        assert_eq!(view.taken_at, now as u32);
        assert_eq!(view.processed_matches, 1);
    }

    #[test]
    fn match_fetch_coverage_gaps_reflect_riot_match_v5_samples() {
        let conn = memory_db();
        let initial = build_match_fetch_coverage_gaps(&conn, "tr1", "16.10", false).unwrap();
        assert_eq!(
            initial.len(),
            MATCH_V5_ROLES.len(),
            "empty Riot data leaves every role below target"
        );

        champion_rates_repo::upsert_rate_with_region(
            &conn,
            &champion_rates_repo::ChampionRateRow {
                champion_id: 238,
                position: "middle".to_string(),
                win_rate: 0.52,
                pick_rate: 0.08,
                ban_rate: 0.0,
                sample_size: MATCH_V5_ROLE_TARGET_SAMPLES,
                patch: "16.10".to_string(),
                source: MATCH_V5_SOURCE.to_string(),
                confidence: "high".to_string(),
            },
            "tr1",
        )
        .unwrap();

        let rate_only = build_match_fetch_coverage_gaps(&conn, "tr1", "16.10", false).unwrap();
        assert!(
            rate_only.iter().any(|gap| gap.role == "middle"),
            "rate coverage alone is not enough when matchup/build samples are missing"
        );

        upsert_canonical_rows(
            &conn,
            &CanonicalRowSet {
                region: "tr1".to_string(),
                rates: vec![
                    crate::recommendation::ingestion_contract::CanonicalRateRow {
                        region: "tr1".to_string(),
                        patch: "16.10".to_string(),
                        champion_id: 238,
                        position: "middle".to_string(),
                        win_rate: 0.52,
                        pick_rate: 0.08,
                        ban_rate: 0.0,
                        sample_size: MATCH_V5_ROLE_TARGET_SAMPLES,
                        source: MATCH_V5_SOURCE.to_string(),
                        confidence: "high".to_string(),
                    },
                ],
                matchups: vec![
                    crate::recommendation::ingestion_contract::CanonicalMatchupRow {
                        region: "tr1".to_string(),
                        patch: "16.10".to_string(),
                        champion_id: 238,
                        opponent_id: 61,
                        position: "middle".to_string(),
                        games: MATCH_V5_ROLE_TARGET_SAMPLES,
                        wins: MATCH_V5_ROLE_TARGET_SAMPLES / 2,
                        win_rate: 0.5,
                        sample_size: MATCH_V5_ROLE_TARGET_SAMPLES,
                        source: MATCH_V5_SOURCE.to_string(),
                        confidence: "high".to_string(),
                    },
                ],
                builds: vec![
                    crate::recommendation::ingestion_contract::CanonicalBuildRow {
                        region: "tr1".to_string(),
                        patch: "16.10".to_string(),
                        champion_id: 238,
                        position: "middle".to_string(),
                        item_ids: vec![3142, 6691, 3814],
                        rune_ids: vec![8112, 8143],
                        summoner_spells: vec![4, 14],
                        games: MATCH_V5_ROLE_TARGET_SAMPLES,
                        win_rate: 0.5,
                        pick_rate: 0.08,
                        sample_size: MATCH_V5_ROLE_TARGET_SAMPLES,
                        source: MATCH_V5_SOURCE.to_string(),
                        confidence: "high".to_string(),
                    },
                ],
            },
        )
        .unwrap();

        let gaps = build_match_fetch_coverage_gaps(&conn, "tr1", "16.10", false).unwrap();
        assert!(
            !gaps.iter().any(|gap| gap.role == "middle"),
            "middle closes only after rate + matchup + build coverage reach target"
        );
        assert!(
            gaps.iter().any(|gap| gap.role == "top"
                && gap.current_samples == 0
                && gap.target_samples == MATCH_V5_ROLE_TARGET_SAMPLES),
            "uncovered roles stay active gaps"
        );
    }
}
