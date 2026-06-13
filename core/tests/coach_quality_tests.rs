// MIGRATED from src-tauri/src/recommendation (host emekliligi): core integration testi.
// Import donusumu: crate::recommendation::->csa_core::, host tip yollari->csa_core::types::.
mod tests {
    use csa_core::coach_quality::*;

    #[test]
    fn absolute_language_is_flagged_without_false_positives() {
        assert!(has_absolute_language("Zed kesin kazandırır"));
        assert!(has_absolute_language("Bu pick %100 win"));
        // Legitimate coaching must NOT trip:
        assert!(!has_absolute_language(
            "Kazanma koşulu: poke ile can avantajı aç"
        ));
        assert!(!has_absolute_language("Garanti yok; matchup'ı kontrol et"));
        assert!(!has_absolute_language("Erken baskı kur, objeye çevir"));
    }

    #[test]
    fn meaningful_rejects_blank_and_bare_labels() {
        assert!(!is_meaningful("", 3));
        assert!(!is_meaningful("   ", 3));
        assert!(!is_meaningful("Zed", 3));
        assert!(is_meaningful("Zed seç çünkü konfor yüksek", 3));
    }

    #[test]
    fn dedup_is_case_insensitive_and_order_preserving() {
        let input = vec![
            "Konfor düşük".to_string(),
            "konfor düşük".to_string(),
            "Rol uyumu zayıf".to_string(),
            "  KONFOR DÜŞÜK ".to_string(),
        ];
        let out = dedup_sentences(&input);
        assert_eq!(out, vec!["Konfor düşük", "Rol uyumu zayıf"]);
    }

    #[test]
    fn audit_flags_empty_absolute_and_duplicates() {
        let issues = audit_coaching(
            "Zed",                                                   // too short → Empty
            Some("Lvl 1-3 kesin kazanırsın"), // absolute → AbsoluteLanguage(lane_plan)
            Some("Takım savaşında peel ver"), // clean
            &["Risk yüksek".to_string(), "risk yüksek".to_string()], // duplicate
        );
        assert!(issues.contains(&CoachIssue::Empty("decision_sentence".to_string())));
        assert!(issues.contains(&CoachIssue::AbsoluteLanguage("lane_plan".to_string())));
        assert!(issues.contains(&CoachIssue::Duplicate("why_not".to_string())));
    }

    #[test]
    fn extended_absolute_variations_flagged() {
        assert!(has_absolute_language("Bu draft garantili win"));
        assert!(has_absolute_language("free win bu maç"));
        assert!(has_absolute_language("rakip hiçbir şey yapamaz"));
        // Legit coaching with overlapping words must NOT trip:
        assert!(!has_absolute_language("Takımı carry'leme potansiyelin var"));
        assert!(!has_absolute_language("Win condition: objeye oyna"));
    }

    #[test]
    fn bare_why_not_entry_is_too_short() {
        let issues = audit_coaching(
            "Zed seç çünkü konfor yüksek ve matchup iyi",
            None,
            None,
            &[
                "Konfor sinyali zayıf olabilir".to_string(),
                "Risk".to_string(),
            ], // 2nd is bare
        );
        assert!(issues.contains(&CoachIssue::TooShort("why_not".to_string())));
    }

    #[test]
    fn newly_added_absolute_phrases_flagged() {
        assert!(has_absolute_language("Bu matchup'ta rakip oynayamaz"));
        assert!(has_absolute_language("Bu draftla kaybetmen imkansız"));
        assert!(has_absolute_language("Sende kesin üstünlük var"));
        // Legit coaching with "üstün"/"oyna" roots must NOT trip:
        assert!(!has_absolute_language(
            "Erken oyunda üstünlük kurmaya çalış"
        ));
        assert!(!has_absolute_language("Objeye oyna, tempoyu koru"));
    }

    #[test]
    fn runaway_decision_sentence_is_too_long() {
        let long = "kelime ".repeat(70); // 70 words, no over-promising language
        let issues = audit_coaching(&long, None, None, &[]);
        assert!(issues.contains(&CoachIssue::TooLong("decision_sentence".to_string())));
    }

    #[test]
    fn bare_label_plan_is_too_short() {
        let issues = audit_coaching(
            "Zed seç çünkü konfor yüksek ve matchup iyi",
            Some("Engage"), // 1-word plan → TooShort
            None,
            &[],
        );
        assert!(issues.contains(&CoachIssue::TooShort("lane_plan".to_string())));
    }

    #[test]
    fn clean_coaching_has_no_issues() {
        let issues = audit_coaching(
            "Zed seç: rakip kompozisyona iyi cevap veriyor. Lane planı: erken baskı kur.",
            Some("Seviye 3'e kadar wave'i kontrollü tut, lvl 6 all-in ara."),
            Some("Takım savaşında ana görevin: arka sırayı koparmak."),
            &[
                "Konfor sinyali zayıf; mekanik hatası pahalı olabilir".to_string(),
                "Rol uyumu düşük; off-role tempo kaybı yaratabilir".to_string(),
            ],
        );
        assert!(
            issues.is_empty(),
            "clean coaching must yield no issues: {issues:?}"
        );
    }

    /// Regression guard for the Draft Brain 2.0 sentence pipeline: run the REAL
    /// engine + `upgrade_*` over a mastered pool and assert every recommendation's
    /// coaching reads clean (meaningful decision sentence, no over-promising
    /// language, deduped `why_not`). Read-only use of the engine APIs — does not
    /// touch the Codex-owned builders.
    #[test]
    fn engine_pipeline_coaching_passes_audit() {
        use csa_core::draft_brain::{
            local_rules_model_pack, local_seed_data_pack, upgrade_recommendations_with_context,
        };
        use csa_core::draft_iq::DraftKnowledgeBase;
        use csa_core::engine::compute_recommendations;
        use csa_core::scoring::{MetaRate, ScoringContext, ScoringWeights};
        use csa_core::types::ChampionRecord;
        use csa_core::types::MasteryRow;
        use csa_core::types::{ChampSelectState, TeamSlot};
        use csa_core::types::{ItemData, RuneTree};
        use std::collections::HashMap;

        let session = ChampSelectState {
            my_cell_id: 0,
            local_player: TeamSlot {
                cell_id: 0,
                champion_id: 0,
                intent_champion_id: 0,
                assigned_position: "middle".into(),
                is_locked: false,
            },
            my_team: vec![],
            their_team: vec![TeamSlot {
                cell_id: 5,
                champion_id: 103, // Ahri visible as the lane opponent
                intent_champion_id: 0,
                assigned_position: "middle".into(),
                is_locked: false,
            }],
            my_bans: vec![],
            their_bans: vec![],
            phase: "BAN_PICK".into(),
            time_left_ms: 30_000,
            action_type: "pick".into(),
            queue_id: 420,
            pick_order: 2,
        };
        let champions = vec![
            ChampionRecord {
                champion_id: 238,
                key: "Zed".into(),
                name: "Zed".into(),
                title: String::new(),
            },
            ChampionRecord {
                champion_id: 99,
                key: "Lux".into(),
                name: "Lux".into(),
                title: String::new(),
            },
            ChampionRecord {
                champion_id: 86,
                key: "Garen".into(),
                name: "Garen".into(),
                title: String::new(),
            },
            ChampionRecord {
                champion_id: 103,
                key: "Ahri".into(),
                name: "Ahri".into(),
                title: String::new(),
            },
        ];
        // Mastery so comfort clears the stretch-pick gate and we get real recs.
        let mastery = vec![
            MasteryRow {
                champion_id: 238,
                level: 7,
                points: 120_000,
                last_play_time: None,
            },
            MasteryRow {
                champion_id: 99,
                level: 6,
                points: 60_000,
                last_play_time: None,
            },
            MasteryRow {
                champion_id: 86,
                level: 5,
                points: 30_000,
                last_play_time: None,
            },
        ];
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
        let kb = DraftKnowledgeBase::load().expect("Draft IQ KB yüklenemedi");
        let items: Vec<ItemData> = vec![];
        let rune_trees: Vec<RuneTree> = vec![];

        let mut recs = compute_recommendations(&ctx, &champions, &items, &rune_trees, &kb);
        assert!(!recs.is_empty(), "mastered pool must yield recommendations");

        upgrade_recommendations_with_context(
            &mut recs,
            Some(&local_rules_model_pack()),
            Some(&local_seed_data_pack()),
        );

        for rec in &recs {
            let issues = audit_coaching(
                &rec.decision_sentence,
                rec.lane_plan.as_deref(),
                rec.teamfight_job.as_deref(),
                &rec.why_not,
            );
            assert!(
                issues.is_empty(),
                "{} coaching failed quality audit: {issues:?}",
                rec.champion_key
            );
        }
    }

    // ── Edge-case pipeline guards (lock v2's no-KB / brawl / blind / first-pick
    //    fallbacks). Fully-qualified paths keep these self-contained — no
    //    module-level imports, no coupling beyond read-only engine APIs. ────────

    fn champ(id: i64, key: &str) -> csa_core::types::ChampionRecord {
        csa_core::types::ChampionRecord {
            champion_id: id,
            key: key.into(),
            name: key.into(),
            title: String::new(),
        }
    }

    fn mastered(id: i64) -> csa_core::types::MasteryRow {
        csa_core::types::MasteryRow {
            champion_id: id,
            level: 7,
            points: 80_000,
            last_play_time: None,
        }
    }

    fn enemy(id: u32, pos: &str) -> csa_core::types::TeamSlot {
        csa_core::types::TeamSlot {
            cell_id: 5,
            champion_id: id,
            intent_champion_id: 0,
            assigned_position: pos.into(),
            is_locked: false,
        }
    }

    fn sess(
        my_pos: &str,
        their_team: Vec<csa_core::types::TeamSlot>,
        queue_id: u32,
        pick_order: u8,
    ) -> csa_core::types::ChampSelectState {
        csa_core::types::ChampSelectState {
            my_cell_id: 0,
            local_player: csa_core::types::TeamSlot {
                cell_id: 0,
                champion_id: 0,
                intent_champion_id: 0,
                assigned_position: my_pos.into(),
                is_locked: false,
            },
            my_team: vec![],
            their_team,
            my_bans: vec![],
            their_bans: vec![],
            phase: "BAN_PICK".into(),
            time_left_ms: 30_000,
            action_type: "pick".into(),
            queue_id,
            pick_order,
        }
    }

    /// Run the real engine + `upgrade_*` for a mastered pool and return the recs.
    fn run_pipeline(
        session: csa_core::types::ChampSelectState,
        champions: Vec<csa_core::types::ChampionRecord>,
        mastery: Vec<csa_core::types::MasteryRow>,
    ) -> Vec<csa_core::models::Recommendation> {
        use csa_core::draft_brain::{
            local_rules_model_pack, local_seed_data_pack, upgrade_recommendations_with_context,
        };
        use csa_core::draft_iq::DraftKnowledgeBase;
        use csa_core::engine::compute_recommendations;
        use csa_core::scoring::{MetaRate, ScoringContext, ScoringWeights};
        use std::collections::HashMap;

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
        let kb = DraftKnowledgeBase::load().expect("Draft IQ KB yüklenemedi");
        let mut recs = compute_recommendations(&ctx, &champions, &[], &[], &kb);
        upgrade_recommendations_with_context(
            &mut recs,
            Some(&local_rules_model_pack()),
            Some(&local_seed_data_pack()),
        );
        recs
    }

    fn assert_all_clean(recs: &[csa_core::models::Recommendation]) {
        assert!(!recs.is_empty(), "pipeline must yield recommendations");
        for rec in recs {
            let issues = audit_coaching(
                &rec.decision_sentence,
                rec.lane_plan.as_deref(),
                rec.teamfight_job.as_deref(),
                &rec.why_not,
            );
            assert!(
                issues.is_empty(),
                "{} coaching failed audit: {issues:?}",
                rec.champion_key
            );
        }
    }

    #[test]
    fn pipeline_kbless_champion_coaching_clean() {
        // Synthetic champion absent from the KB → exercises the no-KB fallback.
        let champions = vec![champ(990_001, "ZzSyntheticKbless"), champ(238, "Zed")];
        let mastery = vec![mastered(990_001), mastered(238)];
        let recs = run_pipeline(
            sess("middle", vec![enemy(103, "middle")], 420, 2),
            champions,
            mastery,
        );
        assert!(
            recs.iter().any(|r| r.champion_id == 990_001),
            "kb-less champ must still be scored + coached"
        );
        assert_all_clean(&recs);
    }

    #[test]
    fn pipeline_blind_no_opponent_coaching_clean() {
        let champions = vec![champ(238, "Zed"), champ(99, "Lux"), champ(86, "Garen")];
        let mastery = vec![mastered(238), mastered(99), mastered(86)];
        // Blind (queue 430), no enemy locked, no lane opponent visible.
        let recs = run_pipeline(sess("middle", vec![], 430, 0), champions, mastery);
        assert_all_clean(&recs);
    }

    #[test]
    fn pipeline_aram_coaching_clean() {
        let champions = vec![champ(238, "Zed"), champ(99, "Lux"), champ(86, "Garen")];
        let mastery = vec![mastered(238), mastered(99), mastered(86)];
        // ARAM (queue 450): laneless brawl scoring profile.
        let recs = run_pipeline(sess("", vec![], 450, 0), champions, mastery);
        assert_all_clean(&recs);
    }

    #[test]
    fn pipeline_first_pick_coaching_clean() {
        let champions = vec![champ(238, "Zed"), champ(99, "Lux"), champ(86, "Garen")];
        let mastery = vec![mastered(238), mastered(99), mastered(86)];
        // First pick: no enemy info, pick_order = 1.
        let recs = run_pipeline(sess("middle", vec![], 420, 1), champions, mastery);
        assert_all_clean(&recs);
    }
}
