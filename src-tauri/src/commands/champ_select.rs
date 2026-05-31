use crate::db::{
    builds_repo, champion_meta_repo, champion_rates_repo, champion_repo, mastery_repo, match_repo,
    matchup_repo,
};
use crate::ddragon::cdragon;
use crate::errors::AppError;
use crate::lcu::champ_pool;
use crate::lcu::{parse_session, ChampSelectState};
use crate::recommendation::ban_advisor::compute_ban_suggestions;
use crate::recommendation::draft_iq::archetype::PowerCurve;
use crate::recommendation::scoring::MatchupEntry;
use crate::recommendation::scoring::MetaRate;
use crate::recommendation::{
    compute_recommendations, BanSuggestion, Recommendation, ScoringContext, ScoringWeights,
};
use crate::AppState;
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, State};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

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

#[tauri::command]
pub async fn get_recommendations(
    session_json: serde_json::Value,
    puuid: String,
    state: State<'_, AppState>,
) -> Result<Vec<Recommendation>, AppError> {
    // Latency budget: champ-select event → recommendation < 500ms (perf target).
    // Measured at the command layer so the pure engine stays I/O-free.
    let started = std::time::Instant::now();
    // Parse the session first — we need `assigned_position` before the DB lock
    // so the lane-scoped meta_rates query knows which position to filter on.
    let session: ChampSelectState = if session_json.is_object() {
        parse_session(&session_json)
            .ok_or_else(|| AppError::Other("Geçersiz session JSON".to_string()))?
    } else {
        serde_json::from_value(session_json)?
    };
    // ARAM (queue_id 450) is laneless — we look up rates under the synthetic
    // "aram" key. Summoner's Rift uses the assigned LCU position lowercase.
    let my_pos = if session.queue_id == 450 {
        "aram".to_string()
    } else {
        session.local_player.assigned_position.to_lowercase()
    };

    // Read all DB data, then release the lock before any further work
    let (mastery, stats, all_champions, role_map, meta_rates, settings, matchup_rows) = {
        let db = state.db.lock().await;

        let mastery = mastery_repo::top_for_puuid(&db, &puuid, 40)?;
        let stats = match_repo::player_stats(&db, &puuid)?;
        let all_champions = champion_repo::list_all(&db)?;

        let meta_map = champion_meta_repo::get_all(&db)?;
        let role_map: HashMap<u32, Vec<String>> = meta_map
            .into_iter()
            .map(|(id, row)| (id, champion_meta_repo::parse_roles(&row.roles)))
            .collect();

        // Patch-level rates from champion_rates (V008), scoped to the local
        // player's lane. Empty result is a safe fallback (no meta signal).
        let meta_rates: HashMap<(u32, String), MetaRate> =
            champion_rates_repo::get_all_for_position(&db, &my_pos)
                .unwrap_or_default()
                .into_iter()
                .map(|r| {
                    let key = (r.champion_id, r.position.clone());
                    let rate = MetaRate {
                        win_rate: r.win_rate,
                        ban_rate: r.ban_rate,
                        sample_size: r.sample_size,
                    };
                    (key, rate)
                })
                .collect();

        // Read scoring weights from persisted settings (fallback to defaults if absent)
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

        let matchup_rows = matchup_repo::fetch_all_for_position(&db, &my_pos).unwrap_or_default();

        (
            mastery,
            stats,
            all_champions,
            role_map,
            meta_rates,
            settings,
            matchup_rows,
        )
    }; // db lock released here

    let matchups: HashMap<(u32, u32), MatchupEntry> = matchup_rows
        .into_iter()
        .map(|r| {
            (
                (r.champion_id as u32, r.opponent_id as u32),
                MatchupEntry {
                    win_rate: r.win_rate as f32,
                    games: r.games as u32,
                },
            )
        })
        .collect();

    let items = state.items_cache.lock().await.clone();
    let rune_trees = state.rune_trees_cache.lock().await.clone();
    if items.is_empty() {
        tracing::warn!("Item cache boş — önce sync_ddragon_champions çağırın");
    }

    // Brawl-mode scoring preset (ARAM=450, Arena/Hexakill=1700):
    // lane matchup + role_fit are meaningless without lane assignment, so the
    // 0.30 weight is redistributed via ScoringWeights::aram(). Non-brawl queues
    // use the user-configured weights from settings.
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

    let kb = state.draft_iq.clone();

    // Build champion_id → PowerCurve map from KB archetypes (joined with all_champions key→id).
    let power_curves: HashMap<u32, PowerCurve> = all_champions
        .iter()
        .filter_map(|c| {
            kb.get_archetype(&c.key)
                .map(|a| (c.champion_id as u32, a.power_curve.clone()))
        })
        .collect();

    let ctx = ScoringContext {
        session: &session,
        mastery: &mastery,
        stats: &stats,
        role_map: &role_map,
        meta_rates: &meta_rates,
        weights,
        matchups: Some(&matchups),
        power_curves: Some(&power_curves),
    };

    let mut recs = compute_recommendations(&ctx, &all_champions, &items, &rune_trees, &kb);

    // Enrich each recommendation with matchup-aware build data.
    // Resolve the enemy laner's archetype first; fall back to default build if unknown.
    {
        let enemy_archetype: Option<String> = session
            .their_team
            .iter()
            .find(|s| s.assigned_position.to_lowercase() == my_pos && s.champion_id != 0)
            .and_then(|s| {
                all_champions
                    .iter()
                    .find(|c| c.champion_id == s.champion_id as i64)
                    .map(|c| c.key.clone())
            })
            .and_then(|key| kb.get_archetype(&key).map(|a| a.archetype.clone()));

        let db = state.db.lock().await;
        for rec in &mut recs {
            let build_result = if let Some(ref arch) = enemy_archetype {
                builds_repo::get_build_for_matchup(&db, rec.champion_id as i64, &my_pos, arch)
            } else {
                builds_repo::get_build(&db, rec.champion_id as i64, &my_pos)
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
            }
        }
    }

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
    let session: ChampSelectState = if session_json.is_object() {
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

        let meta_rates: HashMap<(u32, String), MetaRate> =
            champion_rates_repo::get_all_for_position(&db, &lane_key)
                .unwrap_or_default()
                .into_iter()
                .map(|r| {
                    let key = (r.champion_id, r.position.clone());
                    let rate = MetaRate {
                        win_rate: r.win_rate,
                        ban_rate: r.ban_rate,
                        sample_size: r.sample_size,
                    };
                    (key, rate)
                })
                .collect();

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
