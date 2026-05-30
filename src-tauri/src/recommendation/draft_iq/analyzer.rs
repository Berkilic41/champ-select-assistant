use super::archetype::ChampionArchetype;
use super::combos::ComboDirectory;
use super::narrative;
use crate::recommendation::models::{ComboHint, ComboType, DraftPlan};

/// Caller-assembled inputs for one candidate analysis. All references borrow
/// from an outer scope so no allocations happen inside `analyze_pick`.
pub struct AnalyzerInput<'a> {
    pub candidate_key: &'a str,
    pub candidate: &'a ChampionArchetype,
    /// `(champion_id, champion_key)` for each ally with a known archetype.
    pub ally_id_keys: Vec<(u32, String)>,
    pub ally_archetypes: Vec<&'a ChampionArchetype>,
    pub enemy_archetypes: Vec<&'a ChampionArchetype>,
    pub combo_dir: &'a ComboDirectory,
    /// True when the local player is picking before seeing the full enemy lineup.
    pub is_first_pick: bool,
    /// True when pick_order ≥ 4 — enough enemy picks are visible for counter-picking.
    pub is_late_pick: bool,
    /// 1-indexed global pick position (0 = unknown, from `ChampSelectState.pick_order`).
    pub pick_order: u8,
    /// LCU position string ("top"/"jungle"/"middle"/"bottom"/"utility"/""). Used
    /// for position-specific lane-phase advice. Empty string = ARAM / no lane.
    pub position: &'a str,
    /// Enemy laner's KB archetype (same `assigned_position` as candidate).
    /// `None` when opponent is not visible or has no KB entry.
    pub lane_opponent: Option<&'a ChampionArchetype>,
}

/// Full analysis result. `plan` is the display payload for the UI; the
/// numeric fields are consumed by the scoring engine in DI-4b.
pub struct AnalysisResult {
    pub plan: DraftPlan,
    /// Highest combo strength with any current ally (0.0 if none).
    pub combo_bonus: f32,
    /// How well the candidate's damage type balances the team (0.0–1.0).
    pub damage_balance: f32,
    /// How well the candidate fills detected team role gaps (0.0–1.0).
    pub team_need_score: f32,
    /// Risk penalty applied when picking blind with low blind_safety (0.0 or 0.05).
    pub blind_unsafety: f32,
}

// ── Public building blocks ─────────────────────────────────────────────────

/// Returns `(best_combo_strength, Vec<ComboHint>)`.
/// `ally_id_keys`: `(champion_id, key)` slices for all allies with archetype data.
pub fn compute_combo_bonus(
    candidate_key: &str,
    ally_id_keys: &[(u32, &str)],
    combo_dir: &ComboDirectory,
) -> (f32, Vec<ComboHint>) {
    let ally_keys: Vec<&str> = ally_id_keys.iter().map(|(_, k)| *k).collect();
    let matches = combo_dir.find_for_ally(candidate_key, &ally_keys);
    if matches.is_empty() {
        return (0.0, vec![]);
    }

    let best_strength = matches.iter().map(|c| c.strength).fold(0.0f32, f32::max);

    let hints: Vec<ComboHint> = matches
        .iter()
        .filter_map(|combo| {
            let partner_key = if combo.a.eq_ignore_ascii_case(candidate_key) {
                &combo.b
            } else {
                &combo.a
            };
            let (ally_id, _) = ally_id_keys
                .iter()
                .find(|(_, k)| partner_key.eq_ignore_ascii_case(k))?;

            let combo_type = match combo.combo_type.as_str() {
                "pick_potential" => ComboType::PickPotential,
                "zone_control" => ComboType::ZoneControl,
                "peel_chain" => ComboType::PeelChain,
                "wombo" => ComboType::Wombo,
                _ => ComboType::EngageFollowup,
            };

            Some(ComboHint {
                ally_champion_id: *ally_id,
                ally_champion_key: partner_key.clone(),
                combo_text: combo.tr.clone(),
                combo_type,
            })
        })
        .collect();

    (best_strength, hints)
}

/// Returns 0.0 (poor balance) to 1.0 (ideal balance).
/// High when the candidate's damage type complements what the team currently lacks.
pub fn compute_damage_balance(
    candidate: &ChampionArchetype,
    ally_archetypes: &[&ChampionArchetype],
) -> f32 {
    if ally_archetypes.is_empty() {
        return 0.5;
    }
    let team_ap_avg: f32 = ally_archetypes
        .iter()
        .map(|a| a.damage_profile.ap)
        .sum::<f32>()
        / ally_archetypes.len() as f32;

    // If team is AP-heavy, adding AD candidate is beneficial (and vice-versa).
    if team_ap_avg > 0.5 {
        candidate.damage_profile.ad
    } else {
        candidate.damage_profile.ap
    }
}

/// Returns `(fill_score, fills_team_need_strings)`.
/// Detects engage, frontline, hard-CC, and peel gaps in the current ally team
/// and scores how many the candidate fills.
pub fn compute_team_need_fill(
    candidate: &ChampionArchetype,
    ally_archetypes: &[&ChampionArchetype],
) -> (f32, Vec<String>) {
    let has_initiator = ally_archetypes.iter().any(|a| a.engage_role == "initiator");
    let has_frontline = ally_archetypes
        .iter()
        .any(|a| matches!(a.archetype.as_str(), "vanguard" | "juggernaut" | "warden"));
    let has_hard_cc = ally_archetypes.iter().any(|a| a.cc.has_hard_cc);
    let has_peel = ally_archetypes
        .iter()
        .any(|a| matches!(a.peel_capability.as_str(), "medium" | "high"));

    let total_gaps =
        (!has_initiator) as u8 + (!has_frontline) as u8 + (!has_hard_cc) as u8 + (!has_peel) as u8;

    if total_gaps == 0 {
        return (0.5, vec![]);
    }

    let mut fills = Vec::new();
    let mut filled = 0u8;

    if !has_initiator && candidate.engage_role == "initiator" {
        fills.push("Engage eksiği giderilir".to_string());
        filled += 1;
    }
    if !has_frontline
        && matches!(
            candidate.archetype.as_str(),
            "vanguard" | "juggernaut" | "warden"
        )
    {
        fills.push("Frontline eksiği giderilir".to_string());
        filled += 1;
    }
    if !has_hard_cc && candidate.cc.has_hard_cc {
        fills.push("CC eksiği giderilir".to_string());
        filled += 1;
    }
    if !has_peel && matches!(candidate.peel_capability.as_str(), "medium" | "high") {
        fills.push("Koruma eksiği giderilir".to_string());
        filled += 1;
    }

    let score = filled as f32 / total_gaps as f32;
    (score, fills)
}

/// Returns an extra risk penalty (0.0 or 0.05) for first-picking an unsafe champion.
pub fn compute_blind_unsafety(is_first_pick: bool, blind_safety: f32) -> f32 {
    if is_first_pick && blind_safety < 0.60 {
        0.05
    } else {
        0.0
    }
}

/// Returns a counter bonus [0.0, 0.30] and Turkish descriptor strings.
///
/// Checks `candidate.counters_archetypes` against enemy archetype strings.
/// Each matching enemy archetype contributes +0.15 (capped at 0.30).
pub fn compute_counter_bonus(
    candidate: &ChampionArchetype,
    enemy_archetypes: &[&ChampionArchetype],
) -> (f32, Vec<String>) {
    if candidate.counters_archetypes.is_empty() || enemy_archetypes.is_empty() {
        return (0.0, vec![]);
    }

    let fills: Vec<String> = enemy_archetypes
        .iter()
        .filter(|e| {
            candidate
                .counters_archetypes
                .iter()
                .any(|ca| ca == &e.archetype)
        })
        .map(|e| {
            format!(
                "{} komposyonuna karşı avantajlı",
                archetype_display_tr(&e.archetype)
            )
        })
        .collect();

    let bonus = (fills.len() as f32 * 0.15).min(0.30);
    (bonus, fills)
}

fn archetype_display_tr(archetype: &str) -> &str {
    match archetype {
        "vanguard" => "frontline engage",
        "juggernaut" => "dayanıklı fighter",
        "diver" => "diver",
        "skirmisher" => "duelist",
        "assassin" => "assassin",
        "control_mage" => "kontrol büyücüsü",
        "burst_mage" => "burst büyücüsü",
        "artillery" => "poke büyücüsü",
        "marksman" => "ADC",
        "catcher" => "pick büyücüsü",
        "enchanter" => "enchanter",
        "warden" => "tank warden",
        _ => archetype,
    }
}

/// Returns informational fill strings for utility-tag gaps (waveclear, poke, disengage).
/// Does NOT affect numeric `team_need_score` — purely surfaced to the UI.
pub fn compute_utility_fills(
    candidate: &ChampionArchetype,
    ally_archetypes: &[&ChampionArchetype],
) -> Vec<String> {
    let has_tag = |tag: &str| {
        ally_archetypes
            .iter()
            .any(|a| a.utility_tags.iter().any(|t| t == tag))
    };

    let candidate_has = |tag: &str| candidate.utility_tags.iter().any(|t| t == tag);

    let mut fills = Vec::new();
    if !has_tag("waveclear") && candidate_has("waveclear") {
        fills.push("Waveclear sağlar (lane kontrolü)".to_string());
    }
    if !has_tag("poke") && candidate_has("poke") {
        fills.push("Poke baskısı ekler".to_string());
    }
    if !has_tag("disengage") && candidate_has("disengage") {
        fills.push("Disengage kapasitesi sağlar".to_string());
    }
    fills
}

// ── Top-level pipeline ─────────────────────────────────────────────────────

/// Runs the full Draft IQ analysis for a single candidate champion.
/// Pure function — no I/O, no side effects.
pub fn analyze_pick(input: &AnalyzerInput<'_>) -> AnalysisResult {
    let ally_id_keys_ref: Vec<(u32, &str)> = input
        .ally_id_keys
        .iter()
        .map(|(id, k)| (*id, k.as_str()))
        .collect();

    let (combo_bonus, combo_with) =
        compute_combo_bonus(input.candidate_key, &ally_id_keys_ref, input.combo_dir);

    let damage_balance = compute_damage_balance(input.candidate, &input.ally_archetypes);
    let (team_need_score, mut fills_team_need) =
        compute_team_need_fill(input.candidate, &input.ally_archetypes);

    // Wire previously-unused fields: counters_archetypes + utility_tags
    let (counter_bonus, counter_fills) =
        compute_counter_bonus(input.candidate, &input.enemy_archetypes);
    fills_team_need.extend(counter_fills);
    fills_team_need.extend(compute_utility_fills(
        input.candidate,
        &input.ally_archetypes,
    ));

    // Counter bonus blends into team_need_score at half-weight (doesn't overpower base gaps)
    let combined_need_score = (team_need_score + counter_bonus * 0.5).min(1.0);

    let blind_unsafety = compute_blind_unsafety(input.is_first_pick, input.candidate.blind_safety);

    let threats = narrative::build_threats_text(
        input.candidate,
        &input.enemy_archetypes,
        &input.ally_archetypes,
    );

    // Counter-pick window note: only when late pick AND candidate counters visible enemies.
    let pick_window_note = if input.is_late_pick && !input.enemy_archetypes.is_empty() {
        let countered: Vec<&str> = input
            .enemy_archetypes
            .iter()
            .filter(|e| {
                input
                    .candidate
                    .counters_archetypes
                    .iter()
                    .any(|ca| ca == &e.archetype)
            })
            .map(|e| archetype_display_tr(&e.archetype))
            .collect();
        if countered.is_empty() {
            None
        } else {
            Some(format!(
                "{}. pick penceresi — {} sayar",
                input.pick_order,
                countered.join(" + ")
            ))
        }
    } else {
        None
    };

    let enemy_wc = narrative::detect_enemy_win_condition(&input.enemy_archetypes);
    let comp_clash_note =
        narrative::build_comp_clash_note(&input.candidate.win_condition, enemy_wc);
    let spike_note = narrative::build_spike_note(input.candidate);
    let lane_phase_advice =
        narrative::build_lane_phase_advice(input.candidate, input.lane_opponent, input.position);

    let plan = DraftPlan {
        combo_with,
        win_condition: narrative::build_win_condition_text(&input.candidate.win_condition),
        team_role: narrative::build_team_role_text(input.candidate),
        damage_profile: narrative::build_damage_profile_label(&input.candidate.damage_profile),
        blind_pick_safety: input.candidate.blind_safety,
        execution_difficulty: input.candidate.execution_difficulty,
        threats,
        fills_team_need,
        risk_note: None, // set by engine when stretch pick (DI-4b)
        pick_window_note,
        comp_clash_note,
        spike_note,
        lane_phase_advice,
    };

    AnalysisResult {
        plan,
        combo_bonus,
        damage_balance,
        team_need_score: combined_need_score,
        blind_unsafety,
    }
}
