//! Draft Simulator Quality v2 (Sprint G, Claude) — a measured quality matrix +
//! a coach_quality re-audit of every simulator sentence.
//!
//! Two parts, both decoupled from the Codex-owned command/UI:
//!   1. `audit_sim_result` — pure re-audit of a `DraftSimResult`'s prose using the
//!      same `coach_quality` primitives the rest of the project uses (no
//!      over-promising, meaningful, not a runaway concatenation, deduped factors).
//!   2. A 28-scenario matrix (test module) over the REAL `draft_simulator` engine
//!      that asserts the expected quality property per scenario AND that every
//!      sentence passes the re-audit. Coverage numbers go to the audit doc.
#![allow(dead_code)] // audit fn consumed by the matrix test + future QA tooling

use crate::coach_quality::{has_absolute_language, is_meaningful};
use crate::draft_simulator::DraftSimResult;
use std::collections::HashSet;

/// Runaway-concatenation guard for any single simulator sentence.
const MAX_SENTENCE_WORDS: usize = 60;

/// Lint a simulator result's prose + factor lists. Empty vec == clean.
pub fn audit_sim_result(r: &DraftSimResult) -> Vec<String> {
    let mut issues = Vec::new();
    let sentences = [
        ("coach_sentence", r.coach_sentence.as_str()),
        ("why_this_move", r.why_this_move.as_str()),
        ("why_not_alternative", r.why_not_alternative.as_str()),
        ("risk_summary", r.risk.summary.as_str()),
        ("plan_note", r.plan_shift.note.as_str()),
    ];
    for (name, s) in sentences {
        if has_absolute_language(s) {
            issues.push(format!("absolute:{name}"));
        }
        if !is_meaningful(s, 2) {
            issues.push(format!("empty:{name}"));
        }
        if s.split_whitespace().count() > MAX_SENTENCE_WORDS {
            issues.push(format!("runaway:{name}"));
        }
    }
    // Factor lists must be deduped and a factor must not be both improved + worsened.
    let imp: HashSet<&String> = r.improved_factors.iter().collect();
    let wor: HashSet<&String> = r.worsened_factors.iter().collect();
    if imp.len() != r.improved_factors.len() {
        issues.push("dup:improved_factors".to_string());
    }
    if wor.len() != r.worsened_factors.len() {
        issues.push("dup:worsened_factors".to_string());
    }
    if imp.intersection(&wor).next().is_some() {
        issues.push("conflict:improved_and_worsened".to_string());
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft_simulator::{
        compare_moves, DamageType, DraftSimMove, DraftSimState, SimChampion,
    };

    fn champ(id: u32, key: &str, archetype: &str, dmg: DamageType) -> SimChampion {
        SimChampion {
            champion_id: id,
            champion_key: key.into(),
            archetype: archetype.into(),
            damage: dmg,
            combo_partner_ids: vec![],
        }
    }

    fn combo(
        id: u32,
        key: &str,
        archetype: &str,
        dmg: DamageType,
        partners: Vec<u32>,
    ) -> SimChampion {
        SimChampion {
            champion_id: id,
            champion_key: key.into(),
            archetype: archetype.into(),
            damage: dmg,
            combo_partner_ids: partners,
        }
    }

    fn st(team: Vec<SimChampion>) -> DraftSimState {
        DraftSimState {
            my_team: team,
            ..Default::default()
        }
    }

    fn blind_first(team: Vec<SimChampion>) -> DraftSimState {
        DraftSimState {
            my_team: team,
            enemy_team: vec![],
            blind: true,
            first_pick: true,
        }
    }

    fn mv(c: SimChampion) -> DraftSimMove {
        DraftSimMove {
            champion: c,
            position: None,
        }
    }

    #[derive(Clone, Copy)]
    enum Expect {
        Improved(&'static str),
        Worsened(&'static str),
        RiskLevel(&'static str),
        RiskFactor(&'static str),
    }

    struct Scenario {
        name: &'static str,
        category: &'static str,
        state: DraftSimState,
        mv: DraftSimMove,
        expects: Vec<Expect>,
    }

    fn ap(team_size: u32) -> Vec<SimChampion> {
        (0..team_size)
            .map(|i| champ(i + 1, "APx", "burst_mage", DamageType::Ap))
            .collect()
    }
    fn ad(team_size: u32) -> Vec<SimChampion> {
        (0..team_size)
            .map(|i| champ(i + 1, "ADx", "marksman", DamageType::Ad))
            .collect()
    }

    /// 28 scenarios across the seven quality properties.
    fn scenarios() -> Vec<Scenario> {
        let s = |name, category, state, mv, expects: &[Expect]| Scenario {
            name,
            category,
            state,
            mv,
            expects: expects.to_vec(),
        };
        vec![
            // ── mono-damage / damage balance ─────────────────────────────────────
            s(
                "ap4+ap",
                "damage_balance",
                st(ap(4)),
                mv(champ(9, "Brand", "burst_mage", DamageType::Ap)),
                &[Expect::Worsened("damage_balance")],
            ),
            s(
                "ad4+ad",
                "damage_balance",
                st(ad(4)),
                mv(champ(9, "Cait", "marksman", DamageType::Ad)),
                &[Expect::Worsened("damage_balance")],
            ),
            s(
                "ap3+ad",
                "damage_balance",
                st(ap(3)),
                mv(champ(9, "Graves", "marksman", DamageType::Ad)),
                &[Expect::Improved("damage_balance")],
            ),
            s(
                "ad3+ap",
                "damage_balance",
                st(ad(3)),
                mv(champ(9, "Syndra", "control_mage", DamageType::Ap)),
                &[Expect::Improved("damage_balance")],
            ),
            // ── engage gap fill ──────────────────────────────────────────────────
            s(
                "noengage+vanguard",
                "engage",
                st(vec![
                    champ(1, "Jinx", "marksman", DamageType::Ad),
                    champ(2, "Lulu", "enchanter", DamageType::Ap),
                ]),
                mv(champ(9, "Leona", "vanguard", DamageType::Ap)),
                &[Expect::Improved("engage")],
            ),
            s(
                "noengage+diver",
                "engage",
                st(vec![
                    champ(1, "Ezreal", "marksman", DamageType::Ad),
                    champ(2, "Karma", "enchanter", DamageType::Ap),
                ]),
                mv(champ(9, "Vi", "diver", DamageType::Ad)),
                &[Expect::Improved("engage")],
            ),
            s(
                "noengage+catcher",
                "engage",
                st(vec![champ(1, "Jinx", "marksman", DamageType::Ad)]),
                mv(champ(9, "Thresh", "catcher", DamageType::Ap)),
                &[Expect::Improved("engage")],
            ),
            s(
                "engageheavy+enchanter",
                "engage",
                st(vec![
                    champ(1, "Leona", "vanguard", DamageType::Ap),
                    champ(2, "Vi", "diver", DamageType::Ad),
                ]),
                mv(champ(9, "Janna", "enchanter", DamageType::Ap)),
                &[Expect::Worsened("engage")],
            ),
            // ── peel / frontline need ────────────────────────────────────────────
            s(
                "squishy+warden",
                "peel_frontline",
                st(vec![
                    champ(1, "Jinx", "marksman", DamageType::Ad),
                    champ(2, "Zed", "assassin", DamageType::Ad),
                ]),
                mv(champ(9, "Braum", "warden", DamageType::Ad)),
                &[Expect::Improved("peel"), Expect::Improved("frontline")],
            ),
            s(
                "squishy+enchanter-peel",
                "peel_frontline",
                st(vec![champ(1, "Jinx", "marksman", DamageType::Ad)]),
                mv(champ(9, "Lulu", "enchanter", DamageType::Ap)),
                &[Expect::Improved("peel")],
            ),
            s(
                "nofrontline+vanguard",
                "peel_frontline",
                st(vec![
                    champ(1, "Ahri", "burst_mage", DamageType::Ap),
                    champ(2, "Jinx", "marksman", DamageType::Ad),
                ]),
                mv(champ(9, "Malphite", "vanguard", DamageType::Ap)),
                &[Expect::Improved("frontline")],
            ),
            s(
                "nofrontline+juggernaut",
                "peel_frontline",
                st(vec![champ(1, "Lux", "burst_mage", DamageType::Ap)]),
                mv(champ(9, "Darius", "juggernaut", DamageType::Ad)),
                &[Expect::Improved("frontline")],
            ),
            // ── blind first-pick risk ────────────────────────────────────────────
            s(
                "blind+assassin",
                "blind_risk",
                blind_first(vec![]),
                mv(champ(9, "Zed", "assassin", DamageType::Ad)),
                &[
                    Expect::RiskLevel("high"),
                    Expect::RiskFactor("blind_safety"),
                ],
            ),
            s(
                "blind+skirmisher",
                "blind_risk",
                blind_first(vec![]),
                mv(champ(9, "Yasuo", "skirmisher", DamageType::Ad)),
                &[Expect::RiskFactor("blind_safety")],
            ),
            s(
                "blind+warden-safe",
                "blind_risk",
                blind_first(vec![]),
                mv(champ(9, "Braum", "warden", DamageType::Ad)),
                &[Expect::RiskLevel("low")],
            ),
            s(
                "nonblind+assassin-ok",
                "blind_risk",
                st(vec![]),
                mv(champ(9, "Zed", "assassin", DamageType::Ad)),
                &[Expect::RiskLevel("high")],
            ),
            // ── greedy scaling risk ──────────────────────────────────────────────
            s(
                "scaling+scaling",
                "greedy_scaling",
                st(vec![
                    champ(1, "Jinx", "marksman", DamageType::Ad),
                    champ(2, "Viktor", "control_mage", DamageType::Ap),
                ]),
                mv(champ(9, "Kayle", "marksman", DamageType::Mixed)),
                &[Expect::Worsened("execution_risk")],
            ),
            s(
                "scaling+artillery",
                "greedy_scaling",
                st(vec![champ(1, "Kassadin", "skirmisher", DamageType::Ap)]),
                mv(champ(9, "Xerath", "artillery", DamageType::Ap)),
                &[Expect::RiskLevel("medium")],
            ),
            s(
                "earlycomp+earlypick",
                "greedy_scaling",
                st(vec![champ(1, "Renekton", "juggernaut", DamageType::Ad)]),
                mv(champ(9, "Pantheon", "diver", DamageType::Ad)),
                &[Expect::Improved("lane_pressure")],
            ),
            s(
                "scaling+vanguard-tempo",
                "greedy_scaling",
                st(vec![
                    champ(1, "Jinx", "marksman", DamageType::Ad),
                    champ(2, "Viktor", "control_mage", DamageType::Ap),
                ]),
                mv(champ(9, "Leona", "vanguard", DamageType::Ap)),
                &[Expect::Improved("engage")],
            ),
            // ── combo synergy (does not hide risk) ───────────────────────────────
            s(
                "combo+execrisk",
                "combo_synergy",
                st(vec![combo(
                    1,
                    "Orianna",
                    "control_mage",
                    DamageType::Ap,
                    vec![9],
                )]),
                mv(combo(9, "Yasuo", "skirmisher", DamageType::Ad, vec![1])),
                &[
                    Expect::Improved("synergy"),
                    Expect::RiskFactor("execution_risk"),
                ],
            ),
            s(
                "combo+vanguard",
                "combo_synergy",
                st(vec![combo(
                    1,
                    "Miss Fortune",
                    "marksman",
                    DamageType::Ad,
                    vec![9],
                )]),
                mv(combo(9, "Malphite", "vanguard", DamageType::Ap, vec![1])),
                &[Expect::Improved("synergy"), Expect::Improved("engage")],
            ),
            s(
                "combo+catcher",
                "combo_synergy",
                st(vec![combo(
                    1,
                    "Yasuo",
                    "skirmisher",
                    DamageType::Ad,
                    vec![9],
                )]),
                mv(combo(9, "Malphite", "vanguard", DamageType::Ap, vec![1])),
                &[Expect::Improved("synergy")],
            ),
            s(
                "nocombo+solo",
                "combo_synergy",
                st(vec![champ(1, "Ahri", "burst_mage", DamageType::Ap)]),
                mv(champ(9, "Zed", "assassin", DamageType::Ad)),
                &[Expect::Improved("damage_balance")],
            ),
            // ── objective / lane identity ────────────────────────────────────────
            s(
                "nopoke+artillery",
                "identity",
                st(vec![champ(1, "Zed", "assassin", DamageType::Ad)]),
                mv(champ(9, "Ziggs", "artillery", DamageType::Ap)),
                &[Expect::Improved("objective_identity")],
            ),
            s(
                "nolane+lanebully",
                "identity",
                st(vec![champ(1, "Kassadin", "skirmisher", DamageType::Ap)]),
                mv(champ(9, "Renekton", "juggernaut", DamageType::Ad)),
                &[Expect::Improved("frontline")],
            ),
            s(
                "control+disengage",
                "identity",
                st(vec![champ(1, "Jinx", "marksman", DamageType::Ad)]),
                mv(champ(9, "Janna", "enchanter", DamageType::Ap)),
                &[Expect::Improved("disengage")],
            ),
            s(
                "teamfight+vanguard",
                "identity",
                st(vec![
                    champ(1, "Orianna", "control_mage", DamageType::Ap),
                    champ(2, "Jinx", "marksman", DamageType::Ad),
                ]),
                mv(champ(9, "Sejuani", "vanguard", DamageType::Ad)),
                &[Expect::Improved("frontline")],
            ),
        ]
    }

    #[test]
    fn quality_matrix_holds_and_sentences_pass_audit() {
        let scenarios = scenarios();
        assert!(scenarios.len() >= 25, "need a 25-30 scenario matrix");

        use std::collections::BTreeMap;
        let mut per_category: BTreeMap<&str, (u32, u32)> = BTreeMap::new();

        for sc in &scenarios {
            let result = compare_moves(&sc.state, std::slice::from_ref(&sc.mv))
                .into_iter()
                .next()
                .expect("one move → one result");

            // Sentence + factor re-audit (coach_quality) for EVERY scenario.
            let issues = audit_sim_result(&result);
            if !issues.is_empty() {
                panic!("[{}] sentence audit failed: {issues:?}", sc.name);
            }

            // Expected quality property holds.
            let entry = per_category.entry(sc.category).or_insert((0, 0));
            entry.1 += 1;
            for ex in &sc.expects {
                let ok = match ex {
                    Expect::Improved(f) => result.improved_factors.iter().any(|x| x == f),
                    Expect::Worsened(f) => result.worsened_factors.iter().any(|x| x == f),
                    Expect::RiskLevel(l) => result.risk.level == *l,
                    Expect::RiskFactor(f) => result.risk.factors.iter().any(|x| x == f),
                };
                assert!(ok, "[{}] expectation failed: {:?}", sc.name, describe(ex));
            }
            entry.0 += 1;
        }

        eprintln!(
            "\n=== Draft Simulator quality matrix ({} scenarios) ===",
            scenarios.len()
        );
        for (cat, (pass, total)) in &per_category {
            eprintln!("  {cat:>16}: {pass}/{total} held");
        }
        eprintln!("=====================================================\n");
    }

    fn describe(ex: &Expect) -> String {
        match ex {
            Expect::Improved(f) => format!("improved {f}"),
            Expect::Worsened(f) => format!("worsened {f}"),
            Expect::RiskLevel(l) => format!("risk level {l}"),
            Expect::RiskFactor(f) => format!("risk factor {f}"),
        }
    }

    /// i18n drift guard: every factor key the engine can emit (deltas) and every
    /// risk level must have a `draftSimulator.factor.*` / `riskLevel.*` label in
    /// tr.json — otherwise the panel would render a raw machine key. en.json parity
    /// is covered by the TS `i18n tr/en parity` test (356/356).
    #[test]
    fn every_emitted_factor_and_risk_level_has_an_i18n_label() {
        const TR: &str = include_str!("../../src/i18n/tr.json");
        let tr: serde_json::Value = serde_json::from_str(TR).expect("tr.json parses");
        let factor_labels = &tr["draftSimulator"]["factor"];
        let risk_labels = &tr["draftSimulator"]["riskLevel"];

        // Drive the engine over a few comps so every delta factor key appears.
        let states = [
            st(vec![champ(1, "Jinx", "marksman", DamageType::Ad)]),
            st(ap(4)),
            blind_first(vec![]),
        ];
        let move_ = mv(champ(9, "Leona", "vanguard", DamageType::Ap));
        for state in &states {
            let r = compare_moves(state, std::slice::from_ref(&move_))
                .into_iter()
                .next()
                .unwrap();
            for d in &r.deltas {
                assert!(
                    !factor_labels[&d.factor].is_null(),
                    "factor '{}' has no draftSimulator.factor i18n label",
                    d.factor
                );
            }
            assert!(
                !risk_labels[&r.risk.level].is_null(),
                "risk level '{}' has no draftSimulator.riskLevel i18n label",
                r.risk.level
            );
        }
    }

    #[test]
    fn audit_flags_injected_absolute_language() {
        // Sanity: the auditor actually catches over-promising prose.
        let mut bad = compare_moves(
            &st(vec![champ(1, "Jinx", "marksman", DamageType::Ad)]),
            std::slice::from_ref(&mv(champ(9, "Leona", "vanguard", DamageType::Ap))),
        )
        .remove(0);
        bad.coach_sentence = "Bu pick oyunu kesin kazandırır".to_string();
        let issues = audit_sim_result(&bad);
        assert!(issues.iter().any(|i| i == "absolute:coach_sentence"));
    }
}
