use super::champion_types::{type_counter_score, ChampionType};
use super::draft_iq::archetype::PowerCurve;
use super::team_analysis::TeamComposition;
use crate::db::mastery_repo::MasteryRow;
use crate::lcu::session::ChampSelectState;
use crate::riot::client::ChampionStats;
use std::collections::HashMap;

/// User-configurable scoring weights.
/// Positive contributions: comfort + matchup + team_counter + synergy + meta + role_fit
/// Negative contribution:  risk (penalty subtracted after normalisation)
#[derive(Debug, Clone, Copy)]
pub struct ScoringWeights {
    pub comfort: f32,
    /// Lane matchup vs opposite-lane opponent (formerly `lane_counter`).
    pub matchup: f32,
    pub team_counter: f32,
    pub synergy: f32,
    pub meta: f32,
    /// How well the champion's roles fit the assigned position.
    pub role_fit: f32,
    /// Risk penalty (high ban-rate / low sample-size).
    /// Multiplied with `risk_score` and SUBTRACTED from total.
    pub risk: f32,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            comfort: 0.20,
            matchup: 0.25,
            team_counter: 0.15,
            synergy: 0.10,
            meta: 0.15,
            role_fit: 0.10,
            risk: 0.05,
        }
    }
}

impl ScoringWeights {
    /// ARAM / Arena scoring preset. Lane matchup and role fit are zero-weighted
    /// (no lane assignment in brawl modes); the freed 0.30 weight is redistributed
    /// to comfort (own pool size matters when the random pick excludes mastered
    /// champs), team_counter (constant 5v5 teamfights), and synergy.
    pub fn aram() -> Self {
        Self {
            comfort: 0.35,
            matchup: 0.00,
            team_counter: 0.30,
            synergy: 0.20,
            meta: 0.15,
            role_fit: 0.00,
            risk: 0.05,
        }
    }
}

/// ARAM/Arena utility bonus from KB `utility_tags` + `archetype`.
/// Returns a `±0.20` adjustment applied to the candidate's total score in brawl
/// modes. Poke / waveclear / disengage / sustain are uplifted (no recall, single
/// lane, constant trading); melee assassins and short-range divers are penalised.
pub fn aram_utility_bonus(
    archetype: &crate::recommendation::draft_iq::archetype::ChampionArchetype,
) -> f32 {
    let mut bonus = 0.0_f32;
    let tags = &archetype.utility_tags;
    let has = |t: &str| tags.iter().any(|x| x == t);

    if has("poke") {
        bonus += 0.08;
    }
    if has("waveclear") {
        bonus += 0.05;
    }
    if has("disengage") {
        bonus += 0.05;
    }
    if has("sustain") {
        bonus += 0.05;
    }
    if has("siege") {
        bonus += 0.03;
    }

    bonus += match archetype.archetype.as_str() {
        "artillery" => 0.10,
        "enchanter" | "battle_mage" => 0.06,
        "control_mage" => 0.04,
        "marksman" => 0.02,
        // Short-range carries struggle in ARAM (no flank options, constant poke)
        "assassin" => -0.08,
        "skirmisher" => -0.05,
        _ => 0.0,
    };

    bonus.clamp(-0.20, 0.20)
}

/// Per-matchup win-rate entry populated from the `champion_matchups` table (V006).
#[derive(Debug, Clone)]
pub struct MatchupEntry {
    pub win_rate: f32,
    pub games: u32,
}

/// Patch-level rate snapshot for a champion at a specific lane.
/// Sourced from `champion_rates` (V008) via `champion_rates_repo`.
#[derive(Debug, Clone)]
pub struct MetaRate {
    pub win_rate: f32,
    pub ban_rate: f32,
    pub sample_size: u32,
}

/// All inputs needed to score a champion candidate.
pub struct ScoringContext<'a> {
    pub session: &'a ChampSelectState,
    pub mastery: &'a [MasteryRow],
    pub stats: &'a [ChampionStats],
    pub role_map: &'a HashMap<u32, Vec<String>>,
    pub weights: ScoringWeights,
    /// (champion_id, lcu_position) → patch-level rate snapshot.
    /// Empty map = no meta data → neutral fallback in `meta_score`/`risk_score`.
    pub meta_rates: &'a HashMap<(u32, String), MetaRate>,
    /// (champion_id, opponent_id) → per-matchup win stats from V006.
    /// `None` or missing key → pure archetype heuristic fallback.
    pub matchups: Option<&'a HashMap<(u32, u32), MatchupEntry>>,
    /// champion_id → KB power curve {early, mid, late}. Used to enhance the
    /// matchup heuristic fallback with phase-advantage data from champions.json.
    pub power_curves: Option<&'a HashMap<u32, PowerCurve>>,
}

/// Maps LCU assignedPosition → expected CDragon champion roles.
/// LCU positions are always lowercase ("top","jungle","middle","bottom","utility").
/// CDragon roles: "fighter","tank","mage","assassin","marksman","support".
fn position_expected_roles(position: &str) -> &'static [&'static str] {
    match position {
        "top" => &["fighter", "tank"],
        "jungle" => &["fighter", "assassin", "tank"],
        "middle" => &["mage", "assassin"],
        "bottom" => &["marksman"],
        "utility" => &["support"],
        _ => &[],
    }
}

/// 0.0–1.0: how comfortable the player is on this champion.
/// Based on mastery points + personal win-rate from match history.
///
/// Mastery points are capped at 2 000 000 so OTPs (2M+ mastery) don't hit the
/// ceiling at 200k like lower-ranked players.
///
/// Win-rate uses Bayesian shrinkage (`prior_wr=0.50, prior_n=5`) so that 1–2
/// games contribute a small signal instead of being silently dropped.
pub fn comfort_score(champion_id: u32, ctx: &ScoringContext) -> f32 {
    let mastery_score = ctx
        .mastery
        .iter()
        .find(|m| m.champion_id == champion_id as i64)
        .map(|m| {
            let points_part = (m.points as f32 / 2_000_000.0).min(0.5);
            let level_part = (m.level as f32 / 14.0).min(0.5);
            points_part + level_part
        })
        .unwrap_or(0.0);

    // Bayesian shrinkage: prior WR = 0.50, prior sample size = 5 games.
    // Returns 0.0 when there are no recorded games (no signal, not neutral).
    const PRIOR_WR: f32 = 0.50;
    const PRIOR_N: f32 = 5.0;
    let wr_score = ctx
        .stats
        .iter()
        .find(|s| s.champion_id == champion_id as i64)
        .filter(|s| s.games > 0)
        .map(|s| (s.wins as f32 + PRIOR_WR * PRIOR_N) / (s.games as f32 + PRIOR_N))
        .unwrap_or(0.0);

    (0.6 * mastery_score + 0.4 * wr_score).min(1.0)
}

/// How well this champion counters the lane opponent.
/// Renamed from `lane_counter_score` → `matchup_score` (Sprint E).
///
/// When a `MatchupEntry` is present in `ctx.matchups`, blends the real win-rate
/// with the archetype heuristic proportionally to sample confidence
/// (`real_weight = (games / 100).min(1.0)`). Falls back to pure heuristic when
/// no data is available.
pub fn matchup_score(champion_id: u32, ctx: &ScoringContext) -> f32 {
    let my_pos = ctx.session.local_player.assigned_position.to_lowercase();

    let opponent_id = ctx
        .session
        .their_team
        .iter()
        .find(|s| s.assigned_position.to_lowercase() == my_pos && s.champion_id > 0)
        .map(|s| s.champion_id)
        .unwrap_or(0);

    if opponent_id == 0 {
        return 0.3;
    }

    // Archetype heuristic (always computed as fallback baseline)
    let heuristic = {
        let my_types = ctx
            .role_map
            .get(&champion_id)
            .map(|r| ChampionType::from_roles(r))
            .unwrap_or_default();
        let opp_types = ctx
            .role_map
            .get(&opponent_id)
            .map(|r| ChampionType::from_roles(r))
            .unwrap_or_default();
        if my_types.is_empty() || opp_types.is_empty() {
            0.3_f32
        } else {
            let total: f32 = my_types
                .iter()
                .flat_map(|a| opp_types.iter().map(|d| type_counter_score(a, d)))
                .sum();
            (total / (my_types.len() * opp_types.len()) as f32).min(1.0)
        }
    };

    // Blend with real per-champion data when available
    if let Some(map) = ctx.matchups {
        if let Some(entry) = map.get(&(champion_id, opponent_id)) {
            // Confidence weight: small sample → lean on heuristic
            let real_weight = (entry.games as f32 / 100.0).min(1.0);
            // win_rate 0.40 → 0.0, 0.50 → 0.5, 0.60 → 1.0
            let real_score = ((entry.win_rate - 0.40) / 0.20).clamp(0.0, 1.0);
            return real_weight * real_score + (1.0 - real_weight) * heuristic;
        }
    }

    // No seeded data: blend archetype heuristic with power-curve phase advantage.
    // phase_advantage returns [0.5, 0.5, 0.5] when curves are missing — skip blend.
    let phases = phase_advantage(champion_id, opponent_id, ctx);
    let all_neutral = phases.iter().all(|&v| (v - 0.5).abs() < 0.001);
    if !all_neutral {
        // Early game matters most in lane: 40% / 35% / 25%
        let phase_score = 0.40 * phases[0] + 0.35 * phases[1] + 0.25 * phases[2];
        return 0.60 * heuristic + 0.40 * phase_score;
    }

    heuristic
}

/// Per-phase (early / mid / late) advantage of `champion_id` over `opponent_id`.
/// Returns `[early, mid, late]` each in `[0.0, 1.0]`, where `0.5` is neutral.
/// Returns `[0.5, 0.5, 0.5]` when `ctx.power_curves` is absent or either
/// champion's curve is missing.
pub fn phase_advantage(champion_id: u32, opponent_id: u32, ctx: &ScoringContext) -> [f32; 3] {
    let curves = match ctx.power_curves {
        Some(c) => c,
        None => return [0.5; 3],
    };
    let my = match curves.get(&champion_id) {
        Some(c) => c,
        None => return [0.5; 3],
    };
    let opp = match curves.get(&opponent_id) {
        Some(c) => c,
        None => return [0.5; 3],
    };
    let advantage = |a: f32, b: f32| -> f32 {
        let s = a + b;
        if s < 0.01 {
            0.5
        } else {
            (a / s).clamp(0.0, 1.0)
        }
    };
    [
        advantage(my.early, opp.early),
        advantage(my.mid, opp.mid),
        advantage(my.late, opp.late),
    ]
}

/// How well this champion counters the full enemy team composition.
pub fn team_counter_score(champion_id: u32, ctx: &ScoringContext) -> f32 {
    let enemy_ids: Vec<u32> = ctx
        .session
        .their_team
        .iter()
        .filter(|s| s.champion_id > 0)
        .map(|s| s.champion_id)
        .collect();

    if enemy_ids.is_empty() {
        return 0.3;
    }

    let enemy_comp = TeamComposition::from_champion_ids(&enemy_ids, ctx.role_map);
    let my_types = ctx
        .role_map
        .get(&champion_id)
        .map(|r| ChampionType::from_roles(r))
        .unwrap_or_default();

    enemy_comp.counter_score_for(&my_types)
}

/// How well this champion synergises with the existing allied picks.
pub fn synergy_score(champion_id: u32, ctx: &ScoringContext) -> f32 {
    let ally_ids: Vec<u32> = ctx
        .session
        .my_team
        .iter()
        .filter(|s| s.champion_id > 0 && s.champion_id != champion_id)
        .map(|s| s.champion_id)
        .collect();

    let ally_comp = TeamComposition::from_champion_ids(&ally_ids, ctx.role_map);
    let my_types = ctx
        .role_map
        .get(&champion_id)
        .map(|r| ChampionType::from_roles(r))
        .unwrap_or_default();

    ally_comp.synergy_score_for(&my_types)
}

/// Patch-level meta strength based on aggregated win-rate from `champion_rates`.
/// Returns 0.3 (neutral) when no data is present for (champion, position).
///
/// Linear mapping: win_rate 0.48 → 0.0, 0.55 → 1.0 (clamped).
pub fn meta_score(champion_id: u32, ctx: &ScoringContext) -> f32 {
    let pos = ctx.session.local_player.assigned_position.to_lowercase();
    ctx.meta_rates
        .get(&(champion_id, pos))
        .map(|r| ((r.win_rate - 0.48) / 0.07).clamp(0.0, 1.0))
        .unwrap_or(0.3)
}

/// Risk penalty in [0.0, 0.10]. Subtracted from the total score (via `weights.risk`).
///
/// Drivers:
/// - ban_rate > 0.20  → +0.05
/// - sample_size < 200 → +0.05
///
/// Falls back to 0.1 when no meta-rate row exists for the lane (unknown is risky).
pub fn risk_score(champion_id: u32, ctx: &ScoringContext) -> f32 {
    let pos = ctx.session.local_player.assigned_position.to_lowercase();
    let Some(r) = ctx.meta_rates.get(&(champion_id, pos)) else {
        return 0.1;
    };
    let ban_penalty: f32 = if r.ban_rate > 0.20 { 0.05 } else { 0.0 };
    let sample_penalty: f32 = if r.sample_size < 200 { 0.05 } else { 0.0 };
    (ban_penalty + sample_penalty).min(0.10_f32)
}

/// How well the champion's CDragon roles fit the player's assigned LCU position.
///
/// Formula: `0.2 + (overlap / expected_count) * 0.8`, clamped to 1.0.
///
/// Special cases:
/// - 0.5 = unknown / non-standard position
/// - 0.3 = champion has no role data
pub fn role_fit_score(champion_id: u32, ctx: &ScoringContext) -> f32 {
    let pos = ctx.session.local_player.assigned_position.to_lowercase();
    let expected = position_expected_roles(&pos);
    if expected.is_empty() {
        return 0.5;
    }
    let empty: Vec<String> = Vec::new();
    let champ_roles = ctx.role_map.get(&champion_id).unwrap_or(&empty);
    if champ_roles.is_empty() {
        return 0.3;
    }
    let overlap = champ_roles
        .iter()
        .filter(|r| {
            let r_lc = r.to_lowercase();
            expected.contains(&r_lc.as_str())
        })
        .count();
    (0.2 + (overlap as f32 / expected.len() as f32) * 0.8).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lcu::session::TeamSlot;

    fn empty_session(pos: &str) -> ChampSelectState {
        ChampSelectState {
            my_cell_id: 0,
            local_player: TeamSlot {
                cell_id: 0,
                champion_id: 0,
                intent_champion_id: 0,
                assigned_position: pos.to_string(),
                is_locked: false,
            },
            my_team: vec![],
            their_team: vec![],
            my_bans: vec![],
            their_bans: vec![],
            phase: "BAN_PICK".into(),
            time_left_ms: 30_000,
            action_type: "pick".into(),
            queue_id: 0,
            pick_order: 0,
        }
    }

    fn make_ctx<'a>(
        session: &'a ChampSelectState,
        role_map: &'a HashMap<u32, Vec<String>>,
        meta_rates: &'a HashMap<(u32, String), MetaRate>,
    ) -> ScoringContext<'a> {
        ScoringContext {
            session,
            mastery: &[],
            stats: &[],
            role_map,
            weights: ScoringWeights::default(),
            meta_rates,
            matchups: None,
            power_curves: None,
        }
    }

    #[test]
    fn role_fit_mid_mage_full_overlap() {
        let session = empty_session("middle");
        let mut role_map = HashMap::new();
        role_map.insert(99, vec!["mage".into(), "assassin".into()]);
        let meta = HashMap::new();
        let ctx = make_ctx(&session, &role_map, &meta);
        // expected ["mage","assassin"] len=2, overlap=2 → 0.2 + 2/2*0.8 = 1.0
        assert!((role_fit_score(99, &ctx) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn role_fit_mid_partial_overlap() {
        let session = empty_session("middle");
        let mut role_map = HashMap::new();
        role_map.insert(99, vec!["mage".into()]);
        let meta = HashMap::new();
        let ctx = make_ctx(&session, &role_map, &meta);
        // expected len=2, overlap=1 → 0.2 + 0.5*0.8 = 0.6
        assert!((role_fit_score(99, &ctx) - 0.6).abs() < 1e-6);
    }

    #[test]
    fn role_fit_no_overlap() {
        let session = empty_session("middle");
        let mut role_map = HashMap::new();
        role_map.insert(22, vec!["marksman".into()]);
        let meta = HashMap::new();
        let ctx = make_ctx(&session, &role_map, &meta);
        // overlap=0 → 0.2
        assert!((role_fit_score(22, &ctx) - 0.2).abs() < 1e-6);
    }

    #[test]
    fn role_fit_unknown_position_neutral() {
        let session = empty_session("invalid_pos");
        let mut role_map = HashMap::new();
        role_map.insert(1, vec!["mage".into()]);
        let meta = HashMap::new();
        let ctx = make_ctx(&session, &role_map, &meta);
        assert!((role_fit_score(1, &ctx) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn role_fit_no_role_data() {
        let session = empty_session("top");
        let role_map = HashMap::new();
        let meta = HashMap::new();
        let ctx = make_ctx(&session, &role_map, &meta);
        assert!((role_fit_score(1, &ctx) - 0.3).abs() < 1e-6);
    }

    // ─── comfort_score tests ──────────────────────────────────────────────────

    fn make_ctx_with_mastery_and_stats<'a>(
        session: &'a ChampSelectState,
        mastery: &'a [crate::db::mastery_repo::MasteryRow],
        stats: &'a [crate::riot::client::ChampionStats],
        role_map: &'a HashMap<u32, Vec<String>>,
        meta_rates: &'a HashMap<(u32, String), MetaRate>,
    ) -> ScoringContext<'a> {
        ScoringContext {
            session,
            mastery,
            stats,
            role_map,
            weights: ScoringWeights::default(),
            meta_rates,
            matchups: None,
            power_curves: None,
        }
    }

    #[test]
    fn comfort_mastery_cap_otp_2m_reaches_full_mastery_score() {
        use crate::db::mastery_repo::MasteryRow;
        let session = empty_session("top");
        let mastery = vec![MasteryRow {
            champion_id: 157,
            level: 7,
            points: 2_000_000,
            last_play_time: None,
        }];
        let role_map = HashMap::new();
        let meta = HashMap::new();
        let ctx = make_ctx_with_mastery_and_stats(&session, &mastery, &[], &role_map, &meta);
        // points_part = 2M/2M = 0.5, level_part = 7/14 = 0.5 → mastery_score = 1.0
        // wr_score = 0.0 (no games) → comfort = 0.6 * 1.0 = 0.6
        let score = comfort_score(157, &ctx);
        assert!(
            (score - 0.6).abs() < 1e-5,
            "2M mastery L7, no games → 0.6, got {score}"
        );
    }

    #[test]
    fn comfort_mastery_200k_lower_than_2m() {
        use crate::db::mastery_repo::MasteryRow;
        let session = empty_session("top");
        let mastery_low = vec![MasteryRow {
            champion_id: 157,
            level: 7,
            points: 200_000,
            last_play_time: None,
        }];
        let mastery_high = vec![MasteryRow {
            champion_id: 157,
            level: 7,
            points: 2_000_000,
            last_play_time: None,
        }];
        let role_map = HashMap::new();
        let meta = HashMap::new();
        let ctx_low =
            make_ctx_with_mastery_and_stats(&session, &mastery_low, &[], &role_map, &meta);
        let ctx_high =
            make_ctx_with_mastery_and_stats(&session, &mastery_high, &[], &role_map, &meta);
        assert!(
            comfort_score(157, &ctx_high) > comfort_score(157, &ctx_low),
            "OTP (2M) must score higher than 200k player at same level"
        );
    }

    #[test]
    fn comfort_bayesian_wr_1_game_contributes() {
        use crate::riot::client::ChampionStats;
        let session = empty_session("top");
        let stats_1w = vec![ChampionStats {
            champion_id: 157,
            games: 1,
            wins: 1,
            kda_avg: 3.0,
        }];
        let stats_1l = vec![ChampionStats {
            champion_id: 157,
            games: 1,
            wins: 0,
            kda_avg: 2.0,
        }];
        let role_map = HashMap::new();
        let meta = HashMap::new();
        let ctx_1w = make_ctx_with_mastery_and_stats(&session, &[], &stats_1w, &role_map, &meta);
        let ctx_1l = make_ctx_with_mastery_and_stats(&session, &[], &stats_1l, &role_map, &meta);
        // 1 win, 1 game: bayesian WR = (1+2.5)/(1+5) = 3.5/6 ≈ 0.583 → wr_score ≈ 0.583
        let score_1w = comfort_score(157, &ctx_1w);
        let score_1l = comfort_score(157, &ctx_1l);
        assert!(
            score_1w > 0.0,
            "1 win game must contribute positive wr_score, got {score_1w}"
        );
        assert!(
            score_1w > score_1l,
            "1 win > 1 loss in Bayesian WR (got {score_1w} vs {score_1l})"
        );
    }

    #[test]
    fn comfort_bayesian_wr_zero_games_returns_zero() {
        let session = empty_session("top");
        let role_map = HashMap::new();
        let meta = HashMap::new();
        let ctx = make_ctx_with_mastery_and_stats(&session, &[], &[], &role_map, &meta);
        // No games → wr_score = 0.0, no mastery → mastery_score = 0.0 → total = 0.0
        assert!(
            comfort_score(157, &ctx).abs() < 1e-6,
            "Zero games + no mastery must give comfort = 0.0 (stretch pick gate intact)"
        );
    }

    #[test]
    fn meta_score_no_data_returns_neutral() {
        let session = empty_session("top");
        let role_map = HashMap::new();
        let meta = HashMap::new();
        let ctx = make_ctx(&session, &role_map, &meta);
        assert!((meta_score(157, &ctx) - 0.3).abs() < 1e-6);
    }

    #[test]
    fn meta_score_high_winrate_maps_to_one() {
        let session = empty_session("top");
        let role_map = HashMap::new();
        let mut meta = HashMap::new();
        meta.insert(
            (157, "top".into()),
            MetaRate {
                win_rate: 0.55,
                ban_rate: 0.05,
                sample_size: 5000,
            },
        );
        let ctx = make_ctx(&session, &role_map, &meta);
        assert!((meta_score(157, &ctx) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn meta_score_low_winrate_maps_to_zero() {
        let session = empty_session("top");
        let role_map = HashMap::new();
        let mut meta = HashMap::new();
        meta.insert(
            (157, "top".into()),
            MetaRate {
                win_rate: 0.48,
                ban_rate: 0.05,
                sample_size: 5000,
            },
        );
        let ctx = make_ctx(&session, &role_map, &meta);
        assert!(meta_score(157, &ctx).abs() < 1e-6);
    }

    #[test]
    fn risk_score_no_data_returns_default_penalty() {
        let session = empty_session("top");
        let role_map = HashMap::new();
        let meta = HashMap::new();
        let ctx = make_ctx(&session, &role_map, &meta);
        // No meta_rates row → fallback 0.1
        assert!((risk_score(1, &ctx) - 0.1).abs() < 1e-6);
    }

    #[test]
    fn risk_score_high_ban_only() {
        let session = empty_session("top");
        let role_map = HashMap::new();
        let mut meta = HashMap::new();
        meta.insert(
            (157, "top".into()),
            MetaRate {
                win_rate: 0.51,
                ban_rate: 0.25,
                sample_size: 5000,
            },
        );
        let ctx = make_ctx(&session, &role_map, &meta);
        assert!((risk_score(157, &ctx) - 0.05).abs() < 1e-6);
    }

    #[test]
    fn risk_score_low_sample_only() {
        let session = empty_session("top");
        let role_map = HashMap::new();
        let mut meta = HashMap::new();
        meta.insert(
            (157, "top".into()),
            MetaRate {
                win_rate: 0.51,
                ban_rate: 0.05,
                sample_size: 100,
            },
        );
        let ctx = make_ctx(&session, &role_map, &meta);
        assert!((risk_score(157, &ctx) - 0.05).abs() < 1e-6);
    }

    #[test]
    fn risk_score_both_penalties_capped() {
        let session = empty_session("top");
        let role_map = HashMap::new();
        let mut meta = HashMap::new();
        meta.insert(
            (157, "top".into()),
            MetaRate {
                win_rate: 0.51,
                ban_rate: 0.30,
                sample_size: 50,
            },
        );
        let ctx = make_ctx(&session, &role_map, &meta);
        assert!((risk_score(157, &ctx) - 0.10).abs() < 1e-6);
    }

    // ─── matchup blend tests ──────────────────────────────────────────────────

    fn session_with_opponent(my_pos: &str, opponent_id: u32) -> ChampSelectState {
        use crate::lcu::session::TeamSlot;
        ChampSelectState {
            my_cell_id: 0,
            local_player: TeamSlot {
                cell_id: 0,
                champion_id: 0,
                intent_champion_id: 0,
                assigned_position: my_pos.to_string(),
                is_locked: false,
            },
            my_team: vec![],
            their_team: vec![TeamSlot {
                cell_id: 5,
                champion_id: opponent_id,
                intent_champion_id: 0,
                assigned_position: my_pos.to_string(),
                is_locked: false,
            }],
            my_bans: vec![],
            their_bans: vec![],
            phase: "BAN_PICK".into(),
            time_left_ms: 30_000,
            action_type: "pick".into(),
            queue_id: 0,
            pick_order: 0,
        }
    }

    #[test]
    fn matchup_blend_full_confidence_uses_real_score() {
        // games=200 → real_weight=1.0 → score = (0.60 - 0.40) / 0.20 = 1.0
        let session = session_with_opponent("top", 122);
        let role_map = HashMap::new();
        let meta = HashMap::new();
        let mut matchups = HashMap::new();
        matchups.insert(
            (86, 122),
            MatchupEntry {
                win_rate: 0.60,
                games: 200,
            },
        );
        let ctx = ScoringContext {
            session: &session,
            mastery: &[],
            stats: &[],
            role_map: &role_map,
            weights: ScoringWeights::default(),
            meta_rates: &meta,
            matchups: Some(&matchups),
            power_curves: None,
        };
        assert!((matchup_score(86, &ctx) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn matchup_blend_no_data_falls_back_to_heuristic() {
        // No matchup entry → returns heuristic (0.3 when role_map empty)
        let session = session_with_opponent("top", 122);
        let role_map = HashMap::new();
        let meta = HashMap::new();
        let matchups: HashMap<(u32, u32), MatchupEntry> = HashMap::new();
        let ctx = ScoringContext {
            session: &session,
            mastery: &[],
            stats: &[],
            role_map: &role_map,
            weights: ScoringWeights::default(),
            meta_rates: &meta,
            matchups: Some(&matchups),
            power_curves: None,
        };
        // role_map empty → heuristic = 0.3
        assert!((matchup_score(86, &ctx) - 0.3).abs() < 1e-5);
    }

    #[test]
    fn matchup_blend_partial_confidence_blends() {
        // games=50 → real_weight=0.5; win_rate=0.50 → real_score=0.5
        // heuristic=0.3 (empty role_map) → blended = 0.5*0.5 + 0.5*0.3 = 0.40
        let session = session_with_opponent("top", 122);
        let role_map = HashMap::new();
        let meta = HashMap::new();
        let mut matchups = HashMap::new();
        matchups.insert(
            (86, 122),
            MatchupEntry {
                win_rate: 0.50,
                games: 50,
            },
        );
        let ctx = ScoringContext {
            session: &session,
            mastery: &[],
            stats: &[],
            role_map: &role_map,
            weights: ScoringWeights::default(),
            meta_rates: &meta,
            matchups: Some(&matchups),
            power_curves: None,
        };
        assert!((matchup_score(86, &ctx) - 0.40).abs() < 1e-5);
    }

    // ── phase_advantage tests ─────────────────────────────────────────────────

    #[test]
    fn phase_advantage_neutral_when_no_curves() {
        let session = session_with_opponent("top", 122);
        let role_map = HashMap::new();
        let meta = HashMap::new();
        let ctx = ScoringContext {
            session: &session,
            mastery: &[],
            stats: &[],
            role_map: &role_map,
            weights: ScoringWeights::default(),
            meta_rates: &meta,
            matchups: None,
            power_curves: None,
        };
        let phases = phase_advantage(86, 122, &ctx);
        assert_eq!(phases, [0.5, 0.5, 0.5]);
    }

    #[test]
    fn phase_advantage_early_dominant_when_high_early_curve() {
        use crate::recommendation::draft_iq::archetype::PowerCurve;
        let session = session_with_opponent("top", 122);
        let role_map = HashMap::new();
        let meta = HashMap::new();
        let mut curves = HashMap::new();
        // Champion 86 is an early bully (early 0.8), 122 is a late scaler (early 0.2)
        curves.insert(
            86_u32,
            PowerCurve {
                early: 0.8,
                mid: 0.6,
                late: 0.3,
            },
        );
        curves.insert(
            122_u32,
            PowerCurve {
                early: 0.2,
                mid: 0.5,
                late: 0.8,
            },
        );
        let ctx = ScoringContext {
            session: &session,
            mastery: &[],
            stats: &[],
            role_map: &role_map,
            weights: ScoringWeights::default(),
            meta_rates: &meta,
            matchups: None,
            power_curves: Some(&curves),
        };
        let phases = phase_advantage(86, 122, &ctx);
        // early: 0.8/(0.8+0.2) = 0.80
        assert!(
            (phases[0] - 0.80).abs() < 1e-4,
            "expected early=0.80, got {}",
            phases[0]
        );
        // mid: 0.6/(0.6+0.5) ≈ 0.545
        assert!((phases[1] - 0.6 / 1.1).abs() < 1e-4);
        // late: 0.3/(0.3+0.8) ≈ 0.273 (disadvantage)
        assert!(phases[2] < 0.4, "expected late disadvantage");
    }

    #[test]
    fn phase_advantage_blends_into_matchup_heuristic() {
        use crate::recommendation::draft_iq::archetype::PowerCurve;
        let session = session_with_opponent("top", 122);
        let role_map = HashMap::new();
        let meta = HashMap::new();
        let mut curves = HashMap::new();
        curves.insert(
            86_u32,
            PowerCurve {
                early: 0.8,
                mid: 0.6,
                late: 0.3,
            },
        );
        curves.insert(
            122_u32,
            PowerCurve {
                early: 0.2,
                mid: 0.5,
                late: 0.8,
            },
        );
        let ctx = ScoringContext {
            session: &session,
            mastery: &[],
            stats: &[],
            role_map: &role_map,
            weights: ScoringWeights::default(),
            meta_rates: &meta,
            matchups: None,
            power_curves: Some(&curves),
        };
        let score = matchup_score(86, &ctx);
        // Heuristic=0.3 (empty role_map), phase_score≈0.40*0.80+0.35*0.545+0.25*0.273≈0.578
        // blended = 0.60*0.3 + 0.40*0.578 ≈ 0.411
        assert!(
            score > 0.35,
            "phase blend should push score above pure heuristic 0.30, got {score}"
        );
        assert!(
            score < 0.60,
            "blend should not reach extreme values, got {score}"
        );
    }

    // ── C1: ARAM preset + utility bonus tests ─────────────────────────────────

    #[test]
    fn aram_preset_zeroes_lane_weights() {
        let w = ScoringWeights::aram();
        assert_eq!(w.matchup, 0.0, "ARAM has no lane matchup");
        assert_eq!(w.role_fit, 0.0, "ARAM has no lane role fit");
        // Freed weight must redistribute to brawl-relevant components
        let pos_sum = w.comfort + w.team_counter + w.synergy + w.meta;
        assert!(
            pos_sum >= 0.99 && pos_sum <= 1.01,
            "non-zero weights should sum to ~1.0, got {pos_sum}"
        );
    }

    #[test]
    fn aram_bonus_boosts_artillery_with_poke() {
        use crate::recommendation::draft_iq::archetype::{
            CcProfile, ChampionArchetype, DamageProfile, PowerCurve,
        };
        let arty = ChampionArchetype {
            champion_id: 99,
            archetype: "artillery".to_string(),
            damage_profile: DamageProfile {
                ad: 0.1,
                ap: 0.85,
                true_damage: 0.0,
            },
            cc: CcProfile {
                has_hard_cc: false,
                hard_cc_count: 0,
                primary_cc: vec![],
            },
            mobility: "low".to_string(),
            engage_role: "none".to_string(),
            peel_capability: "low".to_string(),
            blind_safety: 0.5,
            execution_difficulty: 3,
            win_condition: "poke".to_string(),
            ult_type: "skillshot".to_string(),
            confidence: "high".to_string(),
            power_curve: PowerCurve {
                early: 0.5,
                mid: 0.75,
                late: 0.7,
            },
            counters_archetypes: vec![],
            utility_tags: vec!["poke".to_string(), "waveclear".to_string()],
        };
        let bonus = aram_utility_bonus(&arty);
        // artillery (+0.10) + poke (+0.08) + waveclear (+0.05) = 0.23, clamped to 0.20
        assert!(
            bonus >= 0.18 && bonus <= 0.20,
            "expected high ARAM bonus, got {bonus}"
        );
    }

    #[test]
    fn aram_bonus_penalises_assassin() {
        use crate::recommendation::draft_iq::archetype::{
            CcProfile, ChampionArchetype, DamageProfile, PowerCurve,
        };
        let assassin = ChampionArchetype {
            champion_id: 121,
            archetype: "assassin".to_string(),
            damage_profile: DamageProfile {
                ad: 0.8,
                ap: 0.0,
                true_damage: 0.1,
            },
            cc: CcProfile {
                has_hard_cc: false,
                hard_cc_count: 0,
                primary_cc: vec![],
            },
            mobility: "high".to_string(),
            engage_role: "diver".to_string(),
            peel_capability: "low".to_string(),
            blind_safety: 0.4,
            execution_difficulty: 4,
            win_condition: "pick".to_string(),
            ult_type: "single".to_string(),
            confidence: "high".to_string(),
            power_curve: PowerCurve {
                early: 0.5,
                mid: 0.85,
                late: 0.65,
            },
            counters_archetypes: vec![],
            utility_tags: vec![],
        };
        let bonus = aram_utility_bonus(&assassin);
        // assassin (-0.08) + no positive tags = negative
        assert!(
            bonus < 0.0,
            "assassin should receive negative ARAM bonus, got {bonus}"
        );
    }
}
