//! In-game overlay command (Faz 4 runtime). Thin orchestration: fetch the official
//! Live Client Data, run the pure macro engine, return the overlay state. No game logic
//! here. "No live game" is a normal `Ok` (not an error) so frontend polling stays silent.

use crate::db::champion_repo;
use crate::errors::AppError;
use crate::recommendation::draft_iq::archetype::ChampionArchetype;
use crate::recommendation::draft_iq::narrative;
use crate::recommendation::macro_timers::{compute_macro_state, MacroState};
use crate::riot::live_client::{
    enemy_laner_name, parse_active_player, parse_macro_input, ActivePlayerInfo, LiveClientApi,
};
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;

/// Overlay payload. `live=false` ⇒ no game running (port 2999 unreachable); `state` is
/// then `null`. Lets the frontend poll on a timer with no error spam.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct OverlayMacroState {
    pub live: bool,
    pub state: Option<MacroState>,
}

impl OverlayMacroState {
    fn offline() -> Self {
        Self {
            live: false,
            state: None,
        }
    }
}

/// Your in-game game plan — regenerated from the KB archetype of the champion you are
/// actually playing (read from the official Live Client Data API) plus your live,
/// on-screen score. Lets you alt-tab mid-match and re-read your plan. Policy-safe: own
/// champion + public score only, no hidden info.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct IngamePlan {
    pub champion_name: String,
    /// KB/DDragon key for the champion icon (e.g. "MissFortune").
    pub champion_key: String,
    /// Normalized lane ("top"/"jungle"/"middle"/"bottom"/"utility" or "").
    pub position: String,
    pub win_condition: String,
    pub team_role: String,
    pub damage_profile: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spike_note: Option<String>,
    /// Generic lane-phase micro — set only when the lane opponent is unknown (otherwise
    /// the richer `matchup_tips` carry the lane advice, so we avoid duplicating it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane_note: Option<String>,
    /// The enemy laner (opposite team, same role), when resolvable from the scoreboard.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opponent_name: Option<String>,
    /// KB/DDragon key for the enemy laner's icon, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opponent_key: Option<String>,
    /// Up to 4 matchup-specific lane tips vs the enemy laner. Empty when blind/unknown.
    /// Always serialized (even when empty) so the TS side can read `.length` safely.
    #[serde(default)]
    pub matchup_tips: Vec<String>,
    /// Mid-game macro advice (archetype-grounded) — the detailed early/mid/late plan.
    pub mid_plan: String,
    /// Late-game advice keyed by the champion's win condition.
    pub late_plan: String,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub cs: u32,
    pub level: u32,
    /// CS per minute, when game time is known (≥ 60s in).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cs_per_min: Option<f32>,
}

/// Pure: assemble the in-game plan from the parsed player snapshot + KB archetype. When
/// the enemy laner is known, the richer matchup tips carry the lane advice (and the
/// generic `lane_note` is dropped to avoid duplication).
fn build_ingame_plan(
    info: &ActivePlayerInfo,
    champion_key: &str,
    arch: &ChampionArchetype,
    position: &str,
    game_time_secs: u32,
    opponent: Option<(&ChampionArchetype, &str, &str)>, // (archetype, name, key)
) -> IngamePlan {
    let cs_per_min = if game_time_secs >= 60 {
        let per = info.cs as f32 / (game_time_secs as f32 / 60.0);
        Some((per * 10.0).round() / 10.0)
    } else {
        None
    };
    let (lane_note, matchup_tips) = match opponent {
        Some((opp, _, _)) => (None, narrative::build_matchup_tips(arch, opp, position)),
        None => (
            narrative::build_lane_phase_advice(arch, None, position),
            Vec::new(),
        ),
    };
    IngamePlan {
        champion_name: info.champion_name.clone(),
        champion_key: champion_key.to_string(),
        position: position.to_string(),
        win_condition: narrative::build_win_condition_text(&arch.win_condition),
        team_role: narrative::build_team_role_text(arch),
        damage_profile: narrative::build_damage_profile_label(&arch.damage_profile),
        spike_note: narrative::build_spike_note(arch),
        lane_note,
        opponent_name: opponent.map(|(_, name, _)| name.to_string()),
        opponent_key: opponent.map(|(_, _, key)| key.to_string()),
        matchup_tips,
        mid_plan: narrative::build_mid_game_advice(arch),
        late_plan: narrative::build_late_game_advice(&arch.win_condition),
        kills: info.kills,
        deaths: info.deaths,
        assists: info.assists,
        cs: info.cs,
        level: info.level,
        cs_per_min,
    }
}

/// Read-only: your current game plan (champion win-condition / team role / power spike +
/// lane note) regenerated from the KB, plus your live score. Returns `None` when no game
/// is running or the played champion isn't in the KB. Fetched once by the overlay (not
/// polled) — the plan doesn't change mid-game.
#[tauri::command]
pub async fn get_ingame_plan(state: State<'_, AppState>) -> Result<Option<IngamePlan>, AppError> {
    let Ok(api) = LiveClientApi::new() else {
        return Ok(None);
    };
    let raw = match api.fetch_all_game_data().await {
        Ok(raw) => raw,
        Err(_) => return Ok(None), // no live game — quiet
    };
    let Some(info) = parse_active_player(&raw) else {
        return Ok(None);
    };
    let game_time_secs = parse_macro_input(&raw).game_time_secs;

    let champions = {
        let db = state.db.lock().await;
        champion_repo::list_all(&db)?
    };
    // Prefer the locale-independent key from rawChampionName (robust against a
    // localized championName), fall back to a case-insensitive display-name match.
    let Some(key) = info
        .champion_key
        .as_deref()
        .and_then(|k| champions.iter().find(|c| c.key.eq_ignore_ascii_case(k)))
        .or_else(|| {
            champions
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(&info.champion_name))
        })
        .map(|c| c.key.clone())
    else {
        return Ok(None);
    };

    let kb = state.draft_iq.clone();
    let Some(arch) = kb.get_archetype(&key) else {
        return Ok(None);
    };
    let position = info.position.to_lowercase();

    // Resolve the enemy laner (public scoreboard info) → key → archetype for a
    // matchup-aware lane read. None for ARAM / unknown role → generic lane note.
    let opp = enemy_laner_name(&raw, &info.team, &info.position).and_then(|name| {
        champions
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(&name))
            .map(|c| (name, c.key.clone()))
    });
    let opponent = opp
        .as_ref()
        .and_then(|(name, k)| kb.get_archetype(k).map(|a| (a, name.as_str(), k.as_str())));

    Ok(Some(build_ingame_plan(
        &info,
        &key,
        arch,
        &position,
        game_time_secs,
        opponent,
    )))
}

/// Read-only: current in-game macro state (objective timers + reminders + phase) from
/// the official Live Client Data API. Returns `{live:false, state:null}` when no game is
/// running — never an `Err` — so the overlay can poll quietly.
#[tauri::command]
pub async fn get_macro_state() -> Result<OverlayMacroState, AppError> {
    let Ok(api) = LiveClientApi::new() else {
        return Ok(OverlayMacroState::offline());
    };
    match api.fetch_all_game_data().await {
        Ok(raw) => {
            let state = compute_macro_state(&parse_macro_input(&raw));
            Ok(OverlayMacroState {
                live: true,
                state: Some(state),
            })
        }
        // No live game (connection refused / timeout) — quiet, expected.
        Err(_) => Ok(OverlayMacroState::offline()),
    }
}

#[cfg(test)]
mod tests {
    use crate::recommendation::macro_timers::{GAME_PHASES, OBJECTIVES, OBJECTIVE_STATES};

    /// Drift guard: every objective/state/phase token the overlay shows must have an
    /// `overlay.*` i18n label. en parity is covered by `i18n-parity.test.ts`.
    #[test]
    fn every_overlay_token_has_an_i18n_label() {
        const TR: &str = include_str!("../../../src/i18n/tr.json");
        let tr: serde_json::Value = serde_json::from_str(TR).unwrap();
        let ov = &tr["overlay"];
        for o in OBJECTIVES {
            assert!(!ov["objective"][o].is_null(), "objective '{o}' i18n yok");
        }
        for s in OBJECTIVE_STATES {
            assert!(!ov["state"][s].is_null(), "state '{s}' i18n yok");
        }
        for p in GAME_PHASES {
            assert!(!ov["phase"][p].is_null(), "phase '{p}' i18n yok");
        }
    }

    #[test]
    fn ingame_plan_assembles_plan_and_cs_pace_from_archetype_and_score() {
        use crate::recommendation::draft_iq::archetype::{
            CcProfile, ChampionArchetype, DamageProfile, PowerCurve,
        };
        use crate::riot::live_client::ActivePlayerInfo;

        let arch = ChampionArchetype {
            champion_id: 1,
            archetype: "marksman".into(),
            damage_profile: DamageProfile {
                ad: 0.9,
                ap: 0.0,
                true_damage: 0.0,
            },
            cc: CcProfile {
                has_hard_cc: false,
                hard_cc_count: 0,
                primary_cc: vec![],
            },
            mobility: "low".into(),
            engage_role: "none".into(),
            peel_capability: "low".into(),
            blind_safety: 0.6,
            execution_difficulty: 2,
            win_condition: "protect".into(),
            ult_type: "x".into(),
            confidence: "high".into(),
            power_curve: PowerCurve {
                early: 0.4,
                mid: 0.6,
                late: 0.85,
            },
            counters_archetypes: vec![],
            utility_tags: vec![],
        };
        let info = ActivePlayerInfo {
            champion_name: "Jinx".into(),
            champion_key: None,
            position: "BOTTOM".into(),
            team: "ORDER".into(),
            kills: 3,
            deaths: 1,
            assists: 5,
            cs: 150,
            level: 11,
        };

        // No opponent known → generic lane note, no matchup tips.
        let plan = super::build_ingame_plan(&info, "Jinx", &arch, "bottom", 600, None); // 10:00 in
        assert_eq!(plan.champion_name, "Jinx");
        assert_eq!(plan.champion_key, "Jinx");
        assert_eq!(plan.position, "bottom");
        assert!(!plan.win_condition.is_empty());
        assert_eq!(plan.cs, 150);
        assert_eq!(plan.cs_per_min, Some(15.0)); // 150 CS / 10 min
        assert!(plan.lane_note.is_some(), "bottom marksman → lane note");
        assert!(plan.matchup_tips.is_empty());
        assert!(plan.opponent_name.is_none());
        assert!(plan.opponent_key.is_none());

        // Before 1 minute we don't fabricate a CS/min pace.
        let early = super::build_ingame_plan(&info, "Jinx", &arch, "bottom", 30, None);
        assert_eq!(early.cs_per_min, None);

        // With a known enemy laner → matchup tips carry the lane advice, generic
        // lane_note is dropped to avoid duplication.
        let opp = ChampionArchetype {
            archetype: "assassin".into(),
            win_condition: "pick".into(),
            ..arch.clone()
        };
        let vs = super::build_ingame_plan(
            &info,
            "Jinx",
            &arch,
            "bottom",
            600,
            Some((&opp, "Zed", "Zed")),
        );
        assert_eq!(vs.opponent_name.as_deref(), Some("Zed"));
        assert_eq!(vs.opponent_key.as_deref(), Some("Zed"));
        assert!(!vs.matchup_tips.is_empty(), "known opponent → matchup tips");
        assert!(
            vs.lane_note.is_none(),
            "lane advice folded into matchup tips"
        );
    }
}
