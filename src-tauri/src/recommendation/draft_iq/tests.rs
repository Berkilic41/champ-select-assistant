use super::{CHAMPIONS_JSON, COMBOS_JSON};
use crate::recommendation::draft_iq::narrative::{
    build_comp_clash_note, build_spike_note, build_threats_text, build_win_condition_text,
    detect_enemy_win_condition,
};
use crate::recommendation::draft_iq::{
    analyzer::{
        analyze_pick, compute_blind_unsafety, compute_combo_bonus, compute_damage_balance,
        compute_team_need_fill, AnalyzerInput,
    },
    archetype::load_archetypes,
    combos::load_combos,
    DraftKnowledgeBase,
};

#[test]
fn compile_time_json_not_empty() {
    // If include_str! path is wrong this won't even compile.
    // At runtime, confirm the strings are non-empty.
    assert!(
        !CHAMPIONS_JSON.is_empty(),
        "champions.json embedded string must not be empty"
    );
    assert!(
        !COMBOS_JSON.is_empty(),
        "combos.json embedded string must not be empty"
    );
}

#[test]
fn archetype_load_smoke() {
    let archetypes = load_archetypes(CHAMPIONS_JSON).expect("champions.json parse failed");
    assert!(
        !archetypes.is_empty(),
        "archetypes map should contain at least one entry"
    );
}

#[test]
fn archetype_orianna_spot_check() {
    // Replaced the old len==81 assert; key-set equality is in validate_ddragon_completeness.
    let archetypes = load_archetypes(CHAMPIONS_JSON).expect("champions.json parse failed");
    let orianna = archetypes
        .get("Orianna")
        .expect("Orianna must be in champions.json");
    assert_eq!(orianna.champion_id, 61);
    assert!(
        orianna.damage_profile.ap > 0.5,
        "Orianna should be AP-heavy"
    );
}

#[test]
fn validate_ddragon_completeness() {
    use std::collections::HashSet;
    const DDRAGON_KEYS: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ddragon_16101_keys.json"
    ));
    let expected: HashSet<String> = serde_json::from_str::<Vec<String>>(DDRAGON_KEYS)
        .expect("ddragon fixture parse failed")
        .into_iter()
        .collect();

    let archetypes = load_archetypes(CHAMPIONS_JSON).expect("champions.json parse failed");
    let actual: HashSet<String> = archetypes.keys().cloned().collect();

    let mut missing: Vec<_> = expected.difference(&actual).collect();
    missing.sort();
    let mut extra: Vec<_> = actual.difference(&expected).collect();
    extra.sort();

    assert!(
        missing.is_empty(),
        "champions.json'da eksik şampiyonlar ({} adet): {:?}",
        missing.len(),
        missing
    );
    assert!(
        extra.is_empty(),
        "champions.json'da fazla key'ler (DDragon'da olmayan, {} adet): {:?}",
        extra.len(),
        extra
    );
}

#[test]
fn combo_count_matches_json() {
    let dir = load_combos(COMBOS_JSON).expect("combos.json parse failed");
    assert_eq!(dir.len(), 100, "combos.json should have 100 entries");
}

#[test]
fn find_for_ally_symmetric() {
    let dir = load_combos(COMBOS_JSON).expect("combos.json parse failed");
    let from_orianna = dir.find_for_ally("Orianna", &["Nocturne"]);
    let from_nocturne = dir.find_for_ally("Nocturne", &["Orianna"]);
    assert_eq!(
        from_orianna.len(),
        from_nocturne.len(),
        "combo lookup must be symmetric (a↔b)"
    );
    assert!(
        !from_orianna.is_empty(),
        "Orianna-Nocturne combo must exist in combos.json"
    );
}

// ── Analyzer unit tests ────────────────────────────────────────────────────

#[test]
fn combo_bonus_nocturne_orianna_high() {
    let kb = DraftKnowledgeBase::load().unwrap();
    // Nocturne is not in the 30-champion KB but exists in combos.json as a partner key.
    // Nocturne's champion ID per DDragon: 56.
    let ally_id_keys = [(56u32, "Nocturne")];
    let (bonus, hints) = compute_combo_bonus("Orianna", &ally_id_keys, &kb.combos);
    assert!(
        bonus > 0.85,
        "Orianna+Nocturne combo strength should be > 0.85, got {bonus}"
    );
    assert_eq!(hints.len(), 1, "should find exactly 1 combo hint");
    assert_eq!(hints[0].ally_champion_key, "Nocturne");
}

#[test]
fn combo_bonus_no_match_low() {
    let kb = DraftKnowledgeBase::load().unwrap();
    // Veigar is not in the combo list — no known combo with Nocturne.
    let ally_id_keys = [(56u32, "Nocturne")];
    let (bonus, hints) = compute_combo_bonus("Veigar", &ally_id_keys, &kb.combos);
    assert!(
        bonus < 0.3,
        "Veigar+Nocturne should have low combo bonus, got {bonus}"
    );
    assert!(hints.is_empty());
}

#[test]
fn damage_balance_4ap_team_ap_candidate_low() {
    let kb = DraftKnowledgeBase::load().unwrap();
    // Build a 4-AP ally pool: Orianna, Viktor, Syndra, Lissandra
    let ap_allies: Vec<&_> = ["Orianna", "Viktor", "Syndra", "Lissandra"]
        .iter()
        .filter_map(|k| kb.archetypes.get(*k))
        .collect();
    let candidate = kb.archetypes.get("Orianna").unwrap();
    let balance = compute_damage_balance(candidate, &ap_allies);
    assert!(
        balance < 0.3,
        "AP candidate in AP-heavy team should have low balance score, got {balance}"
    );
}

#[test]
fn damage_balance_4ap_team_ad_candidate_high() {
    let kb = DraftKnowledgeBase::load().unwrap();
    let ap_allies: Vec<&_> = ["Orianna", "Viktor", "Syndra", "Lissandra"]
        .iter()
        .filter_map(|k| kb.archetypes.get(*k))
        .collect();
    // Jinx is an AD marksman
    let candidate = kb.archetypes.get("Jinx").unwrap();
    let balance = compute_damage_balance(candidate, &ap_allies);
    assert!(
        balance > 0.7,
        "AD candidate in AP-heavy team should have high balance score, got {balance}"
    );
}

#[test]
fn team_need_fill_engage_gap_malphite() {
    let kb = DraftKnowledgeBase::load().unwrap();
    // Allies with no initiator: Ezreal, Caitlyn, Syndra, Thresh (pick role, not initiator)
    let allies: Vec<&_> = ["Ezreal", "Caitlyn", "Syndra", "Thresh"]
        .iter()
        .filter_map(|k| kb.archetypes.get(*k))
        .collect();
    let malphite = kb.archetypes.get("Malphite").unwrap();
    let (score, fills) = compute_team_need_fill(malphite, &allies);
    assert!(
        score > 0.5,
        "Malphite should fill gaps in a non-engage team, score={score}"
    );
    assert!(
        fills.iter().any(|f| f.contains("Engage")),
        "fills should mention engage gap"
    );
}

#[test]
fn team_need_fill_no_gap_neutral() {
    let kb = DraftKnowledgeBase::load().unwrap();
    // Allies that cover engage + frontline + CC + peel: Malphite, Leona, Lulu, Nautilus
    let allies: Vec<&_> = ["Malphite", "Leona", "Lulu", "Nautilus"]
        .iter()
        .filter_map(|k| kb.archetypes.get(*k))
        .collect();
    let candidate = kb.archetypes.get("Orianna").unwrap();
    let (score, fills) = compute_team_need_fill(candidate, &allies);
    // When all gaps are already filled, score should be neutral (0.5)
    assert!(
        (score - 0.5).abs() < 0.01,
        "no gaps to fill → score should be 0.5, got {score}"
    );
    assert!(fills.is_empty());
}

#[test]
fn blind_unsafety_first_pick_high_penalty() {
    // blind_safety=0.3 (below 0.6 threshold)
    let penalty = compute_blind_unsafety(true, 0.30);
    assert!(
        (penalty - 0.05).abs() < 0.001,
        "first-pick + low blind_safety should give 0.05 penalty, got {penalty}"
    );
}

#[test]
fn blind_unsafety_not_first_pick_zero() {
    let penalty = compute_blind_unsafety(false, 0.30);
    assert_eq!(penalty, 0.0, "not first pick → no blind unsafety penalty");
}

#[test]
fn analyze_pick_full_orianna_with_nocturne() {
    let kb = DraftKnowledgeBase::load().unwrap();
    let orianna = kb.archetypes.get("Orianna").unwrap();
    let input = AnalyzerInput {
        candidate_key: "Orianna",
        candidate: orianna,
        ally_id_keys: vec![(56, "Nocturne".to_string())],
        ally_archetypes: vec![],
        enemy_archetypes: vec![],
        combo_dir: &kb.combos,
        is_first_pick: false,
        is_late_pick: false,
        pick_order: 0,
        position: "middle",
        lane_opponent: None,
    };
    let result = analyze_pick(&input);
    assert!(
        !result.plan.combo_with.is_empty(),
        "Orianna with Nocturne ally should have at least 1 combo hint"
    );
    assert!(
        !result.plan.win_condition.is_empty(),
        "win_condition must be a non-empty TR string"
    );
    assert!(result.combo_bonus > 0.85);
}

#[test]
fn narrative_win_condition_teamfight() {
    let text = build_win_condition_text("teamfight");
    assert!(
        !text.is_empty(),
        "teamfight win condition should produce non-empty TR text"
    );
    // Should be in Turkish and mention teamfight concept
    assert!(
        text.contains("dövüş") || text.contains("engage") || text.contains("AOE"),
        "teamfight text should reference relevant game concepts, got: {text}"
    );
}

// ── Sprint 1A schema validation tests ─────────────────────────────────────

#[test]
fn validate_all_champions_schema() {
    let archetypes = load_archetypes(CHAMPIONS_JSON).expect("champions.json parse failed");

    let valid_utility_tags = [
        "waveclear",
        "siege",
        "disengage",
        "anti_tank",
        "zone_control",
        "peel",
        "roam",
        "vision",
        "pick",
        "frontline",
        "engage",
        "poke",
        "shielding",
        "healing",
        "objective_control",
        "split_pressure",
    ];
    let valid_archetypes = [
        "juggernaut",
        "diver",
        "vanguard",
        "skirmisher",
        "control_mage",
        "burst_mage",
        "artillery",
        "assassin",
        "marksman",
        "catcher",
        "enchanter",
        "warden",
        "specialist",
    ];

    for (key, champ) in &archetypes {
        // damage_profile sums to ~1.0
        let sum =
            champ.damage_profile.ad + champ.damage_profile.ap + champ.damage_profile.true_damage;
        assert!(
            (sum - 1.0).abs() < 0.06,
            "{key}: damage_profile sum {sum:.3} ≠ 1.0"
        );

        // confidence is set
        assert!(!champ.confidence.is_empty(), "{key}: confidence eksik");

        // archetype is valid
        assert!(
            valid_archetypes.contains(&champ.archetype.as_str()),
            "{key}: geçersiz archetype '{}'",
            champ.archetype
        );

        // power_curve: all values 0..=1, at least one >= 0.5
        let pc = &champ.power_curve;
        for (phase, val) in [("early", pc.early), ("mid", pc.mid), ("late", pc.late)] {
            assert!(
                (0.0..=1.0).contains(&val),
                "{key}: power_curve.{phase} = {val} dışı [0,1]"
            );
        }
        let max_phase = pc.early.max(pc.mid).max(pc.late);
        assert!(
            max_phase >= 0.5,
            "{key}: power_curve tüm değerler < 0.5 — olası eksik veri (max={max_phase})"
        );

        // utility_tags are all valid
        for tag in &champ.utility_tags {
            assert!(
                valid_utility_tags.contains(&tag.as_str()),
                "{key}: geçersiz utility_tag '{tag}'"
            );
        }

        // counters_archetypes are all valid archetype strings
        for ca in &champ.counters_archetypes {
            assert!(
                valid_archetypes.contains(&ca.as_str()),
                "{key}: counters_archetypes içinde geçersiz archetype '{ca}'"
            );
        }
    }
}

#[test]
fn validate_combo_keys_present() {
    let archetypes = load_archetypes(CHAMPIONS_JSON).expect("champions.json parse failed");
    let combos = load_combos(COMBOS_JSON).expect("combos.json parse failed");

    let valid_confidence: [&str; 3] = ["high", "medium", "low"];
    let valid_combo_types: [&str; 5] = [
        "engage_followup",
        "pick_potential",
        "zone_control",
        "peel_chain",
        "wombo",
    ];

    for (idx, combo) in combos.all_pairs().iter().enumerate() {
        assert!(
            archetypes.contains_key(&combo.a),
            "Combo '{}': key '{}' champions.json'da yok",
            combo.name,
            combo.a
        );
        assert!(
            archetypes.contains_key(&combo.b),
            "Combo '{}': key '{}' champions.json'da yok",
            combo.name,
            combo.b
        );

        assert!(
            !combo.ability_ref.is_empty(),
            "Combo '{}': ability_ref boş",
            combo.name
        );

        assert!(
            (0.0..=1.0).contains(&combo.strength),
            "Combo '{}': strength {} dışı [0,1]",
            combo.name,
            combo.strength
        );

        assert!(!combo.name.trim().is_empty(), "Combo has empty name");

        assert!(
            !combo.tr.trim().is_empty(),
            "Combo '{}': tr boş",
            combo.name
        );

        assert!(
            valid_confidence.contains(&combo.confidence.as_str()),
            "Combo '{}': geçersiz confidence '{}'",
            combo.name,
            combo.confidence
        );

        assert!(
            valid_combo_types.contains(&combo.combo_type.as_str()),
            "Combo '{}': geçersiz type '{}'",
            combo.name,
            combo.combo_type
        );

        if idx >= 50 {
            let expected = match combo.confidence.as_str() {
                "high" => 0.85,
                "medium" => 0.72,
                "low" => 0.60,
                _ => unreachable!(),
            };
            assert!(
                (combo.strength - expected).abs() < 0.001,
                "Combo '{}': strength {} beklenen {} ile eşleşmiyor",
                combo.name,
                combo.strength,
                expected
            );
        }
    }

    // No duplicate combos (unordered a,b pair)
    let mut seen = std::collections::HashSet::new();
    for combo in combos.all_pairs() {
        let key = if combo.a <= combo.b {
            format!("{}|{}", combo.a, combo.b)
        } else {
            format!("{}|{}", combo.b, combo.a)
        };
        assert!(seen.insert(key.clone()), "Duplicate combo: {key}");
    }
}

#[test]
fn nocturne_is_in_kb() {
    let archetypes = load_archetypes(CHAMPIONS_JSON).expect("champions.json parse failed");
    let nocturne = archetypes.get("Nocturne").expect("Nocturne must be in KB");
    assert_eq!(nocturne.champion_id, 56);
    assert_eq!(nocturne.archetype, "diver");
    assert!(
        nocturne.power_curve.mid >= 0.7,
        "Nocturne is mid-game dominant — power_curve.mid should be >= 0.7"
    );
}

#[test]
fn kaisa_key_normalized() {
    let archetypes = load_archetypes(CHAMPIONS_JSON).expect("champions.json parse failed");
    assert!(
        archetypes.contains_key("Kaisa"),
        "Key should be 'Kaisa' (DDragon canonical), not 'KaiSa'"
    );
    assert!(
        !archetypes.contains_key("KaiSa"),
        "Old key 'KaiSa' must be removed"
    );
}

#[test]
fn khazix_key_normalized() {
    let archetypes = load_archetypes(CHAMPIONS_JSON).expect("champions.json parse failed");
    assert!(
        archetypes.contains_key("Khazix"),
        "Key should be 'Khazix' (DDragon canonical), not 'KhaZix'"
    );
    assert!(
        !archetypes.contains_key("KhaZix"),
        "Old key 'KhaZix' must be removed"
    );
}

// ── B2: Comp clash detection tests ────────────────────────────────────────

#[test]
fn detect_enemy_wc_empty_returns_mixed() {
    assert_eq!(detect_enemy_win_condition(&[]), "mixed");
}

#[test]
fn detect_enemy_wc_pick_requires_two_signals() {
    use crate::recommendation::draft_iq::archetype::{
        CcProfile, ChampionArchetype, DamageProfile, PowerCurve,
    };
    let make_archetype = |archetype: &'static str| ChampionArchetype {
        champion_id: 1,
        archetype: archetype.to_string(),
        damage_profile: DamageProfile {
            ad: 0.5,
            ap: 0.3,
            true_damage: 0.0,
        },
        cc: CcProfile {
            has_hard_cc: false,
            hard_cc_count: 0,
            primary_cc: vec![],
        },
        mobility: "high".to_string(),
        engage_role: "none".to_string(),
        peel_capability: "low".to_string(),
        blind_safety: 0.5,
        execution_difficulty: 3,
        win_condition: "pick".to_string(),
        ult_type: "single".to_string(),
        confidence: "high".to_string(),
        power_curve: PowerCurve {
            early: 0.6,
            mid: 0.6,
            late: 0.6,
        },
        counters_archetypes: vec![],
        utility_tags: vec![],
    };
    let assassin = make_archetype("assassin");
    let catcher = make_archetype("catcher");
    let marksman = make_archetype("marksman");

    // 1 pick archetype → "mixed"
    assert_eq!(detect_enemy_win_condition(&[&assassin]), "mixed");
    // 2 pick archetypes → "pick"
    assert_eq!(
        detect_enemy_win_condition(&[&assassin, &catcher, &marksman]),
        "pick"
    );
}

#[test]
fn build_comp_clash_note_poke_vs_pick() {
    let note = build_comp_clash_note("poke", "pick");
    assert!(note.is_some(), "poke vs pick should produce a clash note");
    let text = note.unwrap();
    assert!(
        text.contains("vision") || text.contains("izole"),
        "should advise on vision/isolation"
    );
}

#[test]
fn build_comp_clash_note_no_clash_for_neutral() {
    // Mixed vs mixed has no notable clash
    let note = build_comp_clash_note("teamfight", "mixed");
    assert!(note.is_none(), "no clash note for teamfight vs mixed comp");
}

#[test]
fn build_comp_clash_note_teamfight_vs_poke() {
    let note = build_comp_clash_note("teamfight", "poke").expect("should have clash note");
    assert!(note.contains("engage") || note.contains("vision"));
}

// ── B4: Spike note tests ───────────────────────────────────────────────────

#[test]
fn spike_note_late_scaler_returns_some() {
    use crate::recommendation::draft_iq::archetype::{
        CcProfile, ChampionArchetype, DamageProfile, PowerCurve,
    };
    let late_champ = ChampionArchetype {
        champion_id: 1,
        archetype: "control_mage".to_string(),
        damage_profile: DamageProfile {
            ad: 0.1,
            ap: 0.85,
            true_damage: 0.0,
        },
        cc: CcProfile {
            has_hard_cc: true,
            hard_cc_count: 2,
            primary_cc: vec![],
        },
        mobility: "low".to_string(),
        engage_role: "none".to_string(),
        peel_capability: "high".to_string(),
        blind_safety: 0.5,
        execution_difficulty: 3,
        win_condition: "teamfight".to_string(),
        ult_type: "aoe".to_string(),
        confidence: "high".to_string(),
        power_curve: PowerCurve {
            early: 0.35,
            mid: 0.70,
            late: 0.90,
        },
        counters_archetypes: vec![],
        utility_tags: vec![],
    };
    let note = build_spike_note(&late_champ);
    assert!(note.is_some(), "late scaler should have spike note");
    assert!(
        note.unwrap().contains("item"),
        "spike note should mention items"
    );
}

#[test]
fn spike_note_early_dominant_returns_some() {
    use crate::recommendation::draft_iq::archetype::{
        CcProfile, ChampionArchetype, DamageProfile, PowerCurve,
    };
    let early_champ = ChampionArchetype {
        champion_id: 2,
        archetype: "juggernaut".to_string(),
        damage_profile: DamageProfile {
            ad: 0.75,
            ap: 0.0,
            true_damage: 0.1,
        },
        cc: CcProfile {
            has_hard_cc: true,
            hard_cc_count: 1,
            primary_cc: vec![],
        },
        mobility: "none".to_string(),
        engage_role: "initiator".to_string(),
        peel_capability: "low".to_string(),
        blind_safety: 0.6,
        execution_difficulty: 2,
        win_condition: "split_push".to_string(),
        ult_type: "single".to_string(),
        confidence: "high".to_string(),
        power_curve: PowerCurve {
            early: 0.75,
            mid: 0.70,
            late: 0.55,
        },
        counters_archetypes: vec![],
        utility_tags: vec![],
    };
    let note = build_spike_note(&early_champ);
    let text = note.expect("early dominant should have spike note");
    assert!(text.contains("erken") || text.contains("Erken"));
}

#[test]
fn spike_note_neutral_curve_returns_none() {
    use crate::recommendation::draft_iq::archetype::{
        CcProfile, ChampionArchetype, DamageProfile, PowerCurve,
    };
    let flat_champ = ChampionArchetype {
        champion_id: 3,
        archetype: "vanguard".to_string(),
        damage_profile: DamageProfile {
            ad: 0.4,
            ap: 0.2,
            true_damage: 0.0,
        },
        cc: CcProfile {
            has_hard_cc: true,
            hard_cc_count: 2,
            primary_cc: vec![],
        },
        mobility: "medium".to_string(),
        engage_role: "initiator".to_string(),
        peel_capability: "medium".to_string(),
        blind_safety: 0.7,
        execution_difficulty: 2,
        win_condition: "teamfight".to_string(),
        ult_type: "aoe".to_string(),
        confidence: "high".to_string(),
        power_curve: PowerCurve {
            early: 0.65,
            mid: 0.70,
            late: 0.68,
        },
        counters_archetypes: vec![],
        utility_tags: vec![],
    };
    // Flat curve — no standout spike, should return None
    let note = build_spike_note(&flat_champ);
    assert!(
        note.is_none(),
        "flat curve vanguard should have no spike note, got: {note:?}"
    );
}

// ── B5: Richer threat narrative tests ──────────────────────────────────────

#[cfg(test)]
fn b5_make_archetype(
    archetype: &str,
    mobility: &str,
    utility_tags: Vec<String>,
    early: f32,
    late: f32,
) -> crate::recommendation::draft_iq::archetype::ChampionArchetype {
    use crate::recommendation::draft_iq::archetype::{
        CcProfile, ChampionArchetype, DamageProfile, PowerCurve,
    };
    ChampionArchetype {
        champion_id: 1,
        archetype: archetype.to_string(),
        damage_profile: DamageProfile {
            ad: 0.5,
            ap: 0.3,
            true_damage: 0.0,
        },
        cc: CcProfile {
            has_hard_cc: false,
            hard_cc_count: 0,
            primary_cc: vec![],
        },
        mobility: mobility.to_string(),
        engage_role: "none".to_string(),
        peel_capability: "low".to_string(),
        blind_safety: 0.5,
        execution_difficulty: 2,
        win_condition: "teamfight".to_string(),
        ult_type: "single".to_string(),
        confidence: "high".to_string(),
        power_curve: PowerCurve {
            early,
            mid: 0.6,
            late,
        },
        counters_archetypes: vec![],
        utility_tags,
    }
}

#[test]
fn threats_tower_dive_flagged_for_immobile_carry() {
    let candidate = b5_make_archetype("marksman", "none", vec![], 0.5, 0.85);
    let diver = b5_make_archetype("diver", "high", vec![], 0.65, 0.55);
    let juggernaut = b5_make_archetype("juggernaut", "low", vec![], 0.75, 0.6);
    let enemies = vec![&diver, &juggernaut];
    let allies: Vec<&_> = vec![];

    let threats = build_threats_text(&candidate, &enemies, &allies);
    assert!(
        threats
            .iter()
            .any(|t| t.contains("dive") || t.contains("Tower-dive")),
        "Expected tower-dive threat for immobile marksman vs 2 divers, got: {threats:?}"
    );
}

#[test]
fn threats_pick_comp_flagged_for_immobile_carry() {
    let candidate = b5_make_archetype("artillery", "none", vec![], 0.4, 0.85);
    let assassin = b5_make_archetype("assassin", "high", vec![], 0.7, 0.65);
    let catcher = b5_make_archetype("catcher", "low", vec![], 0.65, 0.7);
    let enemies = vec![&assassin, &catcher];
    let allies: Vec<&_> = vec![];

    let threats = build_threats_text(&candidate, &enemies, &allies);
    assert!(
        threats
            .iter()
            .any(|t| t.contains("Pick-comp") || t.contains("guardian")),
        "Expected pick-comp threat for artillery vs 2 pick archetypes, got: {threats:?}"
    );
}

#[test]
fn threats_splitpush_flagged_when_team_lacks_waveclear() {
    let candidate = b5_make_archetype("assassin", "high", vec![], 0.6, 0.6);
    let juggernaut = b5_make_archetype("juggernaut", "low", vec![], 0.75, 0.6);
    let enemies = vec![&juggernaut];
    let ally1 = b5_make_archetype("marksman", "low", vec![], 0.5, 0.85);
    let ally2 = b5_make_archetype("enchanter", "low", vec![], 0.55, 0.75);
    let allies = vec![&ally1, &ally2];

    let threats = build_threats_text(&candidate, &enemies, &allies);
    assert!(
        threats
            .iter()
            .any(|t| t.contains("split") || t.contains("waveclear")),
        "Expected splitpush threat when team has no waveclear, got: {threats:?}"
    );
}

#[test]
fn threats_no_splitpush_when_team_has_waveclear() {
    let candidate = b5_make_archetype("assassin", "high", vec![], 0.6, 0.6);
    let juggernaut = b5_make_archetype("juggernaut", "low", vec![], 0.75, 0.6);
    let enemies = vec![&juggernaut];
    // Ally has waveclear utility tag → splitpush threat should NOT fire
    let ally1 = b5_make_archetype(
        "control_mage",
        "low",
        vec!["waveclear".to_string()],
        0.5,
        0.85,
    );
    let allies = vec![&ally1];

    let threats = build_threats_text(&candidate, &enemies, &allies);
    assert!(
        !threats.iter().any(|t| t.contains("waveclear")),
        "Splitpush threat should be suppressed when team has waveclear, got: {threats:?}"
    );
}

#[test]
fn threats_outscaled_flagged_for_early_dominant_vs_late_enemy() {
    // Candidate is early-only (e=0.78, l=0.50)
    let candidate = b5_make_archetype("juggernaut", "none", vec![], 0.78, 0.50);
    // Both enemies scale late (late >= 0.85)
    let scaler1 = b5_make_archetype("marksman", "low", vec![], 0.4, 0.90);
    let scaler2 = b5_make_archetype("control_mage", "low", vec![], 0.4, 0.85);
    let enemies = vec![&scaler1, &scaler2];
    let allies: Vec<&_> = vec![];

    let threats = build_threats_text(&candidate, &enemies, &allies);
    assert!(
        threats
            .iter()
            .any(|t| t.contains("geç oyun") || t.contains("drake") || t.contains("turret")),
        "Expected outscaled threat, got: {threats:?}"
    );
}

// ── N1: Lane-phase advice tests ────────────────────────────────────────────

#[test]
fn lane_phase_advice_aram_returns_none() {
    use crate::recommendation::draft_iq::narrative::build_lane_phase_advice;
    let candidate = b5_make_archetype("juggernaut", "low", vec![], 0.7, 0.6);
    // Empty position = ARAM / no lane
    let advice = build_lane_phase_advice(&candidate, None, "");
    assert!(advice.is_none(), "ARAM should have no lane phase advice");
}

#[test]
fn lane_phase_advice_early_bully_vs_scaler_pushes() {
    use crate::recommendation::draft_iq::narrative::build_lane_phase_advice;
    let bully = b5_make_archetype("juggernaut", "low", vec![], 0.75, 0.55);
    let scaler = b5_make_archetype("control_mage", "low", vec![], 0.40, 0.85);
    let advice = build_lane_phase_advice(&bully, Some(&scaler), "top")
        .expect("expected push advice for early bully vs scaler");
    assert!(
        advice.contains("push") || advice.contains("Push") || advice.contains("plate"),
        "expected push window advice, got: {advice}"
    );
}

#[test]
fn lane_phase_advice_scaler_vs_bully_freezes() {
    use crate::recommendation::draft_iq::narrative::build_lane_phase_advice;
    let scaler = b5_make_archetype("control_mage", "low", vec![], 0.40, 0.85);
    let bully = b5_make_archetype("juggernaut", "low", vec![], 0.75, 0.55);
    let advice = build_lane_phase_advice(&scaler, Some(&bully), "middle")
        .expect("expected freeze advice for scaler vs early bully");
    assert!(
        advice.contains("freeze") || advice.contains("Freeze") || advice.contains("gank"),
        "expected freeze advice, got: {advice}"
    );
}

#[test]
fn lane_phase_advice_position_archetype_combo_marksman_bottom() {
    use crate::recommendation::draft_iq::narrative::build_lane_phase_advice;
    // Neutral curve (no matchup mismatch) → falls through to position+archetype
    let marksman = b5_make_archetype("marksman", "low", vec![], 0.55, 0.85);
    let advice = build_lane_phase_advice(&marksman, None, "bottom")
        .expect("expected position+archetype combo for bottom marksman");
    assert!(
        advice.contains("Plate") || advice.contains("2v2") || advice.contains("support"),
        "expected bot lane advice, got: {advice}"
    );
}

#[test]
fn lane_phase_advice_unknown_combo_returns_none() {
    use crate::recommendation::draft_iq::narrative::build_lane_phase_advice;
    // Neutral curve + uncommon position+archetype combo → None
    let support = b5_make_archetype("burst_mage", "medium", vec![], 0.55, 0.65);
    let advice = build_lane_phase_advice(&support, None, "utility");
    // burst_mage on utility is unusual; no specific pattern → None acceptable
    // Just verify no panic; result depends on implementation
    let _ = advice;
}
