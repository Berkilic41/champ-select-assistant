// MIGRATED from src-tauri/src/recommendation (host emekliligi): core integration testi.
// Import donusumu: crate::recommendation::->csa_core::, host tip yollari->csa_core::types::.
//! Sprint E integration tests for the recommendation scoring functions and
//! build_advisor. All scoring functions now take a `ScoringContext` which
//! carries the `meta_rates` map introduced in Sprint E.

mod scoring_tests {
    use csa_core::types::{ChampSelectState, TeamSlot};
    use csa_core::scoring::{
        meta_score, risk_score, role_fit_score, MetaRate, ScoringContext, ScoringWeights,
    };
    use std::collections::HashMap;

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

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
            feedback_signals: None,
        }
    }

    // -------------------------------------------------------------------------
    // meta_score tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_meta_score_high_win_rate() {
        // win_rate 0.55, large sample → maps to 1.0 (clamped)
        // Formula: (0.55 - 0.48) / 0.07 = 1.0
        let session = empty_session("top");
        let role_map = HashMap::new();
        let mut meta_rates = HashMap::new();
        meta_rates.insert(
            (157, "top".into()),
            MetaRate {
                win_rate: 0.55,
                ban_rate: 0.05,
                sample_size: 5000,
            },
        );
        let ctx = make_ctx(&session, &role_map, &meta_rates);
        let score = meta_score(157, &ctx);
        assert!(
            (score - 1.0).abs() < 1e-5,
            "win_rate=0.55 should map to 1.0, got {score}"
        );
    }

    #[test]
    fn test_meta_score_low_confidence() {
        // When sample_size is very small the score still uses win_rate;
        // but risk_score covers the confidence penalty separately.
        // Here we test that a champion with a small sample but good win_rate
        // still receives a score > 0.3 (data-based, not flat fallback).
        let session = empty_session("middle");
        let role_map = HashMap::new();
        let mut meta_rates = HashMap::new();
        meta_rates.insert(
            (99, "middle".into()),
            MetaRate {
                win_rate: 0.52,
                ban_rate: 0.01,
                sample_size: 50,
            },
        );
        let ctx = make_ctx(&session, &role_map, &meta_rates);
        let score = meta_score(99, &ctx);
        assert!(
            score > 0.3,
            "small-sample champion with positive win_rate should score > 0.3, got {score}"
        );
    }

    #[test]
    fn test_meta_score_no_data() {
        // No meta_rates row → neutral fallback 0.3
        let session = empty_session("middle");
        let role_map = HashMap::new();
        let meta_rates = HashMap::new();
        let ctx = make_ctx(&session, &role_map, &meta_rates);
        let score = meta_score(99, &ctx);
        assert!(
            (score - 0.3).abs() < 1e-5,
            "missing data should return neutral 0.3, got {score}"
        );
    }

    // -------------------------------------------------------------------------
    // role_fit_score tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_role_fit_mage_mid() {
        // mage + assassin in middle = full overlap (both expected roles present)
        // 0.2 + (2/2)*0.8 = 1.0
        let session = empty_session("middle");
        let mut role_map = HashMap::new();
        role_map.insert(99, vec!["mage".into(), "assassin".into()]);
        let meta_rates = HashMap::new();
        let ctx = make_ctx(&session, &role_map, &meta_rates);
        let score = role_fit_score(99, &ctx);
        assert!(
            score >= 0.7,
            "mage in middle should have high role_fit (≥ 0.7), got {score}"
        );
    }

    #[test]
    fn test_role_fit_marksman_support() {
        // marksman in utility = zero overlap with expected ["support"]
        // 0.2 + (0/1)*0.8 = 0.2
        let session = empty_session("utility");
        let mut role_map = HashMap::new();
        role_map.insert(22, vec!["marksman".into()]);
        let meta_rates = HashMap::new();
        let ctx = make_ctx(&session, &role_map, &meta_rates);
        let score = role_fit_score(22, &ctx);
        assert!(
            score <= 0.2,
            "marksman in utility should have low role_fit (≤ 0.2), got {score}"
        );
    }

    #[test]
    fn test_role_fit_unknown_position() {
        // Empty / unknown position → neutral 0.5
        let session = empty_session("");
        let mut role_map = HashMap::new();
        role_map.insert(1, vec!["fighter".into()]);
        let meta_rates = HashMap::new();
        let ctx = make_ctx(&session, &role_map, &meta_rates);
        let score = role_fit_score(1, &ctx);
        assert!(
            (score - 0.5).abs() < 1e-5,
            "unknown position should return 0.5, got {score}"
        );
    }

    // -------------------------------------------------------------------------
    // risk_score tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_risk_score_high_ban_rate() {
        // ban_rate 0.25 > 0.20 → penalty 0.05; sample 5000 ≥ 200 → no penalty
        // expected: 0.05
        let session = empty_session("top");
        let role_map = HashMap::new();
        let mut meta_rates = HashMap::new();
        meta_rates.insert(
            (157, "top".into()),
            MetaRate {
                win_rate: 0.51,
                ban_rate: 0.25,
                sample_size: 5000,
            },
        );
        let ctx = make_ctx(&session, &role_map, &meta_rates);
        let score = risk_score(157, &ctx);
        assert!(
            (score - 0.05).abs() < 1e-5,
            "high ban_rate should yield penalty 0.05, got {score}"
        );
    }

    #[test]
    fn test_risk_score_low_sample() {
        // ban_rate 0.05 ≤ 0.20 → no ban penalty; sample_size 50 < 200 → penalty 0.05
        // expected: 0.05
        let session = empty_session("top");
        let role_map = HashMap::new();
        let mut meta_rates = HashMap::new();
        meta_rates.insert(
            (157, "top".into()),
            MetaRate {
                win_rate: 0.51,
                ban_rate: 0.05,
                sample_size: 50,
            },
        );
        let ctx = make_ctx(&session, &role_map, &meta_rates);
        let score = risk_score(157, &ctx);
        assert!(
            (score - 0.05).abs() < 1e-5,
            "low sample_size should yield penalty 0.05, got {score}"
        );
    }
}

mod build_advisor_tests {
    use csa_core::types::ItemData;
    use csa_core::build_advisor::{counter_item_advice, situational_item_ids};
    use csa_core::team_analysis::TeamComposition;

    fn make_item(id: u32, tags: &[&str]) -> ItemData {
        ItemData {
            id,
            name: format!("Item{id}"),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            total_gold: 3000,
            is_completed: true,
        }
    }

    #[test]
    fn test_situational_items_vs_tank_heavy() {
        // enemy tanks >= 2 → "Health" tag items should be recommended,
        // "PercentHealth" items must NOT appear (bug fix: PercentHealth removed)
        let enemy = TeamComposition {
            tanks: 2,
            ..Default::default()
        };

        let items = vec![
            make_item(1, &["Health", "Armor"]),
            make_item(2, &["PercentHealth"]),
            make_item(3, &["Health"]),
        ];

        let result = situational_item_ids(&enemy, &items);

        assert!(
            !result.is_empty(),
            "Should return at least one situational item for tank-heavy enemy"
        );
        assert!(
            !result.contains(&2),
            "PercentHealth item (id=2) must NOT appear vs tank-heavy enemy after fix"
        );
        let has_health = result.contains(&1) || result.contains(&3);
        assert!(
            has_health,
            "A Health-tagged item should be recommended vs tank-heavy enemy"
        );
    }

    #[test]
    fn test_situational_items_vs_ap_heavy() {
        // is_ap_heavy → SpellBlock recommended
        let enemy = TeamComposition {
            is_ap_heavy: true,
            ..Default::default()
        };

        let items = vec![make_item(10, &["SpellBlock"]), make_item(11, &["Armor"])];

        let result = situational_item_ids(&enemy, &items);
        assert!(
            result.contains(&10),
            "SpellBlock item should be recommended vs AP-heavy"
        );
    }

    #[test]
    fn test_situational_items_no_threat() {
        // Balanced (default) enemy → no situational items
        let enemy = TeamComposition::default();
        let items = vec![make_item(20, &["Health"]), make_item(21, &["SpellBlock"])];
        let result = situational_item_ids(&enemy, &items);
        assert!(
            result.is_empty(),
            "Balanced enemy should yield no situational items"
        );
    }

    // ── counter_item_advice ────────────────────────────────────────────────

    #[test]
    fn counter_items_ap_heavy_suggests_mr() {
        let enemy = TeamComposition {
            is_ap_heavy: true,
            ..Default::default()
        };
        let items = vec![make_item(10, &["SpellBlock"]), make_item(11, &["Armor"])];
        let advice = counter_item_advice(&enemy, false, 0, &items, false);
        let mr = advice.iter().find(|h| h.category == "MR").expect("MR hint");
        assert!(
            mr.item_ids.contains(&10),
            "MR should include the SpellBlock item"
        );
    }

    #[test]
    fn counter_items_squishy_carry_is_text_only_not_tank_icons() {
        // An ADC vs AP must NOT be told to build a tank MR item — text-only advice
        // naming carry-survival items instead.
        let enemy = TeamComposition {
            is_ap_heavy: true,
            ..Default::default()
        };
        let items = vec![make_item(10, &["SpellBlock"])];
        let advice = counter_item_advice(&enemy, false, 0, &items, true);
        let mr = advice.iter().find(|h| h.category == "MR").expect("MR hint");
        assert!(mr.item_ids.is_empty(), "squishy carry → no tank-item icons");
        assert!(
            mr.reason.contains("carry"),
            "names carry survival: {}",
            mr.reason
        );
    }

    #[test]
    fn counter_items_sustain_suggests_anti_heal_text_only() {
        let enemy = TeamComposition::default();
        let advice = counter_item_advice(&enemy, true, 0, &[], false);
        let ah = advice
            .iter()
            .find(|h| h.category == "Anti-heal")
            .expect("anti-heal hint");
        assert!(
            ah.item_ids.is_empty(),
            "anti-heal is text-only (no reliable tag)"
        );
        assert!(ah.reason.contains("anti-heal"));
    }

    #[test]
    fn counter_items_no_threat_is_empty() {
        let enemy = TeamComposition::default();
        let advice = counter_item_advice(&enemy, false, 0, &[], false);
        assert!(advice.is_empty());
    }
}

mod engine_di4b_tests {
    use csa_core::types::ChampionRecord;
    use csa_core::types::{ChampSelectState, TeamSlot};
    use csa_core::draft_iq::DraftKnowledgeBase;
    use csa_core::feedback_signal::FeedbackSignal;
    use csa_core::engine::compute_recommendations;
    use csa_core::scoring::{MetaRate, ScoringContext, ScoringWeights};
    use csa_core::types::ChampionStats;
    use std::collections::HashMap;

    fn make_slot(cell: i32, champ: u32, pos: &str) -> TeamSlot {
        TeamSlot {
            cell_id: cell,
            champion_id: champ,
            intent_champion_id: champ,
            assigned_position: pos.to_string(),
            is_locked: champ > 0,
        }
    }

    fn make_champ(id: i64, key: &str, name: &str) -> ChampionRecord {
        ChampionRecord {
            champion_id: id,
            key: key.to_string(),
            name: name.to_string(),
            title: String::new(),
        }
    }

    fn base_session(queue_id: u32) -> ChampSelectState {
        ChampSelectState {
            my_cell_id: 0,
            local_player: make_slot(0, 0, "middle"),
            my_team: vec![],
            their_team: vec![],
            my_bans: vec![],
            their_bans: vec![],
            phase: "BAN_PICK".into(),
            time_left_ms: 30_000,
            action_type: "pick".into(),
            queue_id,
            pick_order: 0,
        }
    }

    fn make_mastery(
        champion_id: i64,
        level: i64,
        points: i64,
    ) -> csa_core::types::MasteryRow {
        csa_core::types::MasteryRow {
            champion_id,
            level,
            points,
            last_play_time: None,
        }
    }

    fn make_stats(champion_id: i64, games: u32, wins: u32) -> ChampionStats {
        ChampionStats {
            champion_id,
            games,
            wins,
            kda_avg: 3.0,
        }
    }

    fn empty_ctx<'a>(
        session: &'a ChampSelectState,
        mastery: &'a [csa_core::types::MasteryRow],
        stats: &'a [ChampionStats],
        role_map: &'a HashMap<u32, Vec<String>>,
        meta_rates: &'a HashMap<(u32, String), MetaRate>,
    ) -> ScoringContext<'a> {
        ScoringContext {
            session,
            mastery,
            stats,
            role_map,
            meta_rates,
            weights: ScoringWeights::default(),
            matchups: None,
            power_curves: None,
            feedback_signals: None,
        }
    }

    /// Test 1: pool now includes ALL champions, not just those with mastery/stats.
    /// We verify by including Orianna (no mastery) as an ally-combo candidate;
    /// if the pool were restricted, she would never appear.
    #[test]
    fn pool_widened_to_all_champions() {
        let kb = DraftKnowledgeBase::load().expect("KB load failed");

        // Champions in all_champions: A(1) with mastery, Orianna(61) without mastery,
        // Nocturne(56) who is an ally pick (excluded from candidates but needed for key lookup)
        let champs = vec![
            make_champ(1, "TestFighter", "Test Fighter"),
            make_champ(56, "Nocturne", "Nocturne"),
            make_champ(61, "Orianna", "Orianna"),
        ];

        // Only TestFighter has mastery
        let mastery = vec![make_mastery(1, 6, 100_000)];
        let stats = vec![make_stats(1, 10, 6)];

        // Ally: Nocturne (id=56, not in all_champions, but in combos.json)
        let mut session = base_session(0);
        session.my_team = vec![make_slot(1, 56, "jungle")];

        let role_map = HashMap::new();
        let meta_rates = HashMap::new();
        let ctx = empty_ctx(&session, &mastery, &stats, &role_map, &meta_rates);

        let recs = compute_recommendations(&ctx, &champs, &[], &[], &kb);
        let rec_ids: Vec<u32> = recs.iter().map(|r| r.champion_id).collect();

        // Orianna should appear as stretch pick (combo_bonus with Nocturne >= 0.80)
        assert!(
            rec_ids.contains(&61),
            "Orianna should be recommended via Nocturne combo even with no mastery, got: {rec_ids:?}"
        );
    }

    /// Test 2 + 4: stretch pick gate — at most 1 low-comfort pick, and it must have risk_note.
    #[test]
    fn stretch_pick_has_risk_note_and_one_at_most() {
        let kb = DraftKnowledgeBase::load().expect("KB load failed");

        // Put Orianna(61) and Veigar(45) as candidates with NO mastery/stats
        // Plus 4 high-comfort regular candidates; Nocturne(56) as ally needs key lookup
        let champs = vec![
            make_champ(1, "RegA", "RegA"),
            make_champ(2, "RegB", "RegB"),
            make_champ(3, "RegC", "RegC"),
            make_champ(4, "RegD", "RegD"),
            make_champ(56, "Nocturne", "Nocturne"),
            make_champ(61, "Orianna", "Orianna"),
            make_champ(45, "Veigar", "Veigar"),
        ];

        let mastery = vec![
            make_mastery(1, 7, 180_000),
            make_mastery(2, 6, 120_000),
            make_mastery(3, 5, 80_000),
            make_mastery(4, 5, 60_000),
        ];
        let stats = vec![
            make_stats(1, 20, 12),
            make_stats(2, 15, 9),
            make_stats(3, 10, 6),
            make_stats(4, 8, 5),
        ];

        let mut session = base_session(0);
        // Nocturne (56) as ally → Orianna gets combo_bonus=0.92
        session.my_team = vec![make_slot(1, 56, "jungle")];

        let role_map = HashMap::new();
        let meta_rates = HashMap::new();
        let ctx = empty_ctx(&session, &mastery, &stats, &role_map, &meta_rates);

        let recs = compute_recommendations(&ctx, &champs, &[], &[], &kb);

        let stretch_picks: Vec<_> = recs.iter().filter(|r| r.comfort_score < 0.10).collect();
        assert!(
            stretch_picks.len() <= 1,
            "At most 1 stretch pick allowed, got {}",
            stretch_picks.len()
        );

        // If Orianna is in results, she must have risk_note
        if let Some(orianna) = recs.iter().find(|r| r.champion_id == 61) {
            let has_risk_note = orianna
                .draft_plan
                .as_ref()
                .and_then(|p| p.risk_note.as_ref())
                .is_some();
            assert!(
                has_risk_note,
                "Stretch pick Orianna must have risk_note set"
            );
        }
    }

    /// Test 3: no stretch picks when no strong combo exists.
    #[test]
    fn no_stretch_when_no_strong_combo() {
        let kb = DraftKnowledgeBase::load().expect("KB load failed");

        let champs = vec![
            make_champ(1, "RegA", "RegA"),
            make_champ(2, "RegB", "RegB"),
            make_champ(56, "Nocturne", "Nocturne"), // ally key lookup
            // Veigar: no mastery, no known combos → should be excluded
            make_champ(45, "Veigar", "Veigar"),
        ];

        let mastery = vec![make_mastery(1, 5, 80_000), make_mastery(2, 5, 60_000)];
        let stats = vec![make_stats(1, 8, 5), make_stats(2, 6, 4)];

        // Ally is Nocturne — but Veigar has no combo with Nocturne
        let mut session = base_session(0);
        session.my_team = vec![make_slot(1, 56, "jungle")];

        let role_map = HashMap::new();
        let meta_rates = HashMap::new();
        let ctx = empty_ctx(&session, &mastery, &stats, &role_map, &meta_rates);

        let recs = compute_recommendations(&ctx, &champs, &[], &[], &kb);

        let stretch_picks: Vec<_> = recs.iter().filter(|r| r.comfort_score < 0.10).collect();
        assert!(
            stretch_picks.is_empty(),
            "No stretch picks when no strong combo exists, found: {stretch_picks:?}"
        );
        // Veigar must not appear
        assert!(
            recs.iter().all(|r| r.champion_id != 45),
            "Veigar (no mastery, no combo) must not appear in recommendations"
        );
    }

    /// Test 5: ARAM skips combo logic — draft_plan.combo_with should be empty.
    #[test]
    fn aram_skips_combo_logic() {
        let kb = DraftKnowledgeBase::load().expect("KB load failed");

        let champs = vec![make_champ(61, "Orianna", "Orianna")];
        let mastery = vec![make_mastery(61, 6, 100_000)];
        let stats = vec![make_stats(61, 10, 6)];

        let mut session = base_session(450); // ARAM
        session.my_team = vec![make_slot(1, 56, "jungle")]; // Nocturne ally

        let role_map = HashMap::new();
        let meta_rates = HashMap::new();
        let ctx = empty_ctx(&session, &mastery, &stats, &role_map, &meta_rates);

        let recs = compute_recommendations(&ctx, &champs, &[], &[], &kb);
        assert!(!recs.is_empty(), "Should return at least Orianna");

        let orianna = &recs[0];
        let combo_count = orianna
            .draft_plan
            .as_ref()
            .map(|p| p.combo_with.len())
            .unwrap_or(0);
        assert_eq!(
            combo_count, 0,
            "ARAM should skip combo logic — combo_with must be empty"
        );
    }

    /// Test 6: Orianna outranks Veigar when Nocturne is an ally.
    #[test]
    fn nocturne_orianna_outranks_veigar() {
        let kb = DraftKnowledgeBase::load().expect("KB load failed");

        let champs = vec![
            make_champ(56, "Nocturne", "Nocturne"), // ally key lookup
            make_champ(61, "Orianna", "Orianna"),
            make_champ(45, "Veigar", "Veigar"),
        ];

        // Give both equal mastery so scoring difference comes from combo
        let mastery = vec![make_mastery(61, 5, 80_000), make_mastery(45, 5, 80_000)];
        let stats = vec![make_stats(61, 8, 5), make_stats(45, 8, 5)];

        let mut session = base_session(0);
        session.my_team = vec![make_slot(1, 56, "jungle")]; // Nocturne ally

        let role_map = HashMap::new();
        let meta_rates = HashMap::new();
        let ctx = empty_ctx(&session, &mastery, &stats, &role_map, &meta_rates);

        let recs = compute_recommendations(&ctx, &champs, &[], &[], &kb);
        assert!(!recs.is_empty(), "Should return recommendations");

        let orianna_score = recs
            .iter()
            .find(|r| r.champion_id == 61)
            .map(|r| r.total_score);
        let veigar_score = recs
            .iter()
            .find(|r| r.champion_id == 45)
            .map(|r| r.total_score);

        if let (Some(os), Some(vs)) = (orianna_score, veigar_score) {
            assert!(
                os > vs,
                "Orianna (Nocturne combo bonus) should outscore Veigar, got orianna={os:.3} veigar={vs:.3}"
            );
        } else {
            // At minimum Orianna should be in the results
            assert!(
                orianna_score.is_some(),
                "Orianna should appear when Nocturne is ally"
            );
        }
    }

    /// Test 7: champion not in KB falls back to tag heuristic (no panic, stable output).
    #[test]
    fn unknown_champion_fallback_tag_heuristic() {
        let kb = DraftKnowledgeBase::load().expect("KB load failed");

        // "UnknownChamp" with id 9999 — not in KB
        let champs = vec![make_champ(9999, "UnknownChamp", "Unknown")];
        let mastery = vec![make_mastery(9999, 5, 80_000)];
        let stats = vec![make_stats(9999, 5, 3)];

        let session = base_session(0);
        let role_map = HashMap::new();
        let meta_rates = HashMap::new();
        let ctx = empty_ctx(&session, &mastery, &stats, &role_map, &meta_rates);

        // Must not panic; should return a valid recommendation
        let recs = compute_recommendations(&ctx, &champs, &[], &[], &kb);
        assert!(
            !recs.is_empty(),
            "Unknown champion should still be scored via fallback"
        );
        assert!(
            recs[0].draft_plan.is_none(),
            "Champion not in KB should have no draft_plan (no analyzer ran)"
        );
    }

    #[test]
    fn feedback_signal_is_tiny_personalization_tiebreaker() {
        let kb = DraftKnowledgeBase::load().expect("KB load failed");
        let champs = vec![
            make_champ(9001, "NeutralA", "NeutralA"),
            make_champ(9002, "NeutralB", "NeutralB"),
        ];
        let mastery = vec![make_mastery(9001, 5, 80_000), make_mastery(9002, 5, 80_000)];
        let stats = vec![make_stats(9001, 8, 4), make_stats(9002, 8, 4)];
        let session = base_session(0);
        let role_map = HashMap::new();
        let meta_rates = HashMap::new();
        let mut feedback_signals = HashMap::new();
        feedback_signals.insert(
            9002,
            FeedbackSignal {
                champion_id: 9002,
                positive: 8,
                negative: 0,
                sample: 8,
                net_sentiment: 1.0,
                confidence: "high",
                suggested_delta: 0.03,
            },
        );
        let ctx = ScoringContext {
            session: &session,
            mastery: &mastery,
            stats: &stats,
            role_map: &role_map,
            meta_rates: &meta_rates,
            weights: ScoringWeights::default(),
            matchups: None,
            power_curves: None,
            feedback_signals: Some(&feedback_signals),
        };

        let recs = compute_recommendations(&ctx, &champs, &[], &[], &kb);
        let boosted = recs
            .iter()
            .find(|r| r.champion_id == 9002)
            .expect("boosted candidate");
        let neutral = recs
            .iter()
            .find(|r| r.champion_id == 9001)
            .expect("neutral candidate");

        assert!(
            boosted.total_score > neutral.total_score,
            "feedback should only break close ties, boosted={:.3} neutral={:.3}",
            boosted.total_score,
            neutral.total_score
        );
        assert!(
            boosted.total_score - neutral.total_score <= 0.031,
            "feedback delta must stay tiny, diff={:.3}",
            boosted.total_score - neutral.total_score
        );
    }
}

mod e2e_tests {
    use csa_core::types::ChampionRecord;
    use csa_core::types::MasteryRow;
    use csa_core::types::{ChampSelectState, TeamSlot};
    use csa_core::engine::compute_recommendations;
    use csa_core::{
        draft_iq::DraftKnowledgeBase,
        scoring::{MetaRate, ScoringContext, ScoringWeights},
    };
    use csa_core::types::ChampionStats;
    use std::collections::HashMap;

    fn make_slot(cell: i32, champ: u32, pos: &str) -> TeamSlot {
        TeamSlot {
            cell_id: cell,
            champion_id: champ,
            intent_champion_id: champ,
            assigned_position: pos.to_string(),
            is_locked: champ > 0,
        }
    }

    fn make_champ(id: i64, key: &str) -> ChampionRecord {
        ChampionRecord {
            champion_id: id,
            key: key.to_string(),
            name: key.to_string(),
            title: String::new(),
        }
    }

    /// E2E smoke: Nocturne ally → Orianna gets combo hint and outscores Veigar.
    #[test]
    fn orianna_outranks_veigar_with_nocturne_ally() {
        let kb = DraftKnowledgeBase::load().expect("KB must load from embedded JSON");

        let champs = vec![
            make_champ(56, "Nocturne"), // ally key lookup
            make_champ(61, "Orianna"),
            make_champ(45, "Veigar"),
        ];

        let mastery = vec![
            MasteryRow {
                champion_id: 61,
                level: 5,
                points: 80_000,
                last_play_time: None,
            },
            MasteryRow {
                champion_id: 45,
                level: 5,
                points: 80_000,
                last_play_time: None,
            },
        ];
        let stats = vec![
            ChampionStats {
                champion_id: 61,
                games: 8,
                wins: 5,
                kda_avg: 3.0,
            },
            ChampionStats {
                champion_id: 45,
                games: 8,
                wins: 5,
                kda_avg: 3.0,
            },
        ];

        let session = ChampSelectState {
            my_cell_id: 0,
            local_player: make_slot(0, 0, "middle"),
            my_team: vec![make_slot(1, 56, "jungle")], // Nocturne as ally
            their_team: vec![],
            my_bans: vec![],
            their_bans: vec![],
            phase: "BAN_PICK".into(),
            time_left_ms: 30_000,
            action_type: "pick".into(),
            queue_id: 0,
            pick_order: 0,
        };

        let role_map: HashMap<u32, Vec<String>> = HashMap::new();
        let meta_rates: HashMap<(u32, String), MetaRate> = HashMap::new();
        let ctx = ScoringContext {
            session: &session,
            mastery: &mastery,
            stats: &stats,
            role_map: &role_map,
            meta_rates: &meta_rates,
            weights: ScoringWeights::default(),
            matchups: None,
            power_curves: None,
            feedback_signals: None,
        };

        let recs = compute_recommendations(&ctx, &champs, &[], &[], &kb);
        assert!(!recs.is_empty(), "Must return at least one recommendation");

        // Orianna must appear
        let orianna = recs.iter().find(|r| r.champion_id == 61);
        assert!(
            orianna.is_some(),
            "Orianna must be recommended when Nocturne is ally"
        );

        // Orianna must have a combo hint referencing Nocturne
        let plan = orianna
            .and_then(|r| r.draft_plan.as_ref())
            .expect("Orianna should have draft_plan");
        assert!(!plan.combo_with.is_empty(), "combo_with must be non-empty");
        assert!(
            plan.combo_with
                .iter()
                .any(|h| h.ally_champion_key == "Nocturne"),
            "Combo hint must reference Nocturne"
        );

        // Orianna must outscore Veigar (same mastery, different combo bonus)
        let os = orianna.map(|r| r.total_score).unwrap();
        if let Some(vs) = recs
            .iter()
            .find(|r| r.champion_id == 45)
            .map(|r| r.total_score)
        {
            assert!(
                os > vs,
                "Orianna ({os:.3}) should outscore Veigar ({vs:.3}) due to Nocturne combo"
            );
        }
    }
}

mod analyze_champion_tests {
    use csa_core::types::ChampionRecord;
    use csa_core::types::MasteryRow;
    use csa_core::types::{ChampSelectState, TeamSlot};
    use csa_core::draft_iq::DraftKnowledgeBase;
    use csa_core::engine::{analyze_champion, compute_recommendations};
    use csa_core::scoring::{MetaRate, ScoringContext, ScoringWeights};
    use csa_core::types::ChampionStats;
    use std::collections::HashMap;

    fn champ(id: i64, key: &str) -> ChampionRecord {
        ChampionRecord {
            champion_id: id,
            key: key.to_string(),
            name: key.to_string(),
            title: String::new(),
        }
    }
    fn slot(cell: i32, champion_id: u32, pos: &str, locked: bool) -> TeamSlot {
        TeamSlot {
            cell_id: cell,
            champion_id,
            intent_champion_id: 0,
            assigned_position: pos.to_string(),
            is_locked: locked,
        }
    }
    fn mastery(id: i64) -> MasteryRow {
        MasteryRow {
            champion_id: id,
            level: 5,
            points: 80_000,
            last_play_time: None,
        }
    }
    fn stat(id: i64) -> ChampionStats {
        ChampionStats {
            champion_id: id,
            games: 8,
            wins: 4,
            kda_avg: 3.0,
        }
    }

    /// Post-lock pinning: the locked champion is excluded from
    /// `compute_recommendations` (it's "picked") but MUST still be returnable via
    /// `analyze_champion` so the UI can show *its* build/plan after lock.
    #[test]
    fn analyze_champion_returns_locked_pick_excluded_from_recs() {
        let kb = DraftKnowledgeBase::load().expect("KB load");
        let champs = vec![champ(103, "Ahri"), champ(99, "Lux"), champ(45, "Veigar")];

        // Local player locked Ahri (103) mid; Lux/Veigar have mastery so the pool
        // is non-empty and the exclusion assertion is non-vacuous.
        let session = ChampSelectState {
            my_cell_id: 0,
            local_player: slot(0, 103, "middle", true),
            my_team: vec![slot(0, 103, "middle", true)],
            their_team: vec![],
            my_bans: vec![],
            their_bans: vec![],
            phase: "FINALIZATION".into(),
            time_left_ms: 30_000,
            action_type: "".into(),
            queue_id: 420,
            pick_order: 1,
        };
        let role_map: HashMap<u32, Vec<String>> = HashMap::new();
        let meta_rates: HashMap<(u32, String), MetaRate> = HashMap::new();
        let mastery = vec![mastery(103), mastery(99), mastery(45)];
        let stats = vec![stat(103), stat(99), stat(45)];
        let ctx = ScoringContext {
            session: &session,
            mastery: &mastery,
            stats: &stats,
            role_map: &role_map,
            weights: ScoringWeights::default(),
            meta_rates: &meta_rates,
            matchups: None,
            power_curves: None,
            feedback_signals: None,
        };

        let recs = compute_recommendations(&ctx, &champs, &[], &[], &kb);
        assert!(
            !recs.is_empty(),
            "Lux/Veigar should still be recommended (non-vacuous exclusion check)"
        );
        assert!(
            recs.iter().all(|r| r.champion_id != 103),
            "locked Ahri must be excluded from the recommendation pool"
        );

        let locked = analyze_champion(103, &ctx, &champs, &[], &[], &kb);
        assert!(
            locked.is_some(),
            "analyze_champion must return the locked champion even though it's picked"
        );
        assert_eq!(locked.unwrap().champion_id, 103);
    }

    /// Unknown champion id → None, no panic.
    #[test]
    fn analyze_champion_unknown_id_returns_none() {
        let kb = DraftKnowledgeBase::load().expect("KB load");
        let champs = vec![champ(103, "Ahri")];
        let session = ChampSelectState {
            my_cell_id: 0,
            local_player: slot(0, 0, "middle", false),
            my_team: vec![],
            their_team: vec![],
            my_bans: vec![],
            their_bans: vec![],
            phase: "BAN_PICK".into(),
            time_left_ms: 30_000,
            action_type: "pick".into(),
            queue_id: 420,
            pick_order: 0,
        };
        let role_map: HashMap<u32, Vec<String>> = HashMap::new();
        let meta_rates: HashMap<(u32, String), MetaRate> = HashMap::new();
        let mastery: Vec<MasteryRow> = vec![];
        let stats: Vec<ChampionStats> = vec![];
        let ctx = ScoringContext {
            session: &session,
            mastery: &mastery,
            stats: &stats,
            role_map: &role_map,
            weights: ScoringWeights::default(),
            meta_rates: &meta_rates,
            matchups: None,
            power_curves: None,
            feedback_signals: None,
        };
        assert!(analyze_champion(9999, &ctx, &champs, &[], &[], &kb).is_none());
    }

    /// Hard off-role pick (role data present, zero overlap) is demoted below an
    /// on-role pick of equal comfort/meta in a non-brawl queue. Uses ids absent
    /// from the KB so the result depends only on role_fit + the mismatch penalty
    /// (no analyzer/combo noise).
    #[test]
    fn hard_off_role_demoted_below_on_role() {
        let kb = DraftKnowledgeBase::load().expect("KB load");
        let champs = vec![champ(9001, "OffRoleTank"), champ(9002, "OnRoleMage")];

        let mut role_map: HashMap<u32, Vec<String>> = HashMap::new();
        role_map.insert(9001, vec!["fighter".into(), "tank".into()]); // off-role at mid
        role_map.insert(9002, vec!["mage".into()]); // on-role at mid

        let mastery = vec![mastery(9001), mastery(9002)];
        let stats = vec![stat(9001), stat(9002)];

        let session = ChampSelectState {
            my_cell_id: 0,
            local_player: slot(0, 0, "middle", false),
            my_team: vec![],
            their_team: vec![],
            my_bans: vec![],
            their_bans: vec![],
            phase: "BAN_PICK".into(),
            time_left_ms: 30_000,
            action_type: "pick".into(),
            queue_id: 420,
            pick_order: 0,
        };
        let meta_rates: HashMap<(u32, String), MetaRate> = HashMap::new();
        let ctx = ScoringContext {
            session: &session,
            mastery: &mastery,
            stats: &stats,
            role_map: &role_map,
            weights: ScoringWeights::default(),
            meta_rates: &meta_rates,
            matchups: None,
            power_curves: None,
            feedback_signals: None,
        };

        let recs = compute_recommendations(&ctx, &champs, &[], &[], &kb);
        let off = recs
            .iter()
            .find(|r| r.champion_id == 9001)
            .map(|r| r.total_score)
            .expect("off-role scored");
        let on = recs
            .iter()
            .find(|r| r.champion_id == 9002)
            .map(|r| r.total_score)
            .expect("on-role scored");
        assert!(
            on > off,
            "on-role mage ({on:.3}) must outrank off-role tank ({off:.3})"
        );
        assert_eq!(
            recs[0].champion_id, 9002,
            "on-role pick should rank first in a non-brawl queue"
        );
    }
}
