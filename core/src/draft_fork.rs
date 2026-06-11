//! Draft Fork analysis (Sprint G, Claude) — engine-pure "A pick mi B pick mi?".
//!
//! Given a draft state and two candidate moves, contrasts what each does: how the
//! win-plan identity diverges, how risk differs, which factors they share vs split
//! on, and a HEDGED leaning (never a guaranteed-outcome claim). Pure — builds on
//! the public `draft_simulator` API only; no DB/network/hot-file coupling.
#![allow(dead_code)] // public DTO + helper consumed by a command/UI in a later turn

use crate::draft_simulator::{
    compare_moves, DraftSimMove, DraftSimResult, DraftSimState,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use ts_rs::TS;

/// Side-by-side fork read for two candidate picks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct DraftFork {
    pub option_a: DraftSimResult,
    pub option_b: DraftSimResult,
    /// How the team's plan identity diverges (TR).
    pub plan_divergence: String,
    /// How execution/comp risk diverges (TR).
    pub risk_divergence: String,
    /// Factors BOTH picks improve.
    pub shared_factors: Vec<String>,
    /// Factors where the picks disagree (one improves, the other worsens).
    pub diverging_factors: Vec<String>,
    /// Hedged leaning — which option edges ahead and why. No guaranteed-outcome dil.
    pub recommendation: String,
}

fn eval_one(state: &DraftSimState, mv: &DraftSimMove) -> DraftSimResult {
    compare_moves(state, std::slice::from_ref(mv))
        .into_iter()
        .next()
        .expect("compare_moves yields one result for one move")
}

/// Contrast two candidate picks from the same state. Deterministic + pure.
pub fn compare_fork(
    state: &DraftSimState,
    move_a: &DraftSimMove,
    move_b: &DraftSimMove,
) -> DraftFork {
    let a = eval_one(state, move_a);
    let b = eval_one(state, move_b);

    let a_imp: HashSet<&String> = a.improved_factors.iter().collect();
    let b_imp: HashSet<&String> = b.improved_factors.iter().collect();
    let a_wor: HashSet<&String> = a.worsened_factors.iter().collect();
    let b_wor: HashSet<&String> = b.worsened_factors.iter().collect();

    let mut shared_factors: Vec<String> =
        a_imp.intersection(&b_imp).map(|s| (*s).clone()).collect();
    shared_factors.sort();
    // Diverging: A improves while B worsens, or vice-versa.
    let mut diverging: HashSet<String> = HashSet::new();
    for f in a_imp.intersection(&b_wor) {
        diverging.insert((*f).clone());
    }
    for f in b_imp.intersection(&a_wor) {
        diverging.insert((*f).clone());
    }
    let mut diverging_factors: Vec<String> = diverging.into_iter().collect();
    diverging_factors.sort();

    let plan_divergence = if a.plan_shift.after == b.plan_shift.after {
        format!("İki pick de planı {} eksenine çekiyor.", a.plan_shift.after)
    } else {
        format!(
            "{} → {} ekseni; {} → {} ekseni.",
            a.champion_key, a.plan_shift.after, b.champion_key, b.plan_shift.after
        )
    };

    let risk_divergence = if a.risk.level == b.risk.level {
        format!("Risk seviyesi benzer ({}).", a.risk.level)
    } else {
        format!(
            "{} riski {}, {} riski {}.",
            a.champion_key, a.risk.level, b.champion_key, b.risk.level
        )
    };

    let recommendation = build_recommendation(&a, &b);

    DraftFork {
        option_a: a,
        option_b: b,
        plan_divergence,
        risk_divergence,
        shared_factors,
        diverging_factors,
        recommendation,
    }
}

fn build_recommendation(a: &DraftSimResult, b: &DraftSimResult) -> String {
    let margin = a.score_delta - b.score_delta;
    if margin.abs() < 0.05 {
        "İki seçenek yakın; tercih konfor, rol uyumu ve karşı drafta göre verilebilir.".to_string()
    } else if margin > 0.0 {
        format!(
            "{} kompozisyona biraz daha çok katkı veriyor; ama {} risklerini tartmadan kilitleme.",
            a.champion_key, b.champion_key
        )
    } else {
        format!(
            "{} kompozisyona biraz daha çok katkı veriyor; ama {} risklerini tartmadan kilitleme.",
            b.champion_key, a.champion_key
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coach_quality::has_absolute_language;
    use crate::draft_simulator::{DamageType, SimChampion};

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

    #[test]
    fn fork_contrasts_engage_vs_scaling_and_stays_hedged() {
        let base = state(vec![
            champ(1, "Jinx", "marksman", DamageType::Ad),
            champ(2, "Lulu", "enchanter", DamageType::Ap),
        ]);
        let engage = mv(champ(3, "Leona", "vanguard", DamageType::Ap));
        let scaling = mv(champ(4, "Viktor", "control_mage", DamageType::Ap));
        let fork = compare_fork(&base, &engage, &scaling);

        // Engage pick should lift `engage`; the two picks lead to different reads.
        assert!(fork
            .option_a
            .improved_factors
            .contains(&"engage".to_string()));
        assert_ne!(fork.option_a.champion_id, fork.option_b.champion_id);
        for s in [
            &fork.plan_divergence,
            &fork.risk_divergence,
            &fork.recommendation,
        ] {
            assert!(!has_absolute_language(s), "hedged dil bekleniyor: {s}");
            assert!(!s.is_empty());
        }
    }

    #[test]
    fn fork_is_deterministic() {
        let base = state(vec![champ(1, "Ahri", "burst_mage", DamageType::Ap)]);
        let a = mv(champ(2, "Leona", "vanguard", DamageType::Ap));
        let b = mv(champ(3, "Jinx", "marksman", DamageType::Ad));
        assert_eq!(compare_fork(&base, &a, &b), compare_fork(&base, &a, &b));
    }

    #[test]
    fn close_options_yield_a_comfort_based_recommendation() {
        // Two similar engage tanks → small margin → comfort/role framing.
        let base = state(vec![champ(1, "Jinx", "marksman", DamageType::Ad)]);
        let a = mv(champ(2, "Leona", "vanguard", DamageType::Ap));
        let b = mv(champ(3, "Nautilus", "vanguard", DamageType::Ap));
        let fork = compare_fork(&base, &a, &b);
        assert!(
            fork.recommendation.contains("yakın") || fork.recommendation.contains("katkı"),
            "recommendation: {}",
            fork.recommendation
        );
        assert!(!has_absolute_language(&fork.recommendation));
    }

    #[test]
    fn shared_factors_lists_what_both_picks_improve() {
        // AP-heavy base: both AD candidates improve damage_balance.
        let base = state(vec![
            champ(1, "Ahri", "burst_mage", DamageType::Ap),
            champ(2, "Syndra", "control_mage", DamageType::Ap),
            champ(3, "Viktor", "control_mage", DamageType::Ap),
        ]);
        let a = mv(champ(4, "Graves", "marksman", DamageType::Ad));
        let b = mv(champ(5, "Darius", "juggernaut", DamageType::Ad));
        let fork = compare_fork(&base, &a, &b);
        assert!(
            fork.shared_factors.contains(&"damage_balance".to_string()),
            "both AD picks should share damage_balance: {:?}",
            fork.shared_factors
        );
    }

    #[test]
    fn diverging_factors_lists_disagreements() {
        // Base marksman (low peel): warden raises peel, assassin lowers it.
        let base = state(vec![champ(1, "Jinx", "marksman", DamageType::Ad)]);
        let warden = mv(champ(2, "Braum", "warden", DamageType::Ad));
        let assassin = mv(champ(3, "Zed", "assassin", DamageType::Ad));
        let fork = compare_fork(&base, &warden, &assassin);
        assert!(
            fork.diverging_factors.contains(&"peel".to_string()),
            "peel should diverge (warden up, assassin down): {:?}",
            fork.diverging_factors
        );
    }

    #[test]
    fn clear_margin_names_the_stronger_pick() {
        // Base lacks engage + frontline: vanguard contributes far more than an enchanter.
        let base = state(vec![
            champ(1, "Jinx", "marksman", DamageType::Ad),
            champ(2, "Ahri", "burst_mage", DamageType::Ap),
        ]);
        let strong = mv(champ(3, "Leona", "vanguard", DamageType::Ap));
        let weak = mv(champ(4, "Karma", "enchanter", DamageType::Ap));
        let fork = compare_fork(&base, &strong, &weak);
        assert!(
            fork.recommendation.contains("Leona"),
            "stronger pick should be named: {}",
            fork.recommendation
        );
        assert!(!has_absolute_language(&fork.recommendation));
    }

    #[test]
    fn fork_sentences_never_run_away() {
        let base = state(vec![champ(1, "Jinx", "marksman", DamageType::Ad)]);
        let a = mv(champ(2, "Leona", "vanguard", DamageType::Ap));
        let b = mv(champ(3, "Viktor", "control_mage", DamageType::Ap));
        let fork = compare_fork(&base, &a, &b);
        for s in [
            &fork.plan_divergence,
            &fork.risk_divergence,
            &fork.recommendation,
        ] {
            assert!(s.split_whitespace().count() <= 60, "runaway sentence: {s}");
        }
    }
}
