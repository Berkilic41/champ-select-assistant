//! Champion Pool Coach Quality (Sprint I, Claude) — coach_quality re-audit of pool
//! plans + a 28-scenario coverage matrix over the real `pool_coach` engine.
//! Decoupled from any command/UI; pure.
#![allow(dead_code)] // audit fn consumed by the matrix test + future QA tooling

use crate::coach_quality::{has_absolute_language, is_meaningful};
use crate::pool_coach::ChampionPoolPlan;
use std::collections::HashSet;

const MAX_SENTENCE_WORDS: usize = 60;

/// Lint a pool plan's prose + structure. Empty vec == clean.
pub fn audit_pool_plan(plan: &ChampionPoolPlan) -> Vec<String> {
    let mut issues = Vec::new();
    let mut sentences: Vec<(&str, &str)> = vec![("summary", plan.summary.as_str())];
    for g in &plan.gaps {
        sentences.push(("gap_note", g.note.as_str()));
    }
    for t in &plan.training {
        sentences.push(("training_reason", t.reason.as_str()));
        for d in &t.drills {
            sentences.push(("training_drill", d.as_str()));
        }
    }
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
    // Gap dimensions must be unique; training roles + champions must be unique.
    let dims: HashSet<&String> = plan.gaps.iter().map(|g| &g.dimension).collect();
    if dims.len() != plan.gaps.len() {
        issues.push("dup:gap_dimension".to_string());
    }
    let roles: HashSet<&String> = plan.training.iter().map(|t| &t.role_in_plan).collect();
    if roles.len() != plan.training.len() {
        issues.push("dup:training_role".to_string());
    }
    let champs: HashSet<u32> = plan.training.iter().map(|t| t.champion_id).collect();
    if champs.len() != plan.training.len() {
        issues.push("dup:training_champion".to_string());
    }
    if plan.training.len() > 3 {
        issues.push("too_many_training".to_string());
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool_coach::{analyze_pool, PoolChampion, PoolCoachInput};

    fn ch(id: u32, key: &str, arch: &str, blind: f32, exec: u8) -> PoolChampion {
        PoolChampion {
            champion_id: id,
            champion_key: key.into(),
            archetype: arch.into(),
            blind_safety: blind,
            execution_difficulty: exec,
            power_late: 0.5,
            engage: false,
            peel: false,
            comfort: 0.0,
            games: 0,
            meta_strength: 0.5,
        }
    }
    fn p(mut c: PoolChampion, comfort: f32, games: u32) -> PoolChampion {
        c.comfort = comfort;
        c.games = games;
        c
    }
    fn with(mut c: PoolChampion, engage: bool, peel: bool, late: f32) -> PoolChampion {
        c.engage = engage;
        c.peel = peel;
        c.power_late = late;
        c
    }

    #[derive(Clone)]
    enum Expect {
        Gap(&'static str),
        NoGap(&'static str),
        DataState(&'static str),
        TrainingRole(&'static str),
        ExecRisk(&'static str),
    }

    struct Scenario {
        name: &'static str,
        category: &'static str,
        input: PoolCoachInput,
        expects: Vec<Expect>,
    }

    fn inp(role: &str, pool: Vec<PoolChampion>, candidates: Vec<PoolChampion>) -> PoolCoachInput {
        PoolCoachInput {
            role: role.into(),
            pool,
            candidates,
        }
    }

    fn scenarios() -> Vec<Scenario> {
        let s = |name, category, input, expects: Vec<Expect>| Scenario {
            name,
            category,
            input,
            expects,
        };
        vec![
            // ── role coverage ────────────────────────────────────────────────────
            s(
                "mid-has-mage",
                "role",
                inp(
                    "middle",
                    vec![p(ch(1, "Lux", "control_mage", 0.7, 3), 0.6, 40)],
                    vec![],
                ),
                vec![Expect::NoGap("role_coverage")],
            ),
            s(
                "mid-only-marksman",
                "role",
                inp(
                    "middle",
                    vec![p(ch(1, "Jinx", "marksman", 0.6, 2), 0.6, 40)],
                    vec![],
                ),
                vec![Expect::Gap("role_coverage")],
            ),
            s(
                "top-has-juggernaut",
                "role",
                inp(
                    "top",
                    vec![p(ch(1, "Garen", "juggernaut", 0.7, 2), 0.7, 40)],
                    vec![],
                ),
                vec![Expect::NoGap("role_coverage")],
            ),
            s(
                "sup-has-enchanter",
                "role",
                inp(
                    "utility",
                    vec![p(ch(1, "Lulu", "enchanter", 0.7, 3), 0.6, 40)],
                    vec![],
                ),
                vec![Expect::NoGap("role_coverage")],
            ),
            // ── blind safe ───────────────────────────────────────────────────────
            s(
                "blind-safe-present",
                "blind_safe",
                inp(
                    "top",
                    vec![p(ch(1, "Garen", "juggernaut", 0.7, 2), 0.6, 40)],
                    vec![],
                ),
                vec![Expect::NoGap("blind_safe")],
            ),
            s(
                "blind-unsafe-only",
                "blind_safe",
                inp(
                    "middle",
                    vec![p(ch(1, "Zed", "assassin", 0.3, 4), 0.8, 60)],
                    vec![],
                ),
                vec![Expect::Gap("blind_safe")],
            ),
            s(
                "blind-unsafe-two",
                "blind_safe",
                inp(
                    "middle",
                    vec![
                        p(ch(1, "Zed", "assassin", 0.3, 4), 0.7, 40),
                        p(ch(2, "Yasuo", "skirmisher", 0.4, 5), 0.6, 30),
                    ],
                    vec![],
                ),
                vec![Expect::Gap("blind_safe")],
            ),
            s(
                "blind-mixed-ok",
                "blind_safe",
                inp(
                    "middle",
                    vec![
                        p(ch(1, "Zed", "assassin", 0.3, 4), 0.7, 40),
                        p(ch(2, "Viktor", "control_mage", 0.7, 3), 0.6, 30),
                    ],
                    vec![],
                ),
                vec![Expect::NoGap("blind_safe")],
            ),
            // ── counter / flex ───────────────────────────────────────────────────
            s(
                "one-archetype",
                "counter_flex",
                inp(
                    "middle",
                    vec![
                        p(ch(1, "Lux", "control_mage", 0.7, 3), 0.6, 40),
                        p(ch(2, "Syndra", "control_mage", 0.6, 4), 0.5, 30),
                    ],
                    vec![],
                ),
                vec![Expect::Gap("counter_flex")],
            ),
            s(
                "two-archetypes",
                "counter_flex",
                inp(
                    "middle",
                    vec![
                        p(ch(1, "Lux", "control_mage", 0.7, 3), 0.6, 40),
                        p(ch(2, "Zed", "assassin", 0.3, 4), 0.5, 30),
                    ],
                    vec![],
                ),
                vec![Expect::NoGap("counter_flex")],
            ),
            // ── comfort ──────────────────────────────────────────────────────────
            s(
                "rich-no-comfort",
                "comfort",
                inp(
                    "middle",
                    vec![p(ch(1, "Lux", "control_mage", 0.7, 3), 0.0, 5)],
                    vec![],
                ),
                vec![Expect::Gap("comfort"), Expect::DataState("rich")],
            ),
            s(
                "rich-with-comfort",
                "comfort",
                inp(
                    "middle",
                    vec![p(ch(1, "Lux", "control_mage", 0.7, 3), 0.8, 80)],
                    vec![],
                ),
                vec![Expect::NoGap("comfort"), Expect::DataState("rich")],
            ),
            s(
                "thin-skips-comfort",
                "comfort",
                inp("middle", vec![ch(1, "Lux", "control_mage", 0.7, 3)], vec![]),
                vec![Expect::NoGap("comfort"), Expect::DataState("thin")],
            ),
            // ── execution risk ───────────────────────────────────────────────────
            s(
                "high-exec-pool",
                "execution",
                inp(
                    "middle",
                    vec![
                        p(ch(1, "Zed", "assassin", 0.3, 5), 0.6, 40),
                        p(ch(2, "Yasuo", "skirmisher", 0.4, 5), 0.5, 30),
                    ],
                    vec![],
                ),
                vec![Expect::ExecRisk("high"), Expect::Gap("execution_risk")],
            ),
            s(
                "low-exec-pool",
                "execution",
                inp(
                    "top",
                    vec![
                        p(ch(1, "Garen", "juggernaut", 0.7, 2), 0.6, 40),
                        p(ch(2, "Malphite", "vanguard", 0.8, 2), 0.5, 30),
                    ],
                    vec![],
                ),
                vec![Expect::ExecRisk("low"), Expect::NoGap("execution_risk")],
            ),
            s(
                "medium-exec-pool",
                "execution",
                inp(
                    "middle",
                    vec![
                        p(ch(1, "Lux", "control_mage", 0.7, 3), 0.6, 40),
                        p(ch(2, "Ahri", "burst_mage", 0.6, 3), 0.5, 30),
                    ],
                    vec![],
                ),
                vec![Expect::ExecRisk("medium")],
            ),
            // ── identity variety ─────────────────────────────────────────────────
            s(
                "identity-rich",
                "identity",
                inp(
                    "top",
                    vec![
                        p(
                            with(ch(1, "Sion", "vanguard", 0.8, 2), true, false, 0.4),
                            0.6,
                            40,
                        ),
                        p(
                            with(ch(2, "Shen", "warden", 0.7, 2), false, true, 0.5),
                            0.5,
                            30,
                        ),
                    ],
                    vec![],
                ),
                vec![Expect::NoGap("identity_variety")],
            ),
            s(
                "identity-poor",
                "identity",
                inp(
                    "middle",
                    vec![p(
                        with(ch(1, "Lux", "control_mage", 0.7, 3), false, false, 0.5),
                        0.6,
                        40,
                    )],
                    vec![],
                ),
                vec![Expect::Gap("identity_variety")],
            ),
            // ── thin data ────────────────────────────────────────────────────────
            s(
                "empty-thin",
                "thin",
                inp("middle", vec![], vec![]),
                vec![Expect::DataState("thin")],
            ),
            s(
                "nogames-thin",
                "thin",
                inp("top", vec![ch(1, "Garen", "juggernaut", 0.7, 2)], vec![]),
                vec![Expect::DataState("thin")],
            ),
            s(
                "hasgames-rich",
                "thin",
                inp(
                    "top",
                    vec![p(ch(1, "Garen", "juggernaut", 0.7, 2), 0.6, 40)],
                    vec![],
                ),
                vec![Expect::DataState("rich")],
            ),
            // ── training plan ────────────────────────────────────────────────────
            s(
                "plan-blind-safe",
                "training",
                inp(
                    "middle",
                    vec![p(ch(9, "Lux", "control_mage", 0.7, 3), 0.6, 40)],
                    vec![ch(1, "Malphite", "vanguard", 0.85, 2)],
                ),
                vec![Expect::TrainingRole("blind_safe")],
            ),
            s(
                "plan-counter",
                "training",
                inp(
                    "middle",
                    vec![p(ch(9, "Lux", "control_mage", 0.7, 3), 0.6, 40)],
                    vec![ch(2, "Zed", "assassin", 0.3, 4)],
                ),
                vec![Expect::TrainingRole("counter_pick")],
            ),
            s(
                "plan-meta-pick",
                "training",
                inp(
                    "middle",
                    vec![p(ch(9, "Lux", "control_mage", 0.7, 3), 0.6, 40)],
                    // Two blind-safe candidates → the stronger anchors blind_safe, the
                    // other rounds out the plan as the meta/easy entry.
                    vec![
                        ch(1, "Malphite", "vanguard", 0.85, 2),
                        ch(3, "Annie", "burst_mage", 0.6, 1),
                    ],
                ),
                vec![Expect::TrainingRole("meta_pick")],
            ),
            s(
                "plan-full-three",
                "training",
                inp(
                    "middle",
                    vec![p(ch(9, "Lux", "control_mage", 0.7, 3), 0.6, 40)],
                    vec![
                        ch(1, "Malphite", "vanguard", 0.85, 2),
                        ch(2, "Zed", "assassin", 0.3, 4),
                        p(ch(3, "Ahri", "burst_mage", 0.5, 3), 0.4, 20),
                    ],
                ),
                vec![
                    Expect::TrainingRole("blind_safe"),
                    Expect::TrainingRole("counter_pick"),
                    Expect::TrainingRole("meta_pick"),
                ],
            ),
            s(
                "plan-empty-candidates",
                "training",
                inp(
                    "middle",
                    vec![p(ch(9, "Lux", "control_mage", 0.7, 3), 0.6, 40)],
                    vec![],
                ),
                vec![],
            ),
            // ── well-rounded (no gaps) ───────────────────────────────────────────
            s(
                "balanced-pool",
                "balanced",
                inp(
                    "top",
                    vec![
                        p(
                            with(ch(1, "Garen", "juggernaut", 0.7, 2), false, false, 0.5),
                            0.8,
                            80,
                        ),
                        p(
                            with(ch(2, "Malphite", "vanguard", 0.8, 2), true, false, 0.4),
                            0.6,
                            40,
                        ),
                        p(
                            with(ch(3, "Shen", "warden", 0.75, 3), false, true, 0.5),
                            0.5,
                            30,
                        ),
                    ],
                    vec![],
                ),
                vec![
                    Expect::NoGap("blind_safe"),
                    Expect::NoGap("counter_flex"),
                    Expect::NoGap("identity_variety"),
                ],
            ),
            s(
                "balanced-mid",
                "balanced",
                inp(
                    "middle",
                    vec![
                        p(ch(1, "Viktor", "control_mage", 0.7, 3), 0.8, 80),
                        p(ch(2, "Zed", "assassin", 0.3, 4), 0.6, 40),
                    ],
                    vec![],
                ),
                vec![
                    Expect::NoGap("blind_safe"),
                    Expect::NoGap("counter_flex"),
                    Expect::NoGap("comfort"),
                ],
            ),
        ]
    }

    #[test]
    fn pool_quality_matrix_holds_and_prose_passes_audit() {
        let scenarios = scenarios();
        assert!(scenarios.len() >= 25, "need a 25+ scenario matrix");

        use std::collections::BTreeMap;
        let mut per_category: BTreeMap<&str, (u32, u32)> = BTreeMap::new();

        for sc in &scenarios {
            let plan = analyze_pool(&sc.input);

            let issues = audit_pool_plan(&plan);
            assert!(
                issues.is_empty(),
                "[{}] prose audit failed: {issues:?}",
                sc.name
            );

            let entry = per_category.entry(sc.category).or_insert((0, 0));
            entry.1 += 1;
            for ex in &sc.expects {
                let ok = match ex {
                    Expect::Gap(d) => plan.gaps.iter().any(|g| g.dimension == *d),
                    Expect::NoGap(d) => !plan.gaps.iter().any(|g| g.dimension == *d),
                    Expect::DataState(st) => plan.data_state == *st,
                    Expect::TrainingRole(r) => plan.training.iter().any(|t| t.role_in_plan == *r),
                    Expect::ExecRisk(l) => plan.coverage.execution_risk == *l,
                };
                assert!(ok, "[{}] expectation failed", sc.name);
            }
            entry.0 += 1;
        }

        eprintln!(
            "\n=== Champion Pool Coach quality matrix ({} scenarios) ===",
            scenarios.len()
        );
        for (cat, (pass, total)) in &per_category {
            eprintln!("  {cat:>14}: {pass}/{total} held");
        }
        eprintln!("=========================================================\n");
    }

    #[test]
    fn audit_catches_injected_absolute_language() {
        let mut plan = analyze_pool(&inp(
            "middle",
            vec![p(ch(1, "Lux", "control_mage", 0.7, 3), 0.6, 40)],
            vec![],
        ));
        plan.summary = "Bu havuzla kesin kazanırsın".to_string();
        assert!(audit_pool_plan(&plan)
            .iter()
            .any(|i| i == "absolute:summary"));
    }

    /// Cross-language drift guard: every machine token the engine can emit
    /// (data_state, execution_risk, gap dimension/severity, training role_in_plan)
    /// must have a `poolCoach.*` label in tr.json — otherwise the panel renders a
    /// raw key. en.json parity is covered by the TS `i18n tr/en parity` test.
    /// Driving all 28 matrix scenarios surfaces every token.
    #[test]
    fn every_emitted_pool_token_has_an_i18n_label() {
        const TR: &str = include_str!("../../src/i18n/tr.json");
        let tr: serde_json::Value = serde_json::from_str(TR).expect("tr.json parses");
        let pc = &tr["poolCoach"];

        for sc in scenarios() {
            let plan = analyze_pool(&sc.input);
            assert!(
                !pc["dataState"][&plan.data_state].is_null(),
                "data_state '{}' has no poolCoach.dataState label",
                plan.data_state
            );
            assert!(
                !pc["executionRisk"][&plan.coverage.execution_risk].is_null(),
                "execution_risk '{}' has no poolCoach.executionRisk label",
                plan.coverage.execution_risk
            );
            for g in &plan.gaps {
                assert!(
                    !pc["gap"][&g.dimension].is_null(),
                    "gap dimension '{}' has no poolCoach.gap label",
                    g.dimension
                );
                assert!(
                    !pc["severity"][&g.severity].is_null(),
                    "severity '{}' has no poolCoach.severity label",
                    g.severity
                );
            }
            for t in &plan.training {
                assert!(
                    !pc["training"][&t.role_in_plan].is_null(),
                    "role_in_plan '{}' has no poolCoach.training label",
                    t.role_in_plan
                );
            }
        }
    }
}
