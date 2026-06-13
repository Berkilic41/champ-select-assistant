//! Draft Brain explanation GROUNDING scorer (Sprint #2, Claude) — engine-pure.
//!
//! `explanation_audit` answers "is each pillar PRESENT + non-over-promising?" and
//! measured 100% presence. This module answers the *next* question the user flagged:
//! is the prose actually **grounded in THIS draft's data** — does it name the enemy it
//! counters, the item it builds, the team need it fills, the patch — or is it a generic
//! coaching template that would read the same for any pick?
//!
//! Pure: no DB / network / UI. The command layer resolves the concrete tokens (enemy /
//! ally / item display names, team need, patch) and passes them as `GroundingContext`;
//! this only does deterministic, boundary-aware token matching over the reasoning prose.
//! No fabrication: a dimension with no context tokens is *not applicable* (a blind pick
//! is not penalised for not naming an enemy), never a fake "grounded".
#![allow(dead_code)] // consumed by the grounding matrix test + a later coach/QA surface

use crate::models::Recommendation;

/// Evidence axes a grounded explanation can tie itself to. `team_need`/`patch` are the
/// weaker (single-token) axes; matchup/synergy/build are proper-noun driven.
pub const GROUNDING_DIMENSIONS: [&str; 5] = ["matchup", "synergy", "build", "team_need", "patch"];

pub const GROUNDING_VERDICTS: [&str; 4] =
    ["grounded", "partial", "generic", "insufficient_context"];

const GROUNDED_RATIO: f32 = 0.67;
const PARTIAL_RATIO: f32 = 0.34;

/// Caller-resolved concrete tokens for the current draft (Rust-only). Empty fields are
/// "not applicable" — they neither help nor hurt the grounding score.
#[derive(Debug, Clone, Default)]
pub struct GroundingContext {
    /// Enemy champion display names in the draft.
    pub enemy_names: Vec<String>,
    /// Ally / combo-partner champion display names.
    pub ally_names: Vec<String>,
    /// Resolved core + situational item display names.
    pub item_names: Vec<String>,
    /// The specific comp gap this pick fills (prose token, e.g. "ön saf", "tutuş").
    pub team_need: Option<String>,
    /// Patch string, e.g. "14.11".
    pub patch: Option<String>,
}

impl GroundingContext {
    fn tokens_for(&self, dimension: &str) -> Vec<&str> {
        match dimension {
            "matchup" => self.enemy_names.iter().map(String::as_str).collect(),
            "synergy" => self.ally_names.iter().map(String::as_str).collect(),
            "build" => self.item_names.iter().map(String::as_str).collect(),
            "team_need" => self.team_need.as_deref().into_iter().collect(),
            "patch" => self.patch.as_deref().into_iter().collect(),
            _ => Vec::new(),
        }
    }
}

/// Per-recommendation grounding result.
#[derive(Debug, Clone, PartialEq)]
pub struct GroundingReport {
    pub verdict: String,
    /// grounded dimensions / applicable dimensions (0.0 when nothing is applicable).
    pub grounded_score: f32,
    pub grounded_dimensions: Vec<String>,
    pub applicable_dimensions: Vec<String>,
    /// Distinct concrete tokens actually referenced in the prose.
    pub specificity_hits: u32,
    pub generic: bool,
}

/// True when `needle` occurs in `haystack` as a whole token (boundaries are non-
/// alphanumeric on both sides) — so "Vi" does not match inside "Viktor", but "Vi'nin"
/// or "Vi koparır" does. Both are matched lowercased; Unicode-aware (TR ı/ş/ç…).
fn mentions(haystack_lower: &str, needle: &str) -> bool {
    let needle = needle.trim().to_lowercase();
    if needle.chars().count() < 2 {
        return false; // too short to be a specific reference
    }
    for (pos, _) in haystack_lower.match_indices(&needle) {
        let before_ok = haystack_lower[..pos]
            .chars()
            .next_back()
            .map(|c| !c.is_alphanumeric())
            .unwrap_or(true);
        let after_ok = haystack_lower[pos + needle.len()..]
            .chars()
            .next()
            .map(|c| !c.is_alphanumeric())
            .unwrap_or(true);
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

/// Score grounding of explanation `prose` (already the reasoning fields, joined) against
/// the draft context. Pure + deterministic.
pub fn score_grounding(prose: &str, ctx: &GroundingContext) -> GroundingReport {
    let haystack = prose.to_lowercase();
    let mut grounded_dimensions = Vec::new();
    let mut applicable_dimensions = Vec::new();
    let mut specificity_hits = 0u32;

    for dimension in GROUNDING_DIMENSIONS {
        let tokens = ctx.tokens_for(dimension);
        if tokens.is_empty() {
            continue; // not applicable — no fabrication
        }
        applicable_dimensions.push(dimension.to_string());
        let hits = tokens.iter().filter(|t| mentions(&haystack, t)).count();
        if hits > 0 {
            grounded_dimensions.push(dimension.to_string());
            specificity_hits += hits as u32;
        }
    }

    let applicable = applicable_dimensions.len();
    let grounded_score = if applicable == 0 {
        0.0
    } else {
        grounded_dimensions.len() as f32 / applicable as f32
    };
    let verdict = if applicable == 0 {
        "insufficient_context"
    } else if grounded_score >= GROUNDED_RATIO {
        "grounded"
    } else if grounded_score >= PARTIAL_RATIO {
        "partial"
    } else {
        "generic"
    };

    GroundingReport {
        verdict: verdict.to_string(),
        grounded_score,
        grounded_dimensions,
        applicable_dimensions,
        specificity_hits,
        generic: verdict == "generic",
    }
}

/// Collect a recommendation's *reasoning* prose (not the enemy-team data dump, not the
/// raw provenance) into one string for grounding analysis.
pub fn reasoning_prose(rec: &Recommendation) -> String {
    let mut parts: Vec<&str> = vec![rec.decision_sentence.as_str()];
    for opt in [
        rec.lane_plan.as_deref(),
        rec.mid_game_plan.as_deref(),
        rec.teamfight_plan.as_deref(),
        rec.teamfight_job.as_deref(),
        rec.fallback_plan.as_deref(),
        rec.risk_summary.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        parts.push(opt);
    }
    for w in &rec.why_not {
        parts.push(w.as_str());
    }
    parts.join(" \n ")
}

/// Convenience: score a recommendation directly against a resolved context.
pub fn audit_grounding(rec: &Recommendation, ctx: &GroundingContext) -> GroundingReport {
    score_grounding(&reasoning_prose(rec), ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> GroundingContext {
        GroundingContext {
            enemy_names: vec!["Ahri".into(), "Vi".into()],
            ally_names: vec!["Yasuo".into()],
            item_names: vec!["Kraken Slayer".into()],
            team_need: Some("ön saf".into()),
            patch: Some("14.11".into()),
        }
    }

    #[test]
    fn fully_grounded_prose_scores_grounded() {
        let prose = "Ahri'ye karşı lvl 6 öncesi all-in riskli; Vi ganklarına dikkat. \
             Yasuo ile ön saf eksiğini kapatıp Kraken Slayer ile DPS bas. Patch 14.11 meta.";
        let r = score_grounding(prose, &ctx());
        assert_eq!(r.verdict, "grounded");
        assert!(r.grounded_score >= 0.67);
        assert!(!r.generic);
        // matchup (Ahri+Vi) / synergy (Yasuo) / build (Kraken) / team_need / patch all hit.
        assert_eq!(r.applicable_dimensions.len(), 5);
        assert_eq!(r.grounded_dimensions.len(), 5);
        assert!(r.specificity_hits >= 5);
    }

    #[test]
    fn generic_template_prose_scores_generic() {
        // Real-shaped Draft Brain template prose: zero proper nouns, all generic.
        let prose = "Bu şampiyonu seç çünkü kompozisyona iyi cevap veriyor. \
             Erken wave kontrol yap, lvl 6 all-in ara. Orta oyunda obje etrafında oyna. \
             Takım savaşında arka sırayı koparmak ana görevin. Geride kalırsan split push yap.";
        let r = score_grounding(prose, &ctx());
        assert_eq!(r.verdict, "generic");
        assert!(r.generic);
        assert!(r.grounded_dimensions.is_empty());
        assert_eq!(r.grounded_score, 0.0);
        assert_eq!(r.applicable_dimensions.len(), 5);
    }

    #[test]
    fn partial_grounding_names_enemy_only() {
        // Names the enemy + patch but no build/synergy/team-need reference.
        let prose = "Ahri'ye karşı menzil avantajın var. Patch 14.11 için güçlü.";
        let r = score_grounding(prose, &ctx());
        assert_eq!(r.verdict, "partial");
        assert!((0.34..0.67).contains(&r.grounded_score));
        assert!(r.grounded_dimensions.contains(&"matchup".to_string()));
        assert!(r.grounded_dimensions.contains(&"patch".to_string()));
    }

    #[test]
    fn blind_pick_with_no_context_is_insufficient() {
        // No enemies, no items, no need, no patch → nothing applicable, never faked.
        let prose = "Erken tempoyu al ve obje önceliği kur.";
        let r = score_grounding(prose, &GroundingContext::default());
        assert_eq!(r.verdict, "insufficient_context");
        assert_eq!(r.grounded_score, 0.0);
        assert!(r.applicable_dimensions.is_empty());
        assert!(!r.generic);
    }

    #[test]
    fn token_matching_respects_word_boundaries() {
        // "Vi" must not match inside "Viktor"/"vizyon"; must match standalone.
        assert!(!mentions(&"viktor ile vizyon kur".to_lowercase(), "Vi"));
        assert!(mentions(&"vi ekranı koparır".to_lowercase(), "Vi"));
        assert!(mentions(&"ahri'ye karşı".to_lowercase(), "Ahri"));
        // One-char tokens are ignored as non-specific.
        assert!(!mentions("x", "X"));
    }

    #[test]
    fn only_applicable_dimensions_count() {
        // Context has only enemies; prose names them → grounded over 1 applicable dim.
        let ctx = GroundingContext {
            enemy_names: vec!["Darius".into()],
            ..Default::default()
        };
        let r = score_grounding("Darius'un Q'suna ticket verme.", &ctx);
        assert_eq!(r.applicable_dimensions, vec!["matchup".to_string()]);
        assert_eq!(r.verdict, "grounded");
        assert_eq!(r.grounded_score, 1.0);
    }

    #[test]
    fn vocabularies_are_stable() {
        assert_eq!(GROUNDING_DIMENSIONS.len(), 5);
        assert_eq!(GROUNDING_VERDICTS.len(), 4);
        for v in [
            score_grounding("Ahri Vi Yasuo Kraken Slayer ön saf 14.11", &ctx()).verdict,
            score_grounding("generic", &ctx()).verdict,
            score_grounding("x", &GroundingContext::default()).verdict,
        ] {
            assert!(GROUNDING_VERDICTS.contains(&v.as_str()), "verdict {v}");
        }
    }

    // ── Real-pipeline grounding MEASUREMENT (the audit number) ───────────────────
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

    const CATALOG: &[(i64, &str)] = &[
        (238, "Zed"),
        (103, "Ahri"),
        (134, "Syndra"),
        (99, "Lux"),
        (122, "Darius"),
        (54, "Malphite"),
        (64, "LeeSin"),
        (254, "Vi"),
        (222, "Jinx"),
        (145, "Kaisa"),
        (412, "Thresh"),
        (89, "Leona"),
        (117, "Lulu"),
    ];

    fn name_of(id: i64) -> String {
        CATALOG
            .iter()
            .find(|(cid, _)| *cid == id)
            .map(|(_, n)| (*n).to_string())
            .unwrap_or_default()
    }

    fn catalog() -> Vec<ChampionRecord> {
        CATALOG
            .iter()
            .map(|(id, key)| ChampionRecord {
                champion_id: *id,
                key: (*key).into(),
                name: (*key).into(),
                title: String::new(),
            })
            .collect()
    }

    /// Measure grounding of the REAL Draft Brain output. Presence was 100%; this prints
    /// the grounded/partial/generic split so the audit doc has a measured baseline for the
    /// specificity frontier. The HARD assertion is only that the scorer is wired and that
    /// no prose claims grounding it cannot have (deterministic verdicts in-vocabulary).
    #[test]
    fn grounding_measurement_over_real_pipeline() {
        let kb = DraftKnowledgeBase::load().expect("Draft IQ KB yüklenemedi");
        let champions = catalog();
        // (my_pos, enemies[(id,pos)], pool) — visible matchups so `matchup` is applicable.
        let scenarios: &[(&str, &[(u32, &str)], &[i64])] = &[
            ("middle", &[(103, "middle")], &[238, 134]),
            ("top", &[(122, "top")], &[54]),
            ("bottom", &[(222, "bottom"), (89, "utility")], &[145]),
            ("utility", &[(412, "utility")], &[117]),
        ];

        let (mut grounded, mut partial, mut generic, mut total) = (0u32, 0u32, 0u32, 0u32);
        for (my_pos, enemies, pool) in scenarios {
            let their_team: Vec<TeamSlot> = enemies
                .iter()
                .map(|(id, p)| TeamSlot {
                    cell_id: 5,
                    champion_id: *id,
                    intent_champion_id: 0,
                    assigned_position: (*p).into(),
                    is_locked: false,
                })
                .collect();
            let session = ChampSelectState {
                my_cell_id: 0,
                local_player: TeamSlot {
                    cell_id: 0,
                    champion_id: 0,
                    intent_champion_id: 0,
                    assigned_position: (*my_pos).into(),
                    is_locked: false,
                },
                my_team: vec![],
                their_team,
                my_bans: vec![],
                their_bans: vec![],
                phase: "BAN_PICK".into(),
                time_left_ms: 30_000,
                action_type: "pick".into(),
                queue_id: 420,
                pick_order: 3,
            };
            let mastery: Vec<MasteryRow> = pool
                .iter()
                .map(|id| MasteryRow {
                    champion_id: *id,
                    level: 7,
                    points: 90_000,
                    last_play_time: None,
                })
                .collect();
            let role_map: HashMap<u32, Vec<String>> = HashMap::new();
            let meta_rates: HashMap<(u32, String), MetaRate> = HashMap::new();
            let scoring = ScoringContext {
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
            let mut recs = compute_recommendations(&scoring, &champions, &[], &[], &kb);
            upgrade_recommendations_with_context(
                &mut recs,
                Some(&local_rules_model_pack()),
                Some(&local_seed_data_pack()),
            );

            let enemy_names: Vec<String> = enemies
                .iter()
                .map(|(id, _)| name_of(*id as i64))
                .filter(|n| !n.is_empty())
                .collect();
            let gctx = GroundingContext {
                enemy_names,
                ..Default::default()
            };
            for rec in &recs {
                let report = audit_grounding(rec, &gctx);
                assert!(
                    GROUNDING_VERDICTS.contains(&report.verdict.as_str()),
                    "verdict {} out of vocab",
                    report.verdict
                );
                total += 1;
                match report.verdict.as_str() {
                    "grounded" => grounded += 1,
                    "partial" => partial += 1,
                    "generic" => generic += 1,
                    _ => {}
                }
            }
        }

        assert!(total > 0, "measurement must produce recommendations");
        eprintln!(
            "\n=== Draft Brain explanation GROUNDING ({total} recs) ===\n  \
             grounded: {grounded}  partial: {partial}  generic: {generic}\n\
             ====================================================\n"
        );
        // REGRESSION THRESHOLD (2026-06-07): with a visible lane opponent the builders now
        // name it (matchup grounding), so the prose must stay grounded. Measured 5/5;
        // locked at ≥60% so a slide back to generic templates turns this red.
        assert!(
            grounded * 100 >= total * 60,
            "grounding regressed: only {grounded}/{total} recs name the matchup (≥60% required)"
        );
    }
}
