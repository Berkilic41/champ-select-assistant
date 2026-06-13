//! Draft Brain explanation-quality rubric + coverage audit (Sprint A, Claude).
//!
//! Two things live here, both decoupled from the Codex-owned sentence builders:
//!   1. A **rubric** over the explanation-bearing fields of a `Recommendation`
//!      — the pillars the Draft Brain promises (NEDEN / LANE / TEAMFIGHT /
//!      MID-GAME / FALLBACK / RİSK / WHY-NOT / DATA-CONFIDENCE). Pure scoring,
//!      reusing the `coach_quality` primitives.
//!   2. A **20-scenario coverage matrix** (in the test module) exercised against
//!      the REAL engine + `upgrade_*` pipeline, so the audit numbers are measured
//!      rather than assumed. Read-only use of engine APIs — no builder is edited.
//!
//! The hard guardrail (no over-promising language, meaningful decision sentence)
//! is asserted for every scenario; the soft completeness % is reported for the
//! audit doc (`docs/audit/draftbrain-explanation-quality.md`).
#![allow(dead_code)] // rubric is consumed by the matrix test + future QA tooling

use crate::coach_quality::{dedup_sentences, has_absolute_language, is_meaningful};
use crate::models::Recommendation;

/// Minimum words for each explanation field to count as "real content".
const DECISION_MIN_WORDS: usize = 4;
const PLAN_MIN_WORDS: usize = 3;
const WHY_NOT_MIN_WORDS: usize = 2;

/// The explanation pillars the Draft Brain claims to deliver. Coverage of these
/// is the rubric; a missing/thin/over-promising pillar is a gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pillar {
    /// "NEDEN" — the headline decision sentence.
    DecisionSentence,
    /// "LANE planı".
    LanePlan,
    /// "TEAMFIGHT planı" (`teamfight_job` heuristic or DI `teamfight_plan`).
    TeamfightPlan,
    /// Mid-game macro line.
    MidGamePlan,
    /// Plan B when the pick is behind.
    FallbackPlan,
    /// "KAYBETME riski" — `risk_summary` surfaced.
    RiskSurfaced,
    /// Comparative "why not the alternatives".
    WhyNot,
    /// A data provenance / confidence signal is attached.
    DataConfidence,
}

pub const PILLARS: [Pillar; 8] = [
    Pillar::DecisionSentence,
    Pillar::LanePlan,
    Pillar::TeamfightPlan,
    Pillar::MidGamePlan,
    Pillar::FallbackPlan,
    Pillar::RiskSurfaced,
    Pillar::WhyNot,
    Pillar::DataConfidence,
];

/// One gap finding for a pillar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplanationGap {
    /// Pillar absent / empty.
    Missing(Pillar),
    /// Present but below the meaningfulness bar (bare label).
    Thin(Pillar),
    /// Over-promising / guaranteed-outcome language.
    Absolute(Pillar),
}

impl ExplanationGap {
    /// Hard gaps fail the regression bar; soft gaps are reported only.
    /// Over-promising language is always hard; a missing decision sentence is
    /// hard (it's the headline). Everything else is a soft completeness gap.
    pub fn is_hard(&self) -> bool {
        matches!(
            self,
            ExplanationGap::Absolute(_) | ExplanationGap::Missing(Pillar::DecisionSentence)
        )
    }
}

/// Per-recommendation rubric result.
#[derive(Debug, Clone)]
pub struct ExplanationReport {
    pub covered: u8,
    pub total: u8,
    pub gaps: Vec<ExplanationGap>,
}

impl ExplanationReport {
    pub fn coverage_pct(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        f32::from(self.covered) / f32::from(self.total)
    }

    /// True when no HARD gap is present (over-promising or missing headline).
    pub fn passes_hard_bar(&self) -> bool {
        !self.gaps.iter().any(ExplanationGap::is_hard)
    }

    pub fn hard_gaps(&self) -> Vec<&ExplanationGap> {
        self.gaps.iter().filter(|g| g.is_hard()).collect()
    }
}

/// Classify one optional plan field into covered / a gap for `pillar`.
fn rate_optional(
    pillar: Pillar,
    value: Option<&str>,
    min_words: usize,
) -> Result<(), ExplanationGap> {
    match value {
        None => Err(ExplanationGap::Missing(pillar)),
        Some(text) if has_absolute_language(text) => Err(ExplanationGap::Absolute(pillar)),
        Some(text) if !is_meaningful(text, min_words) => Err(ExplanationGap::Thin(pillar)),
        Some(_) => Ok(()),
    }
}

/// Score one recommendation against the rubric. Pure; reads only explanation
/// fields. The teamfight pillar accepts either the DI `teamfight_plan` or the
/// heuristic `teamfight_job` — whichever is populated.
pub fn audit_recommendation(rec: &Recommendation) -> ExplanationReport {
    let mut gaps = Vec::new();

    // NEDEN — the headline.
    if !is_meaningful(&rec.decision_sentence, DECISION_MIN_WORDS) {
        gaps.push(ExplanationGap::Missing(Pillar::DecisionSentence));
    } else if has_absolute_language(&rec.decision_sentence) {
        gaps.push(ExplanationGap::Absolute(Pillar::DecisionSentence));
    }

    for (pillar, value) in [
        (Pillar::LanePlan, rec.lane_plan.as_deref()),
        (Pillar::MidGamePlan, rec.mid_game_plan.as_deref()),
        (Pillar::FallbackPlan, rec.fallback_plan.as_deref()),
        (Pillar::RiskSurfaced, rec.risk_summary.as_deref()),
    ] {
        if let Err(gap) = rate_optional(pillar, value, PLAN_MIN_WORDS) {
            gaps.push(gap);
        }
    }

    // Teamfight: DI plan first, fall back to the heuristic job line.
    let teamfight = rec
        .teamfight_plan
        .as_deref()
        .or(rec.teamfight_job.as_deref());
    if let Err(gap) = rate_optional(Pillar::TeamfightPlan, teamfight, PLAN_MIN_WORDS) {
        gaps.push(gap);
    }

    // WHY-NOT — non-empty, each entry meaningful, no dupes, no over-promising.
    if rec.why_not.is_empty() {
        gaps.push(ExplanationGap::Missing(Pillar::WhyNot));
    } else if rec.why_not.iter().any(|w| has_absolute_language(w)) {
        gaps.push(ExplanationGap::Absolute(Pillar::WhyNot));
    } else if rec
        .why_not
        .iter()
        .any(|w| !is_meaningful(w, WHY_NOT_MIN_WORDS))
        || rec.why_not.len() != dedup_sentences(&rec.why_not).len()
    {
        gaps.push(ExplanationGap::Thin(Pillar::WhyNot));
    }

    // DATA-CONFIDENCE — a provenance badge or a known confidence token.
    let known = |c: &str| matches!(c, "high" | "medium" | "low" | "heuristic");
    let has_confidence = !rec.data_sources.is_empty()
        || known(&rec.matchup_confidence)
        || known(&rec.build_confidence)
        || known(&rec.confidence);
    if !has_confidence {
        gaps.push(ExplanationGap::Missing(Pillar::DataConfidence));
    }

    let flawed: std::collections::HashSet<Pillar> = gaps
        .iter()
        .map(|g| match g {
            ExplanationGap::Missing(p) | ExplanationGap::Thin(p) | ExplanationGap::Absolute(p) => {
                *p
            }
        })
        .collect();
    let covered = PILLARS.len() as u8 - flawed.len() as u8;

    ExplanationReport {
        covered,
        total: PILLARS.len() as u8,
        gaps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft_brain::{
        local_rules_model_pack, local_seed_data_pack, upgrade_recommendations_with_context,
    };
    use crate::draft_iq::DraftKnowledgeBase;
    use crate::engine::compute_recommendations;
    use crate::scoring::{MetaRate, ScoringContext, ScoringWeights};
    use crate::types::ChampionRecord;
    use crate::types::MasteryRow;
    use crate::types::{ChampSelectState, TeamSlot};
    use std::collections::HashMap;

    /// (id, key, name) catalog spanning archetypes/roles used by the scenarios.
    fn catalog() -> Vec<ChampionRecord> {
        const C: &[(i64, &str)] = &[
            (238, "Zed"),
            (91, "Talon"),
            (103, "Ahri"),
            (134, "Syndra"),
            (112, "Viktor"),
            (99, "Lux"),
            (86, "Garen"),
            (122, "Darius"),
            (54, "Malphite"),
            (164, "Camille"),
            (75, "Nasus"),
            (64, "LeeSin"),
            (254, "Vi"),
            (222, "Jinx"),
            (145, "Kaisa"),
            (81, "Ezreal"),
            (67, "Vayne"),
            (117, "Lulu"),
            (412, "Thresh"),
            (89, "Leona"),
            (111, "Nautilus"),
            (53, "Blitzcrank"),
            (350, "Yuumi"),
            (235, "Senna"),
        ];
        C.iter()
            .map(|(id, key)| ChampionRecord {
                champion_id: *id,
                key: (*key).into(),
                name: (*key).into(),
                title: String::new(),
            })
            .collect()
    }

    fn mastery(ids: &[i64]) -> Vec<MasteryRow> {
        ids.iter()
            .map(|id| MasteryRow {
                champion_id: *id,
                level: 7,
                points: 90_000,
                last_play_time: None,
            })
            .collect()
    }

    fn enemy(id: u32, pos: &str) -> TeamSlot {
        TeamSlot {
            cell_id: 5,
            champion_id: id,
            intent_champion_id: 0,
            assigned_position: pos.into(),
            is_locked: false,
        }
    }

    struct Scenario {
        name: &'static str,
        my_pos: &'static str,
        enemies: Vec<(u32, &'static str)>,
        queue: u32,
        pick_order: u8,
        pool: Vec<i64>,
    }

    /// 20 realistic SoloQ contexts: every role, blind/visible/last-pick, full
    /// AP/AD comps, ARAM, no-mastery stretch, kb-less champ.
    fn scenarios() -> Vec<Scenario> {
        let s = |name, my_pos, enemies: &[(u32, &'static str)], queue, pick_order, pool: &[i64]| {
            Scenario {
                name,
                my_pos,
                enemies: enemies.to_vec(),
                queue,
                pick_order,
                pool: pool.to_vec(),
            }
        };
        vec![
            s(
                "mid-vs-ap-laner",
                "middle",
                &[(103, "middle")],
                420,
                2,
                &[238, 134, 99],
            ),
            s(
                "top-vs-bruiser",
                "top",
                &[(122, "top")],
                420,
                3,
                &[86, 54, 164],
            ),
            s("jungle-blind-firstpick", "jungle", &[], 420, 1, &[64, 254]),
            s(
                "bot-vs-botlane",
                "bottom",
                &[(222, "bottom"), (89, "utility")],
                420,
                4,
                &[145, 81, 222],
            ),
            s(
                "support-vs-catcher",
                "utility",
                &[(412, "utility")],
                420,
                5,
                &[117, 350, 235],
            ),
            s("mid-blind-firstpick", "middle", &[], 420, 1, &[103, 112]),
            s(
                "top-lastpick-vs-ad-comp",
                "top",
                &[
                    (122, "top"),
                    (64, "jungle"),
                    (91, "middle"),
                    (222, "bottom"),
                    (555, "utility"),
                ],
                420,
                5,
                &[54, 86],
            ),
            s(
                "mid-vs-full-ap-comp",
                "middle",
                &[(103, "middle"), (112, "jungle"), (99, "utility")],
                420,
                4,
                &[238, 91],
            ),
            s("aram-no-role", "", &[], 450, 0, &[99, 222, 86]),
            s(
                "jungle-vs-diver",
                "jungle",
                &[(64, "jungle")],
                420,
                3,
                &[254, 64],
            ),
            s(
                "support-enchanter-vs-engage",
                "utility",
                &[(89, "utility"), (54, "top")],
                420,
                4,
                &[117, 350],
            ),
            s(
                "bot-poke-adc",
                "bottom",
                &[(81, "bottom")],
                420,
                2,
                &[81, 145],
            ),
            s("top-tank-vs-ap", "top", &[(103, "top")], 420, 3, &[54, 75]),
            s(
                "mid-assassin-vs-squishy",
                "middle",
                &[(99, "middle")],
                420,
                4,
                &[238, 91],
            ),
            s(
                "mid-no-mastery-stretch",
                "middle",
                &[(103, "middle")],
                420,
                2,
                &[],
            ),
            s(
                "pool-with-kbless-champ",
                "middle",
                &[(103, "middle")],
                420,
                2,
                &[990_001, 238],
            ),
            s(
                "jungle-blind-ranked",
                "jungle",
                &[],
                420,
                1,
                &[64, 254, 111],
            ),
            s(
                "support-vs-hook-comp",
                "utility",
                &[(53, "utility"), (111, "jungle")],
                420,
                4,
                &[117, 412],
            ),
            s("top-vs-ranged", "top", &[(67, "top")], 420, 3, &[122, 164]),
            s(
                "mid-control-vs-dive",
                "middle",
                &[(254, "jungle"), (164, "top")],
                420,
                4,
                &[112, 134],
            ),
        ]
    }

    fn run_scenario(
        kb: &DraftKnowledgeBase,
        champions: &[ChampionRecord],
        sc: &Scenario,
    ) -> Vec<Recommendation> {
        let their_team: Vec<TeamSlot> = sc.enemies.iter().map(|(id, p)| enemy(*id, p)).collect();
        let session = ChampSelectState {
            my_cell_id: 0,
            local_player: TeamSlot {
                cell_id: 0,
                champion_id: 0,
                intent_champion_id: 0,
                assigned_position: sc.my_pos.into(),
                is_locked: false,
            },
            my_team: vec![],
            their_team,
            my_bans: vec![],
            their_bans: vec![],
            phase: "BAN_PICK".into(),
            time_left_ms: 30_000,
            action_type: "pick".into(),
            queue_id: sc.queue,
            pick_order: sc.pick_order,
        };
        let mastery = mastery(&sc.pool);
        let role_map: HashMap<u32, Vec<String>> = HashMap::new();
        let meta_rates: HashMap<(u32, String), MetaRate> = HashMap::new();
        let ctx = ScoringContext {
            session: &session,
            mastery: &mastery,
            stats: &[],
            role_map: &role_map,
            weights: ScoringWeights::default(),
            meta_rates: &meta_rates,
            matchups: None,
            power_curves: None,
            feedback_signals: None,
        };
        let mut recs = compute_recommendations(&ctx, champions, &[], &[], kb);
        upgrade_recommendations_with_context(
            &mut recs,
            Some(&local_rules_model_pack()),
            Some(&local_seed_data_pack()),
        );
        recs
    }

    /// THE matrix. Loads the KB once, runs all 20 scenarios through the real
    /// pipeline, asserts the HARD bar for every recommendation, and prints a
    /// per-pillar coverage table (visible with `--nocapture`) for the audit doc.
    #[test]
    fn explanation_quality_matrix() {
        let kb = DraftKnowledgeBase::load().expect("Draft IQ KB yüklenemedi");
        let champions = catalog();
        let scenarios = scenarios();

        let mut total_recs = 0usize;
        let mut pillar_hits = [0usize; PILLARS.len()];
        let mut soft_gaps = 0usize;
        let mut empty_scenarios = Vec::new();

        for sc in &scenarios {
            let recs = run_scenario(&kb, &champions, sc);
            if recs.is_empty() {
                // No-mastery / kb-less may legitimately yield nothing; record it.
                empty_scenarios.push(sc.name);
                continue;
            }
            for rec in &recs {
                total_recs += 1;
                let report = audit_recommendation(rec);

                // HARD bar: no over-promising language, headline present — for
                // EVERY recommendation in EVERY scenario.
                assert!(
                    report.passes_hard_bar(),
                    "[{}] {} failed hard bar: {:?}",
                    sc.name,
                    rec.champion_key,
                    report.hard_gaps()
                );

                for (i, pillar) in PILLARS.iter().enumerate() {
                    let missing = report.gaps.iter().any(|g| match g {
                        ExplanationGap::Missing(p)
                        | ExplanationGap::Thin(p)
                        | ExplanationGap::Absolute(p) => p == pillar,
                    });
                    if !missing {
                        pillar_hits[i] += 1;
                    }
                }
                soft_gaps += report.gaps.len();
            }
        }

        assert!(total_recs > 0, "matrix must produce recommendations");

        eprintln!(
            "\n=== Draft Brain explanation coverage ({total_recs} recs over {} scenarios) ===",
            scenarios.len()
        );
        for (i, pillar) in PILLARS.iter().enumerate() {
            let pct = 100.0 * pillar_hits[i] as f32 / total_recs as f32;
            eprintln!(
                "  {:>16?}: {:>5.1}%  ({}/{})",
                pillar, pct, pillar_hits[i], total_recs
            );
        }
        eprintln!("  soft gaps total: {soft_gaps}");
        if !empty_scenarios.is_empty() {
            eprintln!("  empty scenarios: {empty_scenarios:?}");
        }
        eprintln!("===========================================================\n");

        // Decision sentence + teamfight + lane should be reliably present
        // (always-on builders). These are firm invariants, not soft metrics.
        let decision_idx = 0; // Pillar::DecisionSentence
        assert_eq!(
            pillar_hits[decision_idx], total_recs,
            "every rec must carry a meaningful decision sentence"
        );
    }

    #[test]
    fn rubric_flags_missing_and_thin_pillars() {
        // A deliberately bare recommendation: only a decision sentence.
        let mut rec = Recommendation {
            champion_id: 1,
            champion_key: "Test".into(),
            champion_name: "Test".into(),
            total_score: 0.5,
            comfort_score: 0.5,
            mechanical_comfort: 0.0,
            draft_winrate: None,
            confidence_basis: String::new(),
            matchup_score: 0.5,
            team_counter_score: 0.5,
            synergy_score: 0.5,
            meta_score: 0.5,
            role_fit_score: 0.5,
            risk_score: 0.0,
            reason: String::new(),
            headline: None,
            decision_sentence: "Test seç çünkü konfor yüksek".into(),
            score_breakdown: vec![],
            model_score: 0.5,
            rules_score: 0.5,
            model_version: String::new(),
            build_source: String::new(),
            build_confidence: String::new(),
            build_note: None,
            matchup_confidence: String::new(),
            missing_signals: Vec::new(),
            pro_presence: None,
            lane_form_score: None,
            lane_plan: Some("Engage".into()), // thin (1 word)
            mid_game_plan: None,
            teamfight_plan: None,
            teamfight_job: None,
            fallback_plan: None,
            risk_summary: None,
            why_not: vec![],
            data_sources: vec![],
            coach_depth: String::new(),
            core_items: vec![],
            situational_items: vec![],
            primary_rune_tree: 0,
            keystone: 0,
            skill_order: None,
            summoner_spells: vec![],
            secondary_runes: vec![],
            stat_shards: vec![],
            tier: crate::models::Tier::B,
            confidence: String::new(),
            games_on_champ: 0,
            wins_on_champ: 0,
            enemy_team_summary: String::new(),
            lane_opponent_name: None,
            core_item_name: None,
            draft_plan: None,
            phase_matchup: None,
        };
        let report = audit_recommendation(&rec);
        assert!(report.passes_hard_bar(), "no over-promising → hard bar ok");
        assert!(report
            .gaps
            .contains(&ExplanationGap::Thin(Pillar::LanePlan)));
        assert!(report
            .gaps
            .contains(&ExplanationGap::Missing(Pillar::TeamfightPlan)));
        assert!(report
            .gaps
            .contains(&ExplanationGap::Missing(Pillar::RiskSurfaced)));
        assert!(report
            .gaps
            .contains(&ExplanationGap::Missing(Pillar::WhyNot)));
        assert!(report
            .gaps
            .contains(&ExplanationGap::Missing(Pillar::DataConfidence)));

        // Promote it to a clean, fully-covered rec.
        rec.lane_plan = Some("Seviye 3'e kadar wave kontrol, lvl 6 all-in ara".into());
        rec.mid_game_plan = Some("Orta oyunda obje etrafında pick ara".into());
        rec.teamfight_job = Some("Takım savaşında arka sırayı koparmak".into());
        rec.fallback_plan = Some("Geride kalırsan split push ve obje yokla".into());
        rec.risk_summary = Some("Snowball gecikirse yan koridor baskısı düşer".into());
        rec.why_not = vec![
            "Konfor sinyali zayıf; mekanik hatası pahalı olabilir".into(),
            "Rol uyumu düşük; off-role tempo kaybı yaratabilir".into(),
        ];
        rec.matchup_confidence = "medium".into();
        let clean = audit_recommendation(&rec);
        assert!(
            clean.gaps.is_empty(),
            "fully populated rec has no gaps: {:?}",
            clean.gaps
        );
        assert_eq!(clean.covered, PILLARS.len() as u8);
    }

    #[test]
    fn rubric_flags_absolute_language_as_hard() {
        let mut rec = audit_fixture();
        rec.decision_sentence = "Bu pick kesin kazandırır, rakip hiçbir şey yapamaz".into();
        let report = audit_recommendation(&rec);
        assert!(!report.passes_hard_bar());
        assert!(report
            .gaps
            .iter()
            .any(|g| matches!(g, ExplanationGap::Absolute(Pillar::DecisionSentence))));
    }

    /// Minimal clean rec for tweaking in individual tests.
    fn audit_fixture() -> Recommendation {
        Recommendation {
            champion_id: 1,
            champion_key: "Fix".into(),
            champion_name: "Fix".into(),
            total_score: 0.5,
            comfort_score: 0.5,
            mechanical_comfort: 0.0,
            draft_winrate: None,
            confidence_basis: String::new(),
            matchup_score: 0.5,
            team_counter_score: 0.5,
            synergy_score: 0.5,
            meta_score: 0.5,
            role_fit_score: 0.5,
            risk_score: 0.0,
            reason: String::new(),
            headline: None,
            decision_sentence: "Fix seç çünkü kompozisyona iyi cevap veriyor".into(),
            score_breakdown: vec![],
            model_score: 0.5,
            rules_score: 0.5,
            model_version: String::new(),
            build_source: String::new(),
            build_confidence: "medium".into(),
            build_note: None,
            matchup_confidence: "medium".into(),
            missing_signals: Vec::new(),
            pro_presence: None,
            lane_form_score: None,
            lane_plan: Some("Erken wave kontrol, lvl 6 all-in ara".into()),
            mid_game_plan: Some("Orta oyunda obje etrafında oyna".into()),
            teamfight_plan: None,
            teamfight_job: Some("Arka sırayı koparmak ana görevin".into()),
            fallback_plan: Some("Geride kalırsan split push yap".into()),
            risk_summary: Some("Snowball gecikirse etkin düşer".into()),
            why_not: vec!["Konfor sinyali zayıf olabilir".into()],
            data_sources: vec![],
            coach_depth: String::new(),
            core_items: vec![],
            situational_items: vec![],
            primary_rune_tree: 0,
            keystone: 0,
            skill_order: None,
            summoner_spells: vec![],
            secondary_runes: vec![],
            stat_shards: vec![],
            tier: crate::models::Tier::B,
            confidence: "medium".into(),
            games_on_champ: 0,
            wins_on_champ: 0,
            enemy_team_summary: String::new(),
            lane_opponent_name: None,
            core_item_name: None,
            draft_plan: None,
            phase_matchup: None,
        }
    }
}
