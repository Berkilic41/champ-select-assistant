//! Draft Simulator Core (Sprint F, Claude) — engine-pure "move simulation".
//!
//! Answers "what does adding champion X do to my comp?" across team-composition
//! dimensions (damage balance, engage/disengage, frontline/peel, scaling, lane
//! pressure, objective identity, execution risk, blind safety, synergy). Fully
//! pure: no DB/network/engine-hot-file coupling. The trait values are derived from
//! the SAME KB archetype taxonomy `champion_types` / `scouting` already use, so the
//! reads are grounded heuristics, never fabricated stats — every coaching line is
//! hedged (no guaranteed-outcome language, enforced by `coach_quality`).
//!
//! Factor names in `improved_factors` / `worsened_factors` are stable machine keys
//! (UI i18n-maps them); prose fields (`coach_sentence`, …) are Turkish.
#![allow(dead_code)] // public DTOs + engine consumed by a command/UI in a later turn

use serde::{Deserialize, Serialize};
use ts_rs::TS;

const EPS: f32 = 0.02;

/// Champion damage profile (for team damage-balance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub enum DamageType {
    Ad,
    Ap,
    Mixed,
    True,
}

/// One champion in the simulated draft. `archetype` is a KB fine-grained string
/// ("assassin", "vanguard", "enchanter", …); `combo_partner_ids` are KB combo links
/// the caller resolves read-only from Draft IQ.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct SimChampion {
    pub champion_id: u32,
    pub champion_key: String,
    pub archetype: String,
    pub damage: DamageType,
    #[serde(default)]
    pub combo_partner_ids: Vec<u32>,
}

/// Current draft composition under evaluation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct DraftSimState {
    pub my_team: Vec<SimChampion>,
    #[serde(default)]
    pub enemy_team: Vec<SimChampion>,
    #[serde(default)]
    pub blind: bool,
    #[serde(default)]
    pub first_pick: bool,
}

/// A candidate pick to add to `my_team`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct DraftSimMove {
    pub champion: SimChampion,
    #[serde(default)]
    pub position: Option<String>,
}

/// Full simulation request: a state plus candidate moves to compare.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct DraftSimInput {
    pub state: DraftSimState,
    #[serde(default)]
    pub candidate_moves: Vec<DraftSimMove>,
}

/// One factor's before/after change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct DraftSimDelta {
    pub factor: String,
    pub before: f32,
    pub after: f32,
    pub delta: f32,
}

/// Risk read for a move/state. `level` ∈ {low, medium, high}.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct DraftSimRisk {
    pub level: String,
    pub summary: String,
    pub factors: Vec<String>,
}

/// How the team's win-plan identity shifts after the move.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct DraftSimPlanShift {
    pub before: String,
    pub after: String,
    pub note: String,
}

/// Evaluation of one move (or whole state, vs an empty baseline).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct DraftSimResult {
    pub champion_id: u32,
    pub champion_key: String,
    pub score_delta: f32,
    pub improved_factors: Vec<String>,
    pub worsened_factors: Vec<String>,
    pub deltas: Vec<DraftSimDelta>,
    pub risk: DraftSimRisk,
    pub plan_shift: DraftSimPlanShift,
    pub coach_sentence: String,
    pub why_this_move: String,
    pub why_not_alternative: String,
}

// ── Internal per-archetype trait profile (grounded in the KB taxonomy) ──────────

#[derive(Clone, Copy)]
struct Profile {
    engage: f32,
    disengage: f32,
    frontline: f32,
    peel: f32,
    scaling: f32,
    lane_pressure: f32,
    objective: f32,
    execution_risk: f32,
    blind_safety: f32,
}

fn profile_for(archetype: &str) -> Profile {
    // (engage, disengage, frontline, peel, scaling, lane_pressure, objective, exec_risk, blind_safety)
    let p = |e, d, f, pe, s, l, o, x, b| Profile {
        engage: e,
        disengage: d,
        frontline: f,
        peel: pe,
        scaling: s,
        lane_pressure: l,
        objective: o,
        execution_risk: x,
        blind_safety: b,
    };
    match archetype.to_lowercase().as_str() {
        "assassin" => p(0.5, 0.1, 0.1, 0.1, 0.4, 0.6, 0.3, 0.8, 0.3),
        "diver" => p(0.8, 0.2, 0.5, 0.2, 0.4, 0.6, 0.5, 0.6, 0.5),
        "juggernaut" => p(0.4, 0.3, 0.7, 0.3, 0.5, 0.5, 0.5, 0.3, 0.6),
        "skirmisher" => p(0.4, 0.4, 0.3, 0.3, 0.6, 0.6, 0.4, 0.7, 0.4),
        "marksman" => p(0.1, 0.2, 0.1, 0.2, 0.9, 0.4, 0.6, 0.5, 0.6),
        "burst_mage" => p(0.3, 0.3, 0.1, 0.2, 0.6, 0.6, 0.5, 0.5, 0.5),
        "control_mage" => p(0.3, 0.5, 0.1, 0.4, 0.8, 0.5, 0.7, 0.4, 0.6),
        "battle_mage" => p(0.5, 0.3, 0.4, 0.3, 0.6, 0.5, 0.5, 0.4, 0.6),
        "artillery" => p(0.1, 0.5, 0.1, 0.3, 0.7, 0.7, 0.8, 0.5, 0.7),
        "vanguard" => p(0.9, 0.4, 0.9, 0.4, 0.4, 0.4, 0.6, 0.3, 0.7),
        "warden" => p(0.3, 0.8, 0.8, 0.8, 0.4, 0.3, 0.5, 0.2, 0.8),
        "enchanter" => p(0.1, 0.7, 0.1, 0.9, 0.7, 0.3, 0.5, 0.4, 0.7),
        "catcher" => p(0.6, 0.5, 0.3, 0.5, 0.5, 0.5, 0.6, 0.5, 0.5),
        _ => p(0.4, 0.4, 0.4, 0.4, 0.5, 0.4, 0.4, 0.5, 0.5),
    }
}

#[derive(Clone, Copy, Default)]
struct StateEval {
    damage_balance: f32,
    engage: f32,
    disengage: f32,
    frontline: f32,
    peel: f32,
    scaling: f32,
    lane_pressure: f32,
    objective_identity: f32,
    execution_risk: f32,
    blind_safety: f32,
    synergy: f32,
}

impl StateEval {
    fn composite(&self) -> f32 {
        self.damage_balance
            + self.engage * 0.5
            + self.disengage * 0.4
            + self.frontline * 0.7
            + self.peel * 0.7
            + self.lane_pressure * 0.6
            + self.objective_identity * 0.8
            + self.blind_safety * 0.5
            + self.synergy * 0.6
            - self.execution_risk
    }
}

fn mean(vals: &[f32]) -> f32 {
    if vals.is_empty() {
        0.0
    } else {
        vals.iter().sum::<f32>() / vals.len() as f32
    }
}

/// Damage balance in [0, 1]: 1.0 when AD/AP are evenly split, ~0 when mono-damage.
fn damage_balance(team: &[SimChampion]) -> f32 {
    if team.is_empty() {
        return 0.0;
    }
    let (mut ad, mut ap) = (0.0f32, 0.0f32);
    for c in team {
        match c.damage {
            DamageType::Ad => ad += 1.0,
            DamageType::Ap => ap += 1.0,
            DamageType::Mixed => {
                ad += 0.5;
                ap += 0.5;
            }
            DamageType::True => {}
        }
    }
    let total = ad + ap;
    if total <= 0.0 {
        return 0.5; // all-true → neutral
    }
    1.0 - (ad - ap).abs() / total
}

fn dominant_damage(team: &[SimChampion]) -> Option<DamageType> {
    let (mut ad, mut ap) = (0.0f32, 0.0f32);
    for c in team {
        match c.damage {
            DamageType::Ad => ad += 1.0,
            DamageType::Ap => ap += 1.0,
            DamageType::Mixed => {
                ad += 0.5;
                ap += 0.5;
            }
            DamageType::True => {}
        }
    }
    if ad == 0.0 && ap == 0.0 {
        None
    } else if ad >= ap {
        Some(DamageType::Ad)
    } else {
        Some(DamageType::Ap)
    }
}

fn eval_state(state: &DraftSimState) -> StateEval {
    let team = &state.my_team;
    if team.is_empty() {
        return StateEval::default();
    }
    let profiles: Vec<Profile> = team.iter().map(|c| profile_for(&c.archetype)).collect();
    let engages: Vec<f32> = profiles.iter().map(|p| p.engage).collect();
    let max_engage = engages.iter().cloned().fold(0.0f32, f32::max);

    let scaling = mean(&profiles.iter().map(|p| p.scaling).collect::<Vec<_>>());
    let lane_pressure = mean(&profiles.iter().map(|p| p.lane_pressure).collect::<Vec<_>>());
    // Greedy-scaling penalty: a late comp with no early pressure is execution-risky.
    let greedy = (scaling - lane_pressure).max(0.0) * 0.6;
    let execution_risk = (mean(
        &profiles
            .iter()
            .map(|p| p.execution_risk)
            .collect::<Vec<_>>(),
    ) + greedy)
        .min(1.0);

    // Blind safety: the riskiest blind pick drags the team down (only when blind).
    let blind_safety = profiles
        .iter()
        .map(|p| p.blind_safety)
        .fold(1.0f32, f32::min);

    // Synergy: fraction of team members linked to an on-team combo partner.
    let ids: std::collections::HashSet<u32> = team.iter().map(|c| c.champion_id).collect();
    let linked = team
        .iter()
        .filter(|c| c.combo_partner_ids.iter().any(|p| ids.contains(p)))
        .count();
    let synergy = linked as f32 / team.len() as f32;

    StateEval {
        damage_balance: damage_balance(team),
        engage: 0.6 * max_engage + 0.4 * mean(&engages),
        disengage: mean(&profiles.iter().map(|p| p.disengage).collect::<Vec<_>>()),
        frontline: mean(&profiles.iter().map(|p| p.frontline).collect::<Vec<_>>()),
        peel: mean(&profiles.iter().map(|p| p.peel).collect::<Vec<_>>()),
        scaling,
        lane_pressure,
        objective_identity: mean(&profiles.iter().map(|p| p.objective).collect::<Vec<_>>()),
        execution_risk,
        blind_safety,
        synergy,
    }
}

// ── Public engine ───────────────────────────────────────────────────────────────

/// Add a champion to `my_team`, returning the new state. Pure (clones).
pub fn apply_move(state: &DraftSimState, mv: &DraftSimMove) -> DraftSimState {
    let mut next = state.clone();
    next.my_team.push(mv.champion.clone());
    next
}

/// Stand-alone read of a composition (delta is measured vs an empty baseline).
pub fn evaluate_state(state: &DraftSimState) -> DraftSimResult {
    let base = StateEval::default();
    let after = eval_state(state);
    let champ = state.my_team.last();
    build_result(
        champ.map(|c| c.champion_id).unwrap_or(0),
        champ.map(|c| c.champion_key.clone()).unwrap_or_default(),
        &base,
        &after,
        state,
        None,
    )
}

/// Evaluate each candidate move against the same base state. Deterministic:
/// sorted by `score_delta` desc, ties broken by `champion_id` asc.
pub fn compare_moves(state: &DraftSimState, moves: &[DraftSimMove]) -> Vec<DraftSimResult> {
    let before = eval_state(state);
    let mut out: Vec<DraftSimResult> = moves
        .iter()
        .map(|mv| {
            let next = apply_move(state, mv);
            let after = eval_state(&next);
            build_result(
                mv.champion.champion_id,
                mv.champion.champion_key.clone(),
                &before,
                &after,
                &next,
                Some(&mv.champion),
            )
        })
        .collect();
    out.sort_by(|a, b| {
        b.score_delta
            .partial_cmp(&a.score_delta)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.champion_id.cmp(&b.champion_id))
    });
    out
}

/// Convenience entry for the (future) command layer.
pub fn simulate(input: &DraftSimInput) -> Vec<DraftSimResult> {
    compare_moves(&input.state, &input.candidate_moves)
}

// ── Result building + grounded, hedged coaching ─────────────────────────────────

/// Stable machine factor keys (UI i18n-maps); directional `higher_is_better`.
const DIRECTIONAL: &[(&str, bool)] = &[
    ("damage_balance", true),
    ("engage", true),
    ("disengage", true),
    ("frontline", true),
    ("peel", true),
    ("lane_pressure", true),
    ("objective_identity", true),
    ("execution_risk", false),
    ("blind_safety", true),
    ("synergy", true),
];

fn field(e: &StateEval, key: &str) -> f32 {
    match key {
        "damage_balance" => e.damage_balance,
        "engage" => e.engage,
        "disengage" => e.disengage,
        "frontline" => e.frontline,
        "peel" => e.peel,
        "scaling" => e.scaling,
        "lane_pressure" => e.lane_pressure,
        "objective_identity" => e.objective_identity,
        "execution_risk" => e.execution_risk,
        "blind_safety" => e.blind_safety,
        "synergy" => e.synergy,
        _ => 0.0,
    }
}

fn tr_label(key: &str) -> &'static str {
    match key {
        "damage_balance" => "hasar dengesi",
        "engage" => "engage",
        "disengage" => "disengage",
        "frontline" => "ön saf",
        "peel" => "peel",
        "scaling" => "scaling",
        "lane_pressure" => "koridor baskısı",
        "objective_identity" => "obje kimliği",
        "execution_risk" => "uygulama riski",
        "blind_safety" => "blind güvenliği",
        "synergy" => "sinerji",
        _ => "faktör",
    }
}

fn build_result(
    champion_id: u32,
    champion_key: String,
    before: &StateEval,
    after: &StateEval,
    state_after: &DraftSimState,
    mv: Option<&SimChampion>,
) -> DraftSimResult {
    let mut improved = Vec::new();
    let mut worsened = Vec::new();
    let mut deltas = Vec::new();

    for (key, higher_better) in DIRECTIONAL {
        let b = field(before, key);
        let a = field(after, key);
        let benefit = if *higher_better { a - b } else { b - a };
        if benefit > EPS {
            improved.push((*key).to_string());
        } else if benefit < -EPS {
            worsened.push((*key).to_string());
        }
        deltas.push(DraftSimDelta {
            factor: (*key).to_string(),
            before: b,
            after: a,
            delta: a - b,
        });
    }
    // scaling is a curve, not good/bad — reported in deltas only.
    deltas.push(DraftSimDelta {
        factor: "scaling".to_string(),
        before: before.scaling,
        after: after.scaling,
        delta: after.scaling - before.scaling,
    });

    // Reinforcing an already mono-damage comp worsens balance even if the float floors.
    if let Some(champ) = mv {
        if after.damage_balance < 0.6
            && dominant_damage(&state_after.my_team) == Some(champ.damage)
            && !improved.iter().any(|k| k == "damage_balance")
            && !worsened.iter().any(|k| k == "damage_balance")
        {
            worsened.push("damage_balance".to_string());
        }
    }

    let risk = build_risk(after, state_after);
    let plan_shift = build_plan_shift(before, after);
    let (coach_sentence, why_this_move, why_not_alternative) =
        build_coaching(&champion_key, &improved, &worsened, &risk);

    DraftSimResult {
        champion_id,
        champion_key,
        score_delta: after.composite() - before.composite(),
        improved_factors: improved,
        worsened_factors: worsened,
        deltas,
        risk,
        plan_shift,
        coach_sentence,
        why_this_move,
        why_not_alternative,
    }
}

fn build_risk(after: &StateEval, state: &DraftSimState) -> DraftSimRisk {
    let mut factors = Vec::new();
    let mut severe = false;
    if after.execution_risk > 0.7 {
        factors.push("execution_risk".to_string());
        severe = true;
    } else if after.execution_risk > 0.55 {
        factors.push("execution_risk".to_string());
    }
    if state.blind && state.first_pick && after.blind_safety < 0.5 {
        factors.push("blind_safety".to_string());
        severe = true;
    }
    // Mono-damage only reads as a risk once the team is real (>= 3 picks); a 1–2
    // champ partial draft is trivially "imbalanced" and shouldn't be flagged.
    if state.my_team.len() >= 3 && after.damage_balance < 0.35 {
        factors.push("damage_balance".to_string());
    }
    let level = if severe {
        "high"
    } else if !factors.is_empty() {
        "medium"
    } else {
        "low"
    };
    let summary = if factors.is_empty() {
        "Belirgin risk yok; dengeli ekleme.".to_string()
    } else {
        let labels: Vec<&str> = factors.iter().map(|k| tr_label(k)).collect();
        format!("Dikkat: {}.", labels.join(", "))
    };
    DraftSimRisk {
        level: level.to_string(),
        summary,
        factors,
    }
}

fn identity_label(e: &StateEval) -> &'static str {
    // Pick the dominant strategic identity from the strongest factor.
    let candidates = [
        ("engage/dalış", e.engage),
        ("frontline/teamfight", e.frontline),
        ("poke/obje", e.objective_identity.max(e.disengage)),
        ("koru-ve-DPS", e.peel),
        ("scaling/geç oyun", e.scaling * 0.8),
        ("koridor baskısı", e.lane_pressure),
    ];
    candidates
        .iter()
        .fold(("dengeli", 0.0f32), |acc, &(label, v)| {
            if v > acc.1 {
                (label, v)
            } else {
                acc
            }
        })
        .0
}

fn build_plan_shift(before: &StateEval, after: &StateEval) -> DraftSimPlanShift {
    let b = identity_label(before);
    let a = identity_label(after);
    let note = if b == a {
        format!("Kimlik aynı yönde derinleşiyor: {a}.")
    } else {
        format!("Plan {b} ekseninden {a} eksenine kayıyor.")
    };
    DraftSimPlanShift {
        before: b.to_string(),
        after: a.to_string(),
        note,
    }
}

fn build_coaching(
    champion_key: &str,
    improved: &[String],
    worsened: &[String],
    risk: &DraftSimRisk,
) -> (String, String, String) {
    let join = |keys: &[String]| {
        keys.iter()
            .take(2)
            .map(|k| tr_label(k))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let head = if champion_key.is_empty() {
        "Bu kompozisyon".to_string()
    } else {
        champion_key.to_string()
    };

    let coach_sentence = match (improved.is_empty(), worsened.is_empty()) {
        (false, false) => format!(
            "{head}: {} güçleniyor, {} zayıflıyor.",
            join(improved),
            join(worsened)
        ),
        (false, true) => format!("{head}: {} yönünde katkı sağlıyor.", join(improved)),
        (true, false) => format!(
            "{head}: {} tarafını zorluyor; dikkatli oyna.",
            join(worsened)
        ),
        (true, true) => format!("{head}: tabloyu belirgin değiştirmiyor."),
    };

    let why_this_move = if improved.is_empty() {
        "Belirgin bir boyutu güçlendirmiyor; konfor/rol gerekçesi ağır basmalı.".to_string()
    } else {
        format!("{} tarafına somut katkı veriyor.", join(improved))
    };

    let why_not_alternative = if risk.factors.is_empty() && worsened.is_empty() {
        "Alternatif için belirgin bir dezavantaj sinyali yok.".to_string()
    } else if !worsened.is_empty() {
        format!(
            "Alternatif düşün: {} zayıflıyor ({}).",
            join(worsened),
            risk.level
        )
    } else {
        format!("Alternatif düşün: {}", risk.summary)
    };

    (coach_sentence, why_this_move, why_not_alternative)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coach_quality::has_absolute_language;

    fn champ(id: u32, key: &str, archetype: &str, damage: DamageType) -> SimChampion {
        SimChampion {
            champion_id: id,
            champion_key: key.into(),
            archetype: archetype.into(),
            damage,
            combo_partner_ids: vec![],
        }
    }

    fn mv(c: SimChampion) -> DraftSimMove {
        DraftSimMove {
            champion: c,
            position: None,
        }
    }

    fn state(team: Vec<SimChampion>) -> DraftSimState {
        DraftSimState {
            my_team: team,
            ..Default::default()
        }
    }

    fn no_absolute(r: &DraftSimResult) {
        for s in [
            &r.coach_sentence,
            &r.why_this_move,
            &r.why_not_alternative,
            &r.risk.summary,
        ] {
            assert!(!has_absolute_language(s), "over-promising language: {s}");
        }
    }

    #[test]
    fn adding_ap_to_ap_heavy_comp_worsens_damage_balance() {
        let team = vec![
            champ(1, "Ahri", "burst_mage", DamageType::Ap),
            champ(2, "Syndra", "control_mage", DamageType::Ap),
            champ(3, "Viktor", "control_mage", DamageType::Ap),
            champ(4, "Lux", "burst_mage", DamageType::Ap),
        ];
        let results = compare_moves(
            &state(team),
            &[mv(champ(5, "Brand", "burst_mage", DamageType::Ap))],
        );
        assert!(results[0]
            .worsened_factors
            .contains(&"damage_balance".to_string()));
        no_absolute(&results[0]);
    }

    #[test]
    fn adding_engage_to_no_engage_comp_improves_identity() {
        let team = vec![
            champ(1, "Jinx", "marksman", DamageType::Ad),
            champ(2, "Lulu", "enchanter", DamageType::Ap),
        ];
        let results = compare_moves(
            &state(team),
            &[mv(champ(3, "Leona", "vanguard", DamageType::Ap))],
        );
        assert!(
            results[0].improved_factors.contains(&"engage".to_string()),
            "vanguard must improve engage: {:?}",
            results[0].improved_factors
        );
        no_absolute(&results[0]);
    }

    #[test]
    fn adding_scaling_to_scaling_comp_raises_early_risk() {
        // Already greedy: marksman + control_mage (high scaling, low lane pressure).
        let team = vec![
            champ(1, "Jinx", "marksman", DamageType::Ad),
            champ(2, "Viktor", "control_mage", DamageType::Ap),
        ];
        let before = eval_state(&state(team.clone()));
        let next = apply_move(
            &state(team),
            &mv(champ(3, "Kayle", "marksman", DamageType::Mixed)),
        );
        let after = eval_state(&next);
        assert!(
            after.execution_risk > before.execution_risk,
            "more scaling with no early pressure must raise execution risk ({} -> {})",
            before.execution_risk,
            after.execution_risk
        );
    }

    #[test]
    fn blind_unsafe_assassin_first_pick_risk_surfaces() {
        let st = DraftSimState {
            my_team: vec![],
            enemy_team: vec![],
            blind: true,
            first_pick: true,
        };
        let results = compare_moves(&st, &[mv(champ(1, "Zed", "assassin", DamageType::Ad))]);
        assert_eq!(results[0].risk.level, "high");
        assert!(results[0]
            .risk
            .factors
            .contains(&"blind_safety".to_string()));
        no_absolute(&results[0]);
    }

    #[test]
    fn combo_pick_improves_synergy_without_hiding_execution_risk() {
        // Team has champ 1; the move (champ 2) combos with 1 AND is high-execution.
        let team = vec![champ(1, "Orianna", "control_mage", DamageType::Ap)];
        let mut combo = champ(2, "Yasuo", "skirmisher", DamageType::Ad); // exec_risk 0.7
        combo.combo_partner_ids = vec![1];
        // champ 1 also links back so synergy registers for both.
        let mut team = team;
        team[0].combo_partner_ids = vec![2];
        let results = compare_moves(&state(team), &[mv(combo)]);
        let r = &results[0];
        assert!(
            r.improved_factors.contains(&"synergy".to_string()),
            "synergy up"
        );
        assert!(
            r.risk.factors.contains(&"execution_risk".to_string()),
            "execution risk must still surface: {:?}",
            r.risk.factors
        );
        no_absolute(r);
    }

    #[test]
    fn compare_moves_is_deterministic_and_sorted() {
        let team = vec![champ(1, "Jinx", "marksman", DamageType::Ad)];
        let moves = vec![
            mv(champ(2, "Leona", "vanguard", DamageType::Ap)),
            mv(champ(3, "Thresh", "catcher", DamageType::Ap)),
            mv(champ(4, "Nautilus", "vanguard", DamageType::Ap)),
        ];
        let a = compare_moves(&state(team.clone()), &moves);
        let b = compare_moves(&state(team), &moves);
        assert_eq!(a, b, "same input → same output");
        for w in a.windows(2) {
            assert!(
                w[0].score_delta > w[1].score_delta
                    || (w[0].score_delta == w[1].score_delta
                        && w[0].champion_id < w[1].champion_id),
                "results must be sorted by score_delta desc then id asc"
            );
        }
    }

    #[test]
    fn empty_and_partial_drafts_are_safe() {
        // No champions: evaluate_state must not panic and yields a neutral read.
        let empty = evaluate_state(&DraftSimState::default());
        assert_eq!(empty.champion_id, 0);
        assert!(empty.risk.level == "low" || empty.risk.level == "medium");
        // No moves → empty comparison.
        assert!(compare_moves(&DraftSimState::default(), &[]).is_empty());
        // Single-champion partial draft evaluates cleanly.
        let partial = evaluate_state(&state(vec![champ(1, "Zed", "assassin", DamageType::Ad)]));
        no_absolute(&partial);
    }
}
