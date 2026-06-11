use super::draft_brain::{active_data_pack, active_model_pack};
use crate::db::{
    builds_repo, champion_meta_repo, champion_rates_repo, champion_repo, mastery_repo, match_repo,
    matchup_repo,
};
use crate::ddragon::cdragon;
use crate::errors::AppError;
use crate::lcu::champ_pool;
use crate::lcu::{parse_session, ChampSelectState};
use crate::recommendation::ban_advisor::compute_ban_suggestions;
use crate::recommendation::build_advisor::{counter_item_advice, heuristic_build, CounterItemHint};
use crate::recommendation::draft_brain::{
    upgrade_recommendation_with_context, upgrade_recommendations_with_context,
};
use crate::recommendation::draft_iq::archetype::{ChampionArchetype, PowerCurve};
use crate::recommendation::draft_iq::game_plan::{compute_game_plan, GamePlan};
use crate::recommendation::draft_iq::narrative::{self, build_matchup_tips};
use crate::recommendation::draft_iq::DraftKnowledgeBase;
use crate::recommendation::feedback_signal::{aggregate_feedback, FeedbackInput, FeedbackSignal};
use crate::recommendation::rate_blend;
use crate::recommendation::scoring::matchup_score;
use crate::recommendation::scoring::resolve_lane_opponent_raw;
use crate::recommendation::scoring::MatchupEntry;
use crate::recommendation::scoring::MetaRate;
use crate::recommendation::team_analysis::{compute_comp_summary, TeamCompBoard, TeamComposition};
use crate::recommendation::{
    analyze_champion, compute_counter_picks, compute_draft_verdict, compute_recommendations,
    suggest_pool, BanSuggestion, CounterPickHint, DraftVerdict, PoolSuggestion, Recommendation,
    ScoringContext, ScoringWeights,
};
use crate::AppState;
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, State};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

/// Cross-source blend of `champion_rates` rows for one lane into per-champion
/// `MetaRate`s (Faz B3). With several sources (Meraki, u.gg, Match-V5) now writing
/// one row per source, a naive `collect()` would keep an arbitrary source; this
/// blends them honestly via [`rate_blend::blend_rates`].
fn blend_position_rates(
    rows: Vec<champion_rates_repo::ChampionRateRow>,
) -> HashMap<(u32, String), MetaRate> {
    rate_blend::blend_rates(
        rows.into_iter()
            .map(|r| rate_blend::SourceRate {
                champion_id: r.champion_id,
                position: r.position,
                win_rate: r.win_rate,
                ban_rate: r.ban_rate,
                sample_size: r.sample_size,
            })
            .collect(),
    )
}

/// Read a DB query result, logging (not swallowing) a failure before falling back
/// to the default — so a corrupt schema / missing migration is observable instead
/// of silently degrading recommendations (Faz B).
fn or_warn_default<T: Default>(result: anyhow::Result<T>, what: &str) -> T {
    result.unwrap_or_else(|e| {
        tracing::warn!("{what} okunamadı, boş kullanılıyor: {e}");
        T::default()
    })
}

/// Honest "what data is this pick missing" flags (Faz A). A signal is listed only
/// when it genuinely fell back to a neutral placeholder, so the UI can say "no X
/// data" instead of implying a computed score. Computed before `upgrade_*`
/// overwrites the engine's per-champ matchup confidence.
fn compute_missing_signals(
    rec: &Recommendation,
    meta_rates: &HashMap<(u32, String), MetaRate>,
    role: &str,
) -> Vec<String> {
    let mut out = Vec::new();
    // Meta: no blended champion_rates row for this champion+lane.
    if !meta_rates.contains_key(&(rec.champion_id, role.to_string())) {
        out.push("meta".to_string());
    }
    // Matchup: opponent is known but we have no real matchup data (engine → "heuristic").
    if rec.matchup_confidence == "heuristic" {
        out.push("matchup".to_string());
    }
    // Build: only a generic archetype default (or nothing), not a real build.
    if matches!(rec.build_source.as_str(), "general" | "none" | "") {
        out.push("build".to_string());
    }
    out
}

fn find_own_pick_action_id(session_json: &serde_json::Value, local_cell_id: i32) -> Option<u64> {
    let action_groups = session_json["actions"].as_array()?;
    for group in action_groups {
        for action in group.as_array()? {
            if action["actorCellId"].as_i64()? as i32 == local_cell_id
                && action["type"].as_str()? == "pick"
                && !action["completed"].as_bool().unwrap_or(true)
            {
                return action["id"].as_u64();
            }
        }
    }
    None
}

#[tauri::command]
pub async fn hover_champion(champion_id: u32, state: State<'_, AppState>) -> Result<(), AppError> {
    let client_guard = state.lcu_client.lock().await;
    let client = client_guard.as_ref().ok_or(AppError::NotConnected)?;

    let session = client
        .get_raw("/lol-champ-select/v1/session")
        .await
        .map_err(|e| AppError::Lcu(e.to_string()))?;

    let local_cell_id = session["localPlayerCellId"].as_i64().unwrap_or(0) as i32;

    let action_id = find_own_pick_action_id(&session, local_cell_id)
        .ok_or_else(|| AppError::Lcu("Aktif pick action bulunamadı".to_string()))?;

    let body = serde_json::json!({ "championId": champion_id });
    let url = format!("/lol-champ-select/v1/session/actions/{}", action_id);
    client
        .patch_json(&url, &body)
        .await
        .map_err(|e| AppError::Lcu(e.to_string()))
}

#[tauri::command]
pub async fn start_ws_listener(app: AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    // Verify LCU is connected before starting the watcher.
    {
        let guard = state.lcu_client.lock().await;
        if guard.is_none() {
            return Err(AppError::NotConnected);
        }
    }

    // Load the current lockfile — the WS connector needs the per-launch
    // port/password rather than the HTTP client's pre-built auth header.
    let lockfile = crate::lcu::find_lockfile()
        .map_err(|e| AppError::Lcu(format!("Lockfile bulunamadı: {e}")))?;

    // Stop the HTTP session poller — WS will take over as the single dispatcher.
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

    // Cancel any previous WS watcher gracefully before spawning a new one.
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

    let cancel = CancellationToken::new();
    let new_handle = tokio::spawn(crate::lcu::websocket::start_champ_select_watcher(
        app,
        lockfile,
        cancel.clone(),
    ));

    *state.lcu_ws_cancel.lock().await = Some(cancel);
    *state.lcu_ws_handle.lock().await = Some(new_handle);

    tracing::info!("Champ-select WS watcher başlatıldı, HTTP poller durduruldu");
    Ok(())
}

/// Owned bundle of everything the pure scoring engine needs, loaded from the DB
/// and in-memory caches. Both `get_recommendations` and `get_champion_analysis`
/// build a `ScoringContext` that borrows these fields, so the engine stays I/O-
/// free and both commands share one loading path (no divergence).
struct ScoringInputs {
    mastery: Vec<mastery_repo::MasteryRow>,
    stats: Vec<crate::riot::client::ChampionStats>,
    all_champions: Vec<champion_repo::ChampionRecord>,
    role_map: HashMap<u32, Vec<String>>,
    meta_rates: HashMap<(u32, String), MetaRate>,
    matchups: HashMap<(u32, u32), MatchupEntry>,
    feedback_signals: HashMap<u32, FeedbackSignal>,
    weights: ScoringWeights,
    items: Vec<cdragon::ItemData>,
    rune_trees: Vec<cdragon::RuneTree>,
    power_curves: HashMap<u32, PowerCurve>,
    /// Lane key used for meta_rates / matchups / build lookups.
    /// ARAM (450) → synthetic "aram"; otherwise the assigned LCU position lowercase.
    my_pos: String,
}

/// Decode the `session_json` command argument into a typed `ChampSelectState`.
/// The frontend round-trips the already-parsed state (snake_case, no `actions`);
/// raw LCU sessions (tests / direct passthrough) carry an `actions` array +
/// camelCase fields and go through `parse_session`.
fn parse_session_arg(session_json: serde_json::Value) -> Result<ChampSelectState, AppError> {
    if session_json.get("actions").is_some() {
        parse_session(&session_json)
            .ok_or_else(|| AppError::Other("Geçersiz session JSON".to_string()))
    } else {
        Ok(serde_json::from_value(session_json)?)
    }
}

fn read_feedback_rows(db: &rusqlite::Connection) -> Result<Vec<FeedbackInput>, AppError> {
    let mut stmt = db.prepare("SELECT champion_id, feedback FROM recommendation_feedback")?;
    let rows = stmt.query_map([], |row| {
        let champion_id: i64 = row.get(0)?;
        let verdict: String = row.get(1)?;
        Ok((champion_id, verdict))
    })?;

    let mut feedback = Vec::new();
    for row in rows {
        let (champion_id, verdict) = row?;
        if let Ok(champion_id) = u32::try_from(champion_id) {
            feedback.push(FeedbackInput {
                champion_id,
                verdict,
            });
        }
    }
    Ok(feedback)
}

/// Load all scoring inputs (mastery, stats, champions, role map, meta rates,
/// matchups, weights, items, runes, power curves) for the given session. Holds
/// the DB lock only for the reads, then releases it.
async fn load_scoring_inputs(
    state: &State<'_, AppState>,
    session: &ChampSelectState,
    puuid: &str,
) -> Result<ScoringInputs, AppError> {
    // ARAM (queue_id 450) is laneless — look up rates under the synthetic "aram"
    // key. Summoner's Rift uses the assigned LCU position lowercase.
    let my_pos = if session.queue_id == 450 {
        "aram".to_string()
    } else {
        session.local_player.assigned_position.to_lowercase()
    };

    let (
        mastery,
        stats,
        all_champions,
        role_map,
        meta_rates,
        settings,
        matchup_rows,
        feedback_rows,
    ) = {
        let db = state.db.lock().await;

        let mastery = mastery_repo::top_for_puuid(&db, puuid, 40)?;
        let stats = match_repo::player_stats(&db, puuid)?;
        let all_champions = champion_repo::list_all(&db)?;

        let meta_map = champion_meta_repo::get_all(&db)?;
        let role_map: HashMap<u32, Vec<String>> = meta_map
            .into_iter()
            .map(|(id, row)| (id, champion_meta_repo::parse_roles(&row.roles)))
            .collect();

        // Patch-level rates from champion_rates (V008), scoped to the lane, blended
        // across all sources (Meraki, u.gg, Match-V5) per champion.
        let mut meta_rates: HashMap<(u32, String), MetaRate> =
            blend_position_rates(or_warn_default(
                champion_rates_repo::get_all_for_position(&db, &my_pos),
                "champion_rates",
            ));
        // Role-share filter: u.gg reports every champion in every lane, so an off-role
        // row (e.g. ~1%-of-his-games "bottom Olaf") clears a flat sample floor and used
        // to leak into the lane pool. Keep a lane only when it's a real share of the
        // champion's cross-lane play, so the hard role filter isn't defeated by noise.
        {
            let played = crate::recommendation::rate_blend::role_played_set(
                &or_warn_default(
                    champion_rates_repo::position_sample_totals(&db),
                    "champion_rates_totals",
                ),
                crate::recommendation::rate_blend::ROLE_SHARE_MIN,
            );
            meta_rates.retain(|key, _| played.contains(key));
        }

        // Read scoring weights from persisted settings (fallback to defaults).
        let settings: crate::commands::settings::AppSettings = {
            let json_str: Option<String> = match db.query_row(
                "SELECT value FROM app_config WHERE key = 'settings'",
                [],
                |r| r.get::<_, String>(0),
            ) {
                Ok(s) => Some(s),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(AppError::from(e)),
            };
            json_str
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default()
        };

        let matchup_rows = or_warn_default(
            matchup_repo::fetch_all_for_position(&db, &my_pos),
            "champion_matchups",
        );
        let feedback_rows = read_feedback_rows(&db)?;

        (
            mastery,
            stats,
            all_champions,
            role_map,
            meta_rates,
            settings,
            matchup_rows,
            feedback_rows,
        )
    }; // db lock released here

    // Multiple sources (Match-V5, seed, …) can each carry a row for the same
    // champion-vs-opponent pair. Rather than letting `collect()` keep an arbitrary
    // one, keep the **most-sampled** row per pair — the most reliable single
    // estimate, and (unlike a cross-source mean) it never blends win-rates across
    // different patches (Faz B3).
    let mut matchups: HashMap<(u32, u32), MatchupEntry> = HashMap::new();
    for r in matchup_rows {
        let key = (r.champion_id as u32, r.opponent_id as u32);
        let games = r.games as u32;
        if matchups.get(&key).is_none_or(|e| games > e.games) {
            matchups.insert(
                key,
                MatchupEntry {
                    win_rate: r.win_rate as f32,
                    games,
                },
            );
        }
    }
    let feedback_signals = aggregate_feedback(&feedback_rows);

    let items = state.items_cache.lock().await.clone();
    let rune_trees = state.rune_trees_cache.lock().await.clone();
    if items.is_empty() {
        tracing::warn!("Item cache boş — önce sync_ddragon_champions çağırın");
    }

    // Brawl-mode scoring preset (ARAM=450, Arena/Hexakill=1700): lane matchup +
    // role_fit are meaningless without lane assignment, so the 0.30 weight is
    // redistributed via ScoringWeights::aram(). Non-brawl uses user settings.
    let is_brawl = matches!(session.queue_id, 450 | 1700);
    let weights = if is_brawl {
        ScoringWeights::aram()
    } else {
        ScoringWeights {
            comfort: settings.weight_comfort,
            matchup: settings.weight_matchup,
            team_counter: settings.weight_team_counter,
            synergy: settings.weight_synergy,
            meta: settings.weight_meta,
            role_fit: settings.weight_role_fit,
            risk: 0.05, // not user-configurable
        }
    };

    // champion_id → PowerCurve from KB archetypes (joined with all_champions key→id).
    let kb = &state.draft_iq;
    let power_curves: HashMap<u32, PowerCurve> = all_champions
        .iter()
        .filter_map(|c| {
            kb.get_archetype(&c.key)
                .map(|a| (c.champion_id as u32, a.power_curve.clone()))
        })
        .collect();

    Ok(ScoringInputs {
        mastery,
        stats,
        all_champions,
        role_map,
        meta_rates,
        matchups,
        feedback_signals,
        weights,
        items,
        rune_trees,
        power_curves,
        my_pos,
    })
}

/// Resolve the enemy laner's archetype string (same assigned position as the
/// local player) for matchup-aware build selection. `None` when the opposing
/// laner is not yet visible or has no KB entry.
fn resolve_enemy_archetype(
    session: &ChampSelectState,
    all_champions: &[champion_repo::ChampionRecord],
    kb: &DraftKnowledgeBase,
    my_pos: &str,
) -> Option<String> {
    session
        .their_team
        .iter()
        .find(|s| s.assigned_position.to_lowercase() == my_pos && s.champion_id != 0)
        .and_then(|s| {
            all_champions
                .iter()
                .find(|c| c.champion_id == s.champion_id as i64)
                .map(|c| c.key.clone())
        })
        .and_then(|key| kb.get_archetype(&key).map(|a| a.archetype.clone()))
}

/// Attach build data (core items, runes, summoner spells, shards) to a single
/// recommendation. Order of preference:
///   1. Matchup-specific curated seed build (`build_source = "seed"`).
///   2. Position-default curated seed build (`build_source = "seed"`).
///   3. General archetype heuristic (`build_source = "general"`) — a sensible
///      default so every champion shows a build, labeled "Genel öneri" in the UI
///      with NO winrate claim. `skill_order` is left `None` (not fabricated).
///
/// Leaves `build_source = "none"` only when the archetype is unknown.
fn enrich_build_for_rec(
    db: &rusqlite::Connection,
    rec: &mut Recommendation,
    my_pos: &str,
    enemy_archetype: Option<&str>,
    kb: &DraftKnowledgeBase,
) {
    let cand_arch = kb.get_archetype(&rec.champion_key);
    let build_result = if let Some(arch) = enemy_archetype {
        builds_repo::get_build_for_matchup(db, rec.champion_id as i64, my_pos, arch)
    } else {
        builds_repo::get_build(db, rec.champion_id as i64, my_pos)
    };
    if let Ok(Some(build)) = build_result {
        if let Ok(ids) = serde_json::from_str::<Vec<u32>>(&build.item_ids) {
            rec.core_items = ids.into_iter().take(4).collect();
        }
        if let Ok(rids) = serde_json::from_str::<Vec<u32>>(&build.rune_ids) {
            rec.keystone = rids.first().copied().unwrap_or(0);
            rec.primary_rune_tree = rids.get(1).copied().unwrap_or(0);
        }
        // V011 pro fields — graceful when DB has nulls
        rec.skill_order = build.skill_order.clone();
        if let Some(s) = &build.summoner_spells {
            rec.summoner_spells = serde_json::from_str(s).unwrap_or_default();
        }
        if let Some(s) = &build.secondary_runes {
            rec.secondary_runes = serde_json::from_str(s).unwrap_or_default();
        }
        if let Some(s) = &build.stat_shards {
            rec.stat_shards = serde_json::from_str(s).unwrap_or_default();
        }
        rec.build_source = "seed".to_string();
        rec.build_confidence = "high".to_string();
        rec.build_note = cand_arch.map(|a| narrative::build_rationale(a, enemy_archetype, "seed"));
        return;
    }

    // No curated seed for this (champion, position) → general archetype build.
    if let Some(a) = cand_arch {
        if let Some(b) = heuristic_build(&a.archetype) {
            rec.core_items = b.core_items.into_iter().take(4).collect();
            rec.keystone = b.keystone;
            rec.primary_rune_tree = b.primary_tree;
            rec.secondary_runes = b.secondary_runes;
            rec.stat_shards = b.stat_shards;
            rec.summoner_spells = b.summoner_spells;
            // skill_order intentionally left None — we don't fabricate it.
            rec.build_source = "general".to_string();
            rec.build_confidence = "medium".to_string();
            rec.build_note = Some(narrative::build_rationale(a, enemy_archetype, "general"));
        }
    }
}

/// Resolve the first core item's display name from the item cache so the prose can
/// ground the build. `None` when there is no build or the id is not in the cache.
fn resolve_core_item_name(rec: &Recommendation, items: &[cdragon::ItemData]) -> Option<String> {
    let id = *rec.core_items.first()?;
    items
        .iter()
        .find(|item| item.id == id)
        .map(|item| item.name.clone())
}

#[tauri::command]
pub async fn get_recommendations(
    session_json: serde_json::Value,
    puuid: String,
    state: State<'_, AppState>,
) -> Result<Vec<Recommendation>, AppError> {
    // Latency budget: champ-select event → recommendation < 500ms (perf target).
    let started = std::time::Instant::now();
    let session = parse_session_arg(session_json)?;

    let inputs = load_scoring_inputs(&state, &session, &puuid).await?;
    let kb = state.draft_iq.clone();

    let ctx = ScoringContext {
        session: &session,
        mastery: &inputs.mastery,
        stats: &inputs.stats,
        role_map: &inputs.role_map,
        meta_rates: &inputs.meta_rates,
        weights: inputs.weights,
        matchups: Some(&inputs.matchups),
        power_curves: Some(&inputs.power_curves),
        feedback_signals: Some(&inputs.feedback_signals),
    };

    let mut recs = compute_recommendations(
        &ctx,
        &inputs.all_champions,
        &inputs.items,
        &inputs.rune_trees,
        &kb,
    );

    // Enrich each recommendation with matchup-aware build data and load local
    // DraftBrain packs without blocking champ-select on cloud.
    let (model_pack, data_pack) = {
        let enemy_archetype =
            resolve_enemy_archetype(&session, &inputs.all_champions, &kb, &inputs.my_pos);
        let db = state.db.lock().await;
        // Pro-play presence (pick% + ban% from Leaguepedia), stored under the
        // synthetic "pro" position so it never mixes into the ranked per-role blend.
        let pro_presence: HashMap<u32, f32> = or_warn_default(
            champion_rates_repo::get_all_for_position(&db, "pro"),
            "pro_presence",
        )
        .into_iter()
        .filter(|r| r.source == "leaguepedia")
        .map(|r| (r.champion_id, r.pick_rate + r.ban_rate))
        .collect();
        for rec in &mut recs {
            enrich_build_for_rec(&db, rec, &inputs.my_pos, enemy_archetype.as_deref(), &kb);
            rec.core_item_name = resolve_core_item_name(rec, &inputs.items);
            // Flag honestly which signals fell back to a placeholder (computed here,
            // before upgrade overwrites the per-champ matchup_confidence).
            rec.missing_signals = compute_missing_signals(rec, &inputs.meta_rates, &inputs.my_pos);
            rec.pro_presence = pro_presence.get(&rec.champion_id).copied();
        }
        (active_model_pack(&db)?, active_data_pack(&db)?)
    };
    upgrade_recommendations_with_context(&mut recs, Some(&model_pack), Some(&data_pack));

    let elapsed_ms = started.elapsed().as_millis();
    if elapsed_ms > 500 {
        tracing::warn!(
            "get_recommendations {} öneri / {} ms — <500ms hedefi aşıldı",
            recs.len(),
            elapsed_ms
        );
    } else {
        tracing::info!(
            "get_recommendations {} öneri / {} ms",
            recs.len(),
            elapsed_ms
        );
    }
    Ok(recs)
}

#[tauri::command]
pub async fn get_draft_brain_recommendations(
    session_json: serde_json::Value,
    puuid: String,
    state: State<'_, AppState>,
) -> Result<Vec<Recommendation>, AppError> {
    get_recommendations(session_json, puuid, state).await
}

/// Full coaching analysis for ONE specific champion — the local player's locked
/// or hovered pick. Unlike `get_recommendations` it does NOT exclude already-
/// picked champions, so after you lock a champion the UI can request *its* build,
/// game plan, combos and score breakdown (instead of `recommendations[0]`, which
/// is a different champion once yours is removed from the pool).
///
/// Returns `Ok(None)` when `champion_id` is unknown (e.g. not yet synced).
#[tauri::command]
pub async fn get_champion_analysis(
    session_json: serde_json::Value,
    champion_id: u32,
    puuid: String,
    state: State<'_, AppState>,
) -> Result<Option<Recommendation>, AppError> {
    if champion_id == 0 {
        return Ok(None);
    }
    let session = parse_session_arg(session_json)?;
    let inputs = load_scoring_inputs(&state, &session, &puuid).await?;
    let kb = state.draft_iq.clone();

    let ctx = ScoringContext {
        session: &session,
        mastery: &inputs.mastery,
        stats: &inputs.stats,
        role_map: &inputs.role_map,
        meta_rates: &inputs.meta_rates,
        weights: inputs.weights,
        matchups: Some(&inputs.matchups),
        power_curves: Some(&inputs.power_curves),
        feedback_signals: Some(&inputs.feedback_signals),
    };

    let mut rec = match analyze_champion(
        champion_id,
        &ctx,
        &inputs.all_champions,
        &inputs.items,
        &inputs.rune_trees,
        &kb,
    ) {
        Some(r) => r,
        None => return Ok(None),
    };

    let (model_pack, data_pack) = {
        let enemy_archetype =
            resolve_enemy_archetype(&session, &inputs.all_champions, &kb, &inputs.my_pos);
        let db = state.db.lock().await;
        enrich_build_for_rec(
            &db,
            &mut rec,
            &inputs.my_pos,
            enemy_archetype.as_deref(),
            &kb,
        );
        rec.core_item_name = resolve_core_item_name(&rec, &inputs.items);
        (active_model_pack(&db)?, active_data_pack(&db)?)
    };
    upgrade_recommendation_with_context(&mut rec, Some(&model_pack), Some(&data_pack));

    Ok(Some(rec))
}

/// Team-level macro game plan for the current draft (#6 — "oyun planı").
///
/// Aggregates the locked allies (plus the local player's locked/hovered pick)
/// and the visible enemy picks into a `GamePlan`: team identity, phase timeline,
/// win conditions, objective priorities, enemy threat and a fallback plan.
/// Always succeeds — an empty/partial draft yields a `partial: true` plan.
#[tauri::command]
pub async fn get_game_plan(
    session_json: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<GamePlan, AppError> {
    let session = parse_session_arg(session_json)?;

    let all_champions = {
        let db = state.db.lock().await;
        champion_repo::list_all(&db)?
    };
    let kb = state.draft_iq.clone();

    // Allies: every locked teammate + the local player's pick (locked or hovered).
    let mut ally_ids: Vec<u32> = session
        .my_team
        .iter()
        .filter(|s| s.champion_id > 0)
        .map(|s| s.champion_id)
        .collect();
    if session.local_player.champion_id == 0 && session.local_player.intent_champion_id > 0 {
        ally_ids.push(session.local_player.intent_champion_id);
    }
    let enemy_ids: Vec<u32> = session
        .their_team
        .iter()
        .filter(|s| s.champion_id > 0)
        .map(|s| s.champion_id)
        .collect();

    let ally_arch: Vec<_> = ally_ids
        .iter()
        .filter_map(|&id| {
            all_champions
                .iter()
                .find(|c| c.champion_id == id as i64)
                .and_then(|c| kb.get_archetype(&c.key))
        })
        .collect();
    let enemy_arch: Vec<_> = enemy_ids
        .iter()
        .filter_map(|&id| {
            all_champions
                .iter()
                .find(|c| c.champion_id == id as i64)
                .and_then(|c| kb.get_archetype(&c.key))
        })
        .collect();

    Ok(compute_game_plan(&ally_arch, &enemy_arch))
}

/// Counter-pick suggestions from the player's mastery pool against the visible
/// lane opponent. Empty when the opponent isn't locked yet or nothing in the
/// pool clears the neutral matchup threshold.
#[tauri::command]
pub async fn get_counter_picks(
    session_json: serde_json::Value,
    puuid: String,
    state: State<'_, AppState>,
) -> Result<Vec<CounterPickHint>, AppError> {
    let session = parse_session_arg(session_json)?;
    let inputs = load_scoring_inputs(&state, &session, &puuid).await?;
    let kb = state.draft_iq.clone();

    let ctx = ScoringContext {
        session: &session,
        mastery: &inputs.mastery,
        stats: &inputs.stats,
        role_map: &inputs.role_map,
        meta_rates: &inputs.meta_rates,
        weights: inputs.weights,
        matchups: Some(&inputs.matchups),
        power_curves: Some(&inputs.power_curves),
        feedback_signals: Some(&inputs.feedback_signals),
    };

    // Pool = the player's mastered champions (top-40 by mastery, from load).
    let pool_ids: Vec<u32> = inputs
        .mastery
        .iter()
        .map(|m| m.champion_id as u32)
        .collect();
    Ok(compute_counter_picks(
        &ctx,
        &inputs.all_champions,
        &kb,
        &pool_ids,
    ))
}

/// Both teams' composition summaries (role counts, AP/AD share, utility flags,
/// gaps) for the draft board UI. Includes the local player's hovered pick on the
/// ally side. Always succeeds; a partial draft yields partial counts.
#[tauri::command]
pub async fn get_team_comp(
    session_json: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<TeamCompBoard, AppError> {
    let session = parse_session_arg(session_json)?;

    let (all_champions, role_map) = {
        let db = state.db.lock().await;
        let all = champion_repo::list_all(&db)?;
        let meta = champion_meta_repo::get_all(&db)?;
        let rm: HashMap<u32, Vec<String>> = meta
            .into_iter()
            .map(|(id, row)| (id, champion_meta_repo::parse_roles(&row.roles)))
            .collect();
        (all, rm)
    };
    let kb = state.draft_iq.clone();

    let mut ally_ids: Vec<u32> = session
        .my_team
        .iter()
        .filter(|s| s.champion_id > 0)
        .map(|s| s.champion_id)
        .collect();
    if session.local_player.champion_id == 0 && session.local_player.intent_champion_id > 0 {
        ally_ids.push(session.local_player.intent_champion_id);
    }
    let enemy_ids: Vec<u32> = session
        .their_team
        .iter()
        .filter(|s| s.champion_id > 0)
        .map(|s| s.champion_id)
        .collect();

    let ally_arch: Vec<_> = ally_ids
        .iter()
        .filter_map(|&id| {
            all_champions
                .iter()
                .find(|c| c.champion_id == id as i64)
                .and_then(|c| kb.get_archetype(&c.key))
        })
        .collect();
    let enemy_arch: Vec<_> = enemy_ids
        .iter()
        .filter_map(|&id| {
            all_champions
                .iter()
                .find(|c| c.champion_id == id as i64)
                .and_then(|c| kb.get_archetype(&c.key))
        })
        .collect();

    Ok(TeamCompBoard {
        ally: compute_comp_summary(&ally_ids, &role_map, &ally_arch),
        enemy: compute_comp_summary(&enemy_ids, &role_map, &enemy_arch),
    })
}

/// One champion's KB archetype, for lobby badges. `archetype` is the raw KB key
/// (e.g. "burst_mage"); the frontend maps it to a display label.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ChampionArchetypeInfo {
    pub champion_id: u32,
    pub archetype: String,
}

/// Champion → KB archetype, straight from the embedded Draft IQ knowledge base
/// (no DB, no network). Powers archetype badges in the lobby champion grid.
#[tauri::command]
pub async fn get_champion_archetypes(
    state: State<'_, AppState>,
) -> Result<Vec<ChampionArchetypeInfo>, AppError> {
    let kb = state.draft_iq.clone();
    Ok(kb
        .archetypes
        .values()
        .map(|a| ChampionArchetypeInfo {
            champion_id: a.champion_id,
            archetype: a.archetype.clone(),
        })
        .collect())
}

/// One ally-combo entry for the synergy board.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ComboBoardEntry {
    pub ally_champion_id: u32,
    pub ally_champion_key: String,
    pub name: String,
    /// Turkish combo description (the `tr` field — user-facing, not `ability_ref`).
    pub combo_text: String,
    pub combo_type: String,
    pub strength: f32,
}

/// Known combos between the local player's pick (locked or hovered) and the
/// locked allies, strongest first. Empty when no pick or no ally combo exists.
#[tauri::command]
pub async fn get_combo_board(
    session_json: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<Vec<ComboBoardEntry>, AppError> {
    let session = parse_session_arg(session_json)?;
    let all_champions = {
        let db = state.db.lock().await;
        champion_repo::list_all(&db)?
    };
    let kb = state.draft_iq.clone();

    // Local pick: locked champion, else hovered intent.
    let my_id = if session.local_player.champion_id > 0 {
        session.local_player.champion_id
    } else {
        session.local_player.intent_champion_id
    };
    if my_id == 0 {
        return Ok(vec![]);
    }
    let my_key = match all_champions.iter().find(|c| c.champion_id == my_id as i64) {
        Some(c) => c.key.clone(),
        None => return Ok(vec![]),
    };

    // Locked allies (excluding self).
    let ally: Vec<(u32, String)> = session
        .my_team
        .iter()
        .filter(|s| s.champion_id > 0 && s.champion_id != my_id)
        .filter_map(|s| {
            all_champions
                .iter()
                .find(|c| c.champion_id == s.champion_id as i64)
                .map(|c| (s.champion_id, c.key.clone()))
        })
        .collect();
    if ally.is_empty() {
        return Ok(vec![]);
    }
    let ally_keys: Vec<&str> = ally.iter().map(|(_, k)| k.as_str()).collect();

    let mut entries: Vec<ComboBoardEntry> = kb
        .combos
        .find_for_ally(&my_key, &ally_keys)
        .into_iter()
        .filter_map(|combo| {
            let partner = if combo.a.eq_ignore_ascii_case(&my_key) {
                &combo.b
            } else {
                &combo.a
            };
            let (ally_id, ally_key) = ally.iter().find(|(_, k)| partner.eq_ignore_ascii_case(k))?;
            Some(ComboBoardEntry {
                ally_champion_id: *ally_id,
                ally_champion_key: ally_key.clone(),
                name: combo.name.clone(),
                combo_text: combo.tr.clone(),
                combo_type: combo.combo_type.clone(),
                strength: combo.strength,
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        b.strength
            .partial_cmp(&a.strength)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(entries)
}

/// A combo this champion participates in (for the detail card).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ChampionDetailCombo {
    pub partner_key: String,
    pub name: String,
    pub combo_text: String,
    pub combo_type: String,
    pub strength: f32,
}

/// Full KB profile for one champion — for the lobby detail card.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ChampionDetail {
    pub champion_id: u32,
    pub champion_key: String,
    pub archetype: String,
    pub power_early: f32,
    pub power_mid: f32,
    pub power_late: f32,
    pub win_condition: String,
    pub damage_ad: f32,
    pub damage_ap: f32,
    pub has_hard_cc: bool,
    pub mobility: String,
    pub blind_safety: f32,
    pub execution_difficulty: u8,
    pub utility_tags: Vec<String>,
    pub combos: Vec<ChampionDetailCombo>,
}

/// KB profile for `champion_id` (archetype, power curve, win condition, damage,
/// CC, combos). `None` when the champion has no KB entry. No DB/network.
#[tauri::command]
pub async fn get_champion_detail(
    champion_id: u32,
    state: State<'_, AppState>,
) -> Result<Option<ChampionDetail>, AppError> {
    let kb = state.draft_iq.clone();
    let Some((key, a)) = kb
        .archetypes
        .iter()
        .find(|(_, a)| a.champion_id == champion_id)
    else {
        return Ok(None);
    };

    let mut combos: Vec<ChampionDetailCombo> = kb
        .combos
        .all_pairs()
        .iter()
        .filter_map(|p| {
            let partner = if p.a.eq_ignore_ascii_case(key) {
                Some(p.b.clone())
            } else if p.b.eq_ignore_ascii_case(key) {
                Some(p.a.clone())
            } else {
                None
            }?;
            Some(ChampionDetailCombo {
                partner_key: partner,
                name: p.name.clone(),
                combo_text: p.tr.clone(),
                combo_type: p.combo_type.clone(),
                strength: p.strength,
            })
        })
        .collect();
    combos.sort_by(|x, y| {
        y.strength
            .partial_cmp(&x.strength)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(Some(ChampionDetail {
        champion_id,
        champion_key: key.clone(),
        archetype: a.archetype.clone(),
        power_early: a.power_curve.early,
        power_mid: a.power_curve.mid,
        power_late: a.power_curve.late,
        win_condition: a.win_condition.clone(),
        damage_ad: a.damage_profile.ad,
        damage_ap: a.damage_profile.ap,
        has_hard_cc: a.cc.has_hard_cc,
        mobility: a.mobility.clone(),
        blind_safety: a.blind_safety,
        execution_difficulty: a.execution_difficulty,
        utility_tags: a.utility_tags.clone(),
        combos,
    }))
}

/// Single decisive read on the draft: favorability + dodge consideration + team
/// needs + top action. Synthesizes the game plan, both teams' comp summaries and
/// the local pick's lane matchup. Always succeeds (partial when draft incomplete).
#[tauri::command]
pub async fn get_draft_verdict(
    session_json: serde_json::Value,
    puuid: String,
    state: State<'_, AppState>,
) -> Result<DraftVerdict, AppError> {
    let session = parse_session_arg(session_json)?;
    let inputs = load_scoring_inputs(&state, &session, &puuid).await?;
    let kb = state.draft_iq.clone();

    let mut ally_ids: Vec<u32> = session
        .my_team
        .iter()
        .filter(|s| s.champion_id > 0)
        .map(|s| s.champion_id)
        .collect();
    if session.local_player.champion_id == 0 && session.local_player.intent_champion_id > 0 {
        ally_ids.push(session.local_player.intent_champion_id);
    }
    let enemy_ids: Vec<u32> = session
        .their_team
        .iter()
        .filter(|s| s.champion_id > 0)
        .map(|s| s.champion_id)
        .collect();

    let ally_arch: Vec<_> = ally_ids
        .iter()
        .filter_map(|&id| {
            inputs
                .all_champions
                .iter()
                .find(|c| c.champion_id == id as i64)
                .and_then(|c| kb.get_archetype(&c.key))
        })
        .collect();
    let enemy_arch: Vec<_> = enemy_ids
        .iter()
        .filter_map(|&id| {
            inputs
                .all_champions
                .iter()
                .find(|c| c.champion_id == id as i64)
                .and_then(|c| kb.get_archetype(&c.key))
        })
        .collect();

    let plan = compute_game_plan(&ally_arch, &enemy_arch);
    let ally_comp = compute_comp_summary(&ally_ids, &inputs.role_map, &ally_arch);
    let enemy_comp = compute_comp_summary(&enemy_ids, &inputs.role_map, &enemy_arch);

    // Lane matchup for the local pick (locked or hovered) — only when the lane
    // opponent is visible. matchup_score detects the opponent from the session.
    let my_pick = if session.local_player.champion_id > 0 {
        session.local_player.champion_id
    } else {
        session.local_player.intent_champion_id
    };
    let opp_visible = session
        .their_team
        .iter()
        .any(|s| s.assigned_position.to_lowercase() == inputs.my_pos && s.champion_id > 0);
    let lane_matchup = if my_pick > 0 && opp_visible {
        let ctx = ScoringContext {
            session: &session,
            mastery: &inputs.mastery,
            stats: &inputs.stats,
            role_map: &inputs.role_map,
            meta_rates: &inputs.meta_rates,
            weights: inputs.weights,
            matchups: Some(&inputs.matchups),
            power_curves: Some(&inputs.power_curves),
            feedback_signals: Some(&inputs.feedback_signals),
        };
        Some(matchup_score(my_pick, &ctx))
    } else {
        None
    };

    Ok(compute_draft_verdict(
        &plan,
        &ally_comp,
        &enemy_comp,
        lane_matchup,
    ))
}

/// Defensive counter-itemization advice vs the enemy composition (MR/armor/
/// health by tag, plus anti-heal / tenacity text when the enemy has sustain or
/// heavy hard CC). Empty when no notable threat.
#[tauri::command]
pub async fn get_counter_items(
    session_json: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<Vec<CounterItemHint>, AppError> {
    let session = parse_session_arg(session_json)?;

    let (all_champions, role_map) = {
        let db = state.db.lock().await;
        let all = champion_repo::list_all(&db)?;
        let meta = champion_meta_repo::get_all(&db)?;
        let rm: HashMap<u32, Vec<String>> = meta
            .into_iter()
            .map(|(id, row)| (id, champion_meta_repo::parse_roles(&row.roles)))
            .collect();
        (all, rm)
    };
    let kb = state.draft_iq.clone();
    let items = state.items_cache.lock().await.clone();

    let enemy_ids: Vec<u32> = session
        .their_team
        .iter()
        .filter(|s| s.champion_id > 0)
        .map(|s| s.champion_id)
        .collect();
    let enemy_comp = TeamComposition::from_champion_ids(&enemy_ids, &role_map);

    let enemy_arch: Vec<_> = enemy_ids
        .iter()
        .filter_map(|&id| {
            all_champions
                .iter()
                .find(|c| c.champion_id == id as i64)
                .and_then(|c| kb.get_archetype(&c.key))
        })
        .collect();
    let enemy_sustain = enemy_arch
        .iter()
        .any(|a| a.utility_tags.iter().any(|t| t == "sustain"));
    let enemy_hard_cc: u32 = enemy_arch.iter().map(|a| a.cc.hard_cc_count as u32).sum();

    // The local pick's class — squishy carries (ADC/mage/assassin) get carry-survival
    // advice (Maw/GA/Sterak), not misleading pure-tank counter items.
    let my_champ = if session.local_player.champion_id > 0 {
        session.local_player.champion_id
    } else {
        session.local_player.intent_champion_id
    };
    let candidate_squishy_carry = all_champions
        .iter()
        .find(|c| c.champion_id == my_champ as i64)
        .and_then(|c| kb.get_archetype(&c.key))
        .is_some_and(|a| {
            matches!(
                a.archetype.as_str(),
                "marksman" | "artillery" | "burst_mage" | "control_mage" | "assassin"
            )
        });

    Ok(counter_item_advice(
        &enemy_comp,
        enemy_sustain,
        enemy_hard_cc,
        &items,
        candidate_squishy_carry,
    ))
}

/// Rich lane-matchup read for the local pick vs the visible lane opponent:
/// per-phase advantage + concrete lane tips. `None` when there's no lane (ARAM),
/// no local pick, no opponent, or missing KB data.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct LaneMatchup {
    pub opponent_key: String,
    pub opponent_name: String,
    pub phase_advantage: [f32; 3],
    pub tips: Vec<String>,
    /// True when the opponent was inferred (Blind/Normal — no LCU positions) by
    /// archetype fit rather than read directly. UI labels it "Tahmini rakip".
    pub inferred: bool,
}

#[tauri::command]
pub async fn get_lane_matchup(
    session_json: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<Option<LaneMatchup>, AppError> {
    let session = parse_session_arg(session_json)?;
    let my_pos = session.local_player.assigned_position.to_lowercase();
    if my_pos.is_empty() {
        return Ok(None); // ARAM / no lane
    }
    let my_id = if session.local_player.champion_id > 0 {
        session.local_player.champion_id
    } else {
        session.local_player.intent_champion_id
    };
    if my_id == 0 {
        return Ok(None);
    }

    // Load champions + CDragon role map (role map drives the Blind/Normal opponent
    // inference when LCU omits enemy positions).
    let (all_champions, role_map) = {
        let db = state.db.lock().await;
        let all = champion_repo::list_all(&db)?;
        let meta = champion_meta_repo::get_all(&db)?;
        let rm: HashMap<u32, Vec<String>> = meta
            .into_iter()
            .map(|(id, row)| (id, champion_meta_repo::parse_roles(&row.roles)))
            .collect();
        (all, rm)
    };
    let kb = state.draft_iq.clone();

    let (opp_id, inferred) =
        match resolve_lane_opponent_raw(&session.their_team, &role_map, &my_pos) {
            Some(v) => v,
            None => return Ok(None),
        };

    let my_key = match all_champions.iter().find(|c| c.champion_id == my_id as i64) {
        Some(c) => c.key.clone(),
        None => return Ok(None),
    };
    let opp_rec = match all_champions
        .iter()
        .find(|c| c.champion_id == opp_id as i64)
    {
        Some(c) => c,
        None => return Ok(None),
    };
    let (cand, opp_arch) = match (kb.get_archetype(&my_key), kb.get_archetype(&opp_rec.key)) {
        (Some(c), Some(o)) => (c, o),
        _ => return Ok(None),
    };

    let adv = |a: f32, b: f32| -> f32 {
        let s = a + b;
        if s < 0.01 {
            0.5
        } else {
            (a / s).clamp(0.0, 1.0)
        }
    };
    let phase_advantage = [
        adv(cand.power_curve.early, opp_arch.power_curve.early),
        adv(cand.power_curve.mid, opp_arch.power_curve.mid),
        adv(cand.power_curve.late, opp_arch.power_curve.late),
    ];

    Ok(Some(LaneMatchup {
        opponent_key: opp_rec.key.clone(),
        opponent_name: opp_rec.name.clone(),
        phase_advantage,
        tips: build_matchup_tips(cand, opp_arch, &my_pos),
        inferred,
    }))
}

/// Champions to learn for `role` — KB-driven (role fit + low execution + blind
/// safety + synergy with the player's mastered pool), excluding owned ones.
/// `role` is an LCU position ("top"/"jungle"/"middle"/"bottom"/"utility").
#[tauri::command]
pub async fn get_pool_suggestions(
    role: String,
    puuid: String,
    state: State<'_, AppState>,
) -> Result<Vec<PoolSuggestion>, AppError> {
    let (all_champions, mastery, mut meta_rates, position_totals) = {
        let db = state.db.lock().await;
        (
            champion_repo::list_all(&db)?,
            mastery_repo::top_for_puuid(&db, &puuid, 60)?,
            // Blended meta for the role drives the "learn meta-strong picks" ranking.
            blend_position_rates(or_warn_default(
                champion_rates_repo::get_all_for_position(&db, &role),
                "champion_rates",
            )),
            or_warn_default(
                champion_rates_repo::position_sample_totals(&db),
                "champion_rates_totals",
            ),
        )
    };
    // Role-share filter: drop off-role noise (u.gg lists every champion in every lane)
    // so e.g. a 1%-of-his-games "bottom Olaf" row can't sneak into the ADC pool.
    let played = rate_blend::role_played_set(&position_totals, rate_blend::ROLE_SHARE_MIN);
    meta_rates.retain(|key, _| played.contains(key));
    let kb = state.draft_iq.clone();

    let owned: Vec<(u32, String)> = mastery
        .iter()
        .filter_map(|m| {
            all_champions
                .iter()
                .find(|c| c.champion_id == m.champion_id)
                .map(|c| (m.champion_id as u32, c.key.clone()))
        })
        .collect();

    let candidates: Vec<(u32, String, &ChampionArchetype)> = kb
        .archetypes
        .iter()
        .map(|(key, a)| (a.champion_id, key.clone(), a))
        .collect();

    Ok(suggest_pool(
        &role,
        &owned,
        &candidates,
        &kb.combos,
        Some(&meta_rates),
    ))
}

#[tauri::command]
pub async fn sync_cdragon_meta(state: State<'_, AppState>) -> Result<usize, AppError> {
    // Fetch without holding DB lock
    let champions = cdragon::fetch_champion_meta().await?;
    let count = champions.len();

    let db = state.db.lock().await;
    for champ in &champions {
        let roles_json = serde_json::to_string(&champ.roles).unwrap_or_else(|_| "[]".to_string());
        let is_melee = cdragon::is_melee_from_roles(&champ.roles);

        let row = champion_meta_repo::ChampionMetaRow {
            champion_id: champ.id as u32,
            roles: roles_json,
            is_melee,
            attack_range: if is_melee { 175 } else { 550 },
            resource_type: None,
        };
        champion_meta_repo::upsert_meta(&db, &row)?;
    }

    tracing::info!("CDragon meta senkronize edildi: {} şampiyon", count);
    Ok(count)
}

#[derive(Debug, Serialize)]
pub struct ChampionPersonalStats {
    pub games: u32,
    pub wins: u32,
    pub losses: u32,
    pub win_rate: f32,
    pub mastery_level: i32,
    pub mastery_points: i32,
    pub last_played_days_ago: Option<u32>,
}

#[tauri::command]
pub async fn get_champion_personal_stats(
    puuid: String,
    champion_id: u32,
    state: State<'_, AppState>,
) -> Result<ChampionPersonalStats, AppError> {
    let db = state.db.lock().await;

    let stats = match_repo::player_stats(&db, &puuid)?;
    let champ_stat = stats.iter().find(|s| s.champion_id as u32 == champion_id);

    let masteries = mastery_repo::top_for_puuid(&db, &puuid, 100)?;
    let mastery = masteries
        .iter()
        .find(|m| m.champion_id as u32 == champion_id);

    let last_played: Option<i64> = db.query_row(
        "SELECT MAX(played_at) FROM matches WHERE puuid = ?1 AND champion_id = ?2",
        rusqlite::params![puuid, champion_id as i64],
        |r| r.get::<_, Option<i64>>(0),
    )?;

    let last_played_days_ago = last_played.map(|ts| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        ((now - ts) / 86400) as u32
    });

    let (games, wins) = champ_stat.map(|s| (s.games, s.wins)).unwrap_or((0, 0));
    let losses = games.saturating_sub(wins);
    let win_rate = if games > 0 {
        wins as f32 / games as f32
    } else {
        0.0
    };

    Ok(ChampionPersonalStats {
        games,
        wins,
        losses,
        win_rate,
        mastery_level: mastery.map(|m| m.level as i32).unwrap_or(0),
        mastery_points: mastery.map(|m| m.points as i32).unwrap_or(0),
        last_played_days_ago,
    })
}

/// Return up to 3 ban suggestions for the active ban phase.
///
/// Inputs are derived from the same session JSON as `get_recommendations`.
/// ARAM (queue_id 450) reuses the synthetic "aram" key for meta_rates lookup.
/// The player's top-5 mastery pool is used to bias the threat score toward
/// champions that counter the player's likely picks.
#[tauri::command]
pub async fn get_ban_suggestions(
    session_json: serde_json::Value,
    puuid: String,
    state: State<'_, AppState>,
) -> Result<Vec<BanSuggestion>, AppError> {
    // The frontend round-trips the already-parsed ChampSelectState (snake_case)
    // back to us via the `champ-select-session` event. Raw LCU sessions (tests,
    // direct passthrough) carry an "actions" array + camelCase fields — only those
    // go through `parse_session`; everything else deserializes as ChampSelectState.
    let session: ChampSelectState = if session_json.get("actions").is_some() {
        parse_session(&session_json)
            .ok_or_else(|| AppError::Other("Geçersiz session JSON".to_string()))?
    } else {
        serde_json::from_value(session_json)?
    };

    let lane_key = if session.queue_id == 450 {
        "aram".to_string()
    } else {
        session.local_player.assigned_position.to_lowercase()
    };

    let (all_champions, meta_rates, my_pool, role_map) = {
        let db = state.db.lock().await;

        let all_champions = champion_repo::list_all(&db)?;

        let meta_rates: HashMap<(u32, String), MetaRate> = blend_position_rates(or_warn_default(
            champion_rates_repo::get_all_for_position(&db, &lane_key),
            "champion_rates",
        ));

        // Top-5 mastery pool drives the "counters our pool" signal.
        let my_pool = mastery_repo::top_for_puuid(&db, &puuid, 5)?;

        let meta_map = champion_meta_repo::get_all(&db)?;
        let role_map: HashMap<u32, Vec<String>> = meta_map
            .into_iter()
            .map(|(id, row)| (id, champion_meta_repo::parse_roles(&row.roles)))
            .collect();

        (all_champions, meta_rates, my_pool, role_map)
    }; // db lock released here

    Ok(compute_ban_suggestions(
        &session,
        &all_champions,
        &meta_rates,
        &my_pool,
        &role_map,
        &state.draft_iq,
    ))
}

/// Champion pool summary for one enemy slot.
/// `play_rate` is the fraction (0..1) of the last `game_count` games on `top_champion_id`.
/// If `game_count` is 0 or no match history was available, `top_champion_id` will be 0.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct EnemyPoolSummary {
    pub cell_id: i32,
    /// Most-played champion in recent history. 0 = unavailable.
    pub top_champion_id: u32,
    pub top_champion_key: String,
    /// Fraction 0..1 of recent games on top_champion_id.
    pub play_rate: f32,
    /// Number of games analysed.
    pub game_count: u32,
}

/// Fetch the recent champion pool for each enemy in champ-select.
///
/// Reads `theirTeam[i].summonerId` from the raw session JSON, queries the LCU
/// for each enemy's match history (last 20 games), and returns one
/// `EnemyPoolSummary` per enemy whose summonerId is non-zero.
///
/// Partial failures are silently skipped — the command always succeeds even
/// if LCU is unreachable for some slots.
#[tauri::command]
pub async fn get_enemy_champion_pools(
    session_json: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<Vec<EnemyPoolSummary>, AppError> {
    let client_guard = state.lcu_client.lock().await;
    let client = match client_guard.as_ref() {
        Some(c) => c.clone(),
        None => return Ok(vec![]),
    };
    drop(client_guard);

    // Collect champion_id→key map from DB for name resolution.
    let champ_key_map: HashMap<u32, String> = {
        let db = state.db.lock().await;
        champion_repo::list_all(&db)
            .unwrap_or_default()
            .into_iter()
            .map(|c| (c.champion_id as u32, c.key))
            .collect()
    };

    // Parse enemy slots from raw JSON (summoner_id is not in ChampSelectState).
    let enemy_slots: Vec<(i32, i64)> = session_json["theirTeam"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|s| {
            let cell_id = s["cellId"].as_i64()? as i32;
            let summoner_id = s["summonerId"].as_i64().unwrap_or(0);
            if summoner_id != 0 {
                Some((cell_id, summoner_id))
            } else {
                None
            }
        })
        .collect();

    // Fetch match history for all enemies concurrently (20 games each).
    const HISTORY_COUNT: u32 = 20;
    let tasks: Vec<_> = enemy_slots
        .iter()
        .map(|(cell_id, summoner_id)| {
            let client = client.clone();
            let cell_id = *cell_id;
            let summoner_id = *summoner_id;
            async move {
                let puuid = match champ_pool::fetch_summoner_puuid(&client, summoner_id).await {
                    Ok(p) => p,
                    Err(_) => return None,
                };
                let history =
                    match champ_pool::fetch_match_history(&client, &puuid, HISTORY_COUNT).await {
                        Ok(h) => h,
                        Err(_) => return None,
                    };
                let pool = champ_pool::compute_champion_pool(&history, &puuid);
                let total_games: u32 = pool.iter().map(|(_, c)| c).sum();
                if total_games == 0 {
                    return None;
                }
                let (top_id, top_count) = pool[0];
                Some((cell_id, top_id, top_count, total_games))
            }
        })
        .collect();

    let results = join_all(tasks).await;

    let summaries: Vec<EnemyPoolSummary> = results
        .into_iter()
        .flatten()
        .map(|(cell_id, top_id, top_count, total)| EnemyPoolSummary {
            cell_id,
            top_champion_id: top_id,
            top_champion_key: champ_key_map.get(&top_id).cloned().unwrap_or_default(),
            play_rate: top_count as f32 / total as f32,
            game_count: total,
        })
        .collect();

    Ok(summaries)
}
