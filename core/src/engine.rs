use std::collections::{HashMap, HashSet};

use super::build_advisor::{situational_item_ids, suggest_rune_tree};
use super::champion_types::{archetype_position_fit, flex_positions, ChampionType};
use super::draft_iq::{
    analyzer::{analyze_pick, AnalyzerInput},
    archetype::ChampionArchetype,
    DraftKnowledgeBase,
};
use super::feedback_signal::apply_feedback;
use super::models::{DataSourceBadge, Recommendation, Tier};
use super::scoring::{
    aram_utility_bonus, comfort_score, draft_winrate_signal, is_hard_role_mismatch, matchup_score,
    mechanical_comfort, meta_score, phase_advantage, resolve_lane_opponent, risk_score,
    role_fit_score, synergy_score, team_counter_score, ScoringContext,
};
use super::team_analysis::TeamComposition;
use crate::types::ChampionRecord;
use crate::types::{ItemData, RuneTree};

/// Compute top-5 recommendations given the current champ-select state.
///
/// Candidate pool: **all champions in `all_champions`** (no longer restricted to
/// mastery/stats). A comfort gate keeps low-comfort picks out of the top-5 unless
/// they have a strong, known combo with an ally (combo_bonus ≥ 0.80, max 1 stretch
/// pick per result set).
pub fn compute_recommendations(
    ctx: &ScoringContext<'_>,
    all_champions: &[ChampionRecord],
    items: &[ItemData],
    rune_trees: &[RuneTree],
    kb: &DraftKnowledgeBase,
) -> Vec<Recommendation> {
    // ── 1. Exclusion set: already picked or banned champions ─────────────────
    let picked: HashSet<u32> = ctx
        .session
        .my_team
        .iter()
        .chain(ctx.session.their_team.iter())
        .filter(|s| s.champion_id > 0)
        .map(|s| s.champion_id)
        .chain(ctx.session.my_bans.iter().copied())
        .chain(ctx.session.their_bans.iter().copied())
        .collect();

    // Candidate-independent context (team comps, archetypes, pick-order flags).
    let pre = precompute(ctx, all_champions, kb);

    // Hard role filter: when the lane is known (non-ARAM, assigned position present),
    // a champion is only a candidate if it ACTUALLY fits the lane — real meta data
    // showing it's played there (sample-gated, so off-role trolls don't count), or
    // the KB archetype plausibly fits. This is what stops e.g. a marksman / jungler
    // being recommended for support just because the player has mastery on it.
    let role_known = !pre.is_aram && !pre.my_pos.is_empty();
    let fits_role = |c: &ChampionRecord| -> bool {
        if !role_known {
            return true;
        }
        let id = c.champion_id as u32;
        let meta_present = ctx
            .meta_rates
            .get(&(id, pre.my_pos.clone()))
            .is_some_and(|r| r.sample_size >= ROLE_FIT_MIN_SAMPLE);
        meta_present
            || flex_positions(id).contains(&pre.my_pos.as_str())
            || kb
                .get_archetype(&c.key)
                .is_none_or(|a| archetype_position_fit(&a.archetype, &pre.my_pos))
    };

    // ── 2. Score every non-excluded, role-fitting champion ───────────────────
    let mut combo_bonuses: HashMap<u32, f32> = HashMap::new();
    let mut scored: Vec<Recommendation> = all_champions
        .iter()
        .filter(|c| !picked.contains(&(c.champion_id as u32)) && fits_role(c))
        .map(|record| {
            let (rec, combo_bonus) = score_candidate(record, ctx, &pre, items, rune_trees, kb);
            combo_bonuses.insert(rec.champion_id, combo_bonus);
            rec
        })
        .collect();

    // ── 3. Sort and apply stretch pick gate ──────────────────────────────────
    scored.sort_by(|a, b| {
        b.total_score
            .partial_cmp(&a.total_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut result: Vec<Recommendation> = Vec::with_capacity(5);
    let mut stretch_count = 0usize;

    for mut rec in scored {
        if result.len() >= 5 {
            break;
        }
        let is_stretch = rec.comfort_score < 0.10;
        if is_stretch {
            let cb = combo_bonuses.get(&rec.champion_id).copied().unwrap_or(0.0);
            // Only allow one stretch pick, and only with strong combo backing
            if cb < 0.80 || stretch_count >= 1 {
                continue;
            }
            if let Some(ref mut plan) = rec.draft_plan {
                let games = rec.games_on_champ;
                let wins = rec.wins_on_champ;
                let note = if games == 0 {
                    "0 maç — kombo'ya bağımlı".to_string()
                } else {
                    let losses = games - wins;
                    let wr = (wins as f32 / games as f32 * 100.0).round() as u32;
                    format!("{wins}G-{losses}L (%{wr}) — düşük deneyim")
                };
                plan.risk_note = Some(note);
            }
            rec.confidence = "low".to_string();
            stretch_count += 1;
        }
        result.push(rec);
    }

    result
}

/// Demotion factor applied to a hard off-role candidate's final score in
/// non-brawl queues. 0.8 = a 20% penalty — enough to push a clearly off-role
/// pick (e.g. a pure tank for `middle`) out of the top-5 without burying a
/// high-comfort flex pick entirely.
const ROLE_MISMATCH_FACTOR: f32 = 0.8;

/// Minimum blended sample for meta data to count as "actually played in this lane"
/// in the hard role filter. Below this, an off-role row (a handful of troll games on
/// e.g. ADC Olaf) does not qualify a champion for the lane.
const ROLE_FIT_MIN_SAMPLE: u32 = 100;

fn sample_confidence(sample_size: u32) -> &'static str {
    match sample_size {
        0..=99 => "low",
        100..=499 => "medium",
        _ => "high",
    }
}

/// Recommendation confidence from sample depth + signal convergence. Answers "how
/// much real data backs this and do the signals agree", NOT "how likely to win" —
/// deliberately distinct from the score. Returns `(confidence, basis)`.
///
/// Downgrade-only relative to the old games-only buckets: a deep, agreeing sample
/// stays "high"; conflicting signals (a wide spread among the positive drivers) or
/// no real matchup backing on a shallow sample pull it down. It never upgrades past
/// what the sample supports.
fn compute_confidence(
    games: u32,
    comfort: f32,
    matchup: f32,
    meta: f32,
    synergy: f32,
    matchup_confidence: &str,
) -> (String, String) {
    let depth_tier: u8 = match games {
        0..=2 => 0,
        3..=10 => 1,
        _ => 2,
    };

    // Spread among the meaningful positive drivers. A wide gap means the total
    // leans on a single signal rather than several agreeing ones.
    let drivers = [comfort, matchup, meta, synergy];
    let max = drivers.iter().copied().fold(f32::MIN, f32::max);
    let min = drivers.iter().copied().fold(f32::MAX, f32::min);
    let conflicting = (max - min) > 0.50;
    let thin_matchup = matches!(matchup_confidence, "none" | "heuristic");

    let mut tier = depth_tier;
    let mut basis = "sample_depth";
    if conflicting && tier > 0 {
        tier -= 1;
        basis = "signal_convergence";
    }
    if thin_matchup && depth_tier == 0 {
        tier = 0;
        if basis == "sample_depth" {
            basis = "signal_convergence";
        }
    }
    // No personal games AND no driver signal at all → purely a cold read.
    if games == 0 && drivers.iter().all(|&d| d <= 0.0) {
        basis = "games_only";
    }

    let confidence = match tier {
        0 => "low",
        1 => "medium",
        _ => "high",
    };
    (confidence.to_string(), basis.to_string())
}

fn source_badge(
    source: &str,
    patch: Option<String>,
    sample_size: Option<u32>,
    confidence: &str,
    risk_level: &str,
) -> DataSourceBadge {
    DataSourceBadge {
        source: source.to_string(),
        patch,
        sample_size,
        confidence: confidence.to_string(),
        risk_level: risk_level.to_string(),
        updated_at: None,
    }
}

fn matchup_confidence_for(champion_id: u32, opponent_id: u32, ctx: &ScoringContext<'_>) -> String {
    if opponent_id == 0 {
        return "none".to_string();
    }
    ctx.matchups
        .and_then(|m| m.get(&(champion_id, opponent_id)))
        .map(|entry| sample_confidence(entry.games).to_string())
        .unwrap_or_else(|| "heuristic".to_string())
}

fn build_trust_badges(
    champion_id: u32,
    opponent_id: u32,
    my_pos: &str,
    games_on_champ: u32,
    ctx: &ScoringContext<'_>,
) -> Vec<DataSourceBadge> {
    let mut badges = vec![source_badge("draft_iq", None, Some(109), "high", "low")];

    if games_on_champ > 0 {
        badges.push(source_badge(
            "local_profile",
            None,
            Some(games_on_champ),
            sample_confidence(games_on_champ),
            "low",
        ));
    } else {
        badges.push(source_badge(
            "local_profile",
            None,
            Some(0),
            "low",
            "medium",
        ));
    }

    if let Some(rate) = ctx.meta_rates.get(&(champion_id, my_pos.to_lowercase())) {
        badges.push(source_badge(
            "meraki_meta",
            None,
            Some(rate.sample_size),
            sample_confidence(rate.sample_size),
            "low",
        ));
    } else {
        badges.push(source_badge("meta_fallback", None, None, "low", "medium"));
    }

    if opponent_id > 0 {
        if let Some(entry) = ctx
            .matchups
            .and_then(|m| m.get(&(champion_id, opponent_id)))
        {
            badges.push(source_badge(
                "matchup_seed",
                None,
                Some(entry.games),
                sample_confidence(entry.games),
                "low",
            ));
        } else {
            badges.push(source_badge(
                "matchup_heuristic",
                None,
                None,
                "medium",
                "medium",
            ));
        }
    }

    badges
}

/// Candidate-independent state computed once per scoring pass. Champion
/// archetype references borrow from the KB (`'a`); all other fields are owned so
/// the struct outlives the borrowed `ScoringContext`.
struct Precomputed<'a> {
    enemy_comp: TeamComposition,
    ally_comp: TeamComposition,
    enemy_summary: String,
    enemy_laner_name: Option<String>,
    is_aram: bool,
    is_first_pick: bool,
    is_late_pick: bool,
    /// `(champion_id, key)` for every ally with a champion locked/picked.
    ally_id_keys_for_kb: Vec<(u32, String)>,
    enemy_archetypes_for_kb: Vec<&'a ChampionArchetype>,
    /// Local player's assigned LCU position (raw, as the engine used it before).
    my_pos: String,
    /// Opponent laner champion id (0 = not visible).
    opp_id: u32,
    lane_opponent_kb: Option<&'a ChampionArchetype>,
}

/// Compute the shared, candidate-independent context. Used by both
/// `compute_recommendations` (the scoring loop) and `analyze_champion` (a single
/// locked/hovered champion).
fn precompute<'a>(
    ctx: &ScoringContext<'_>,
    all_champions: &[ChampionRecord],
    kb: &'a DraftKnowledgeBase,
) -> Precomputed<'a> {
    let enemy_ids: Vec<u32> = ctx
        .session
        .their_team
        .iter()
        .filter(|s| s.champion_id > 0)
        .map(|s| s.champion_id)
        .collect();
    let enemy_comp = TeamComposition::from_champion_ids(&enemy_ids, ctx.role_map);
    let enemy_summary = enemy_comp.summary_text();

    let ally_ids: Vec<u32> = ctx
        .session
        .my_team
        .iter()
        .filter(|s| s.champion_id > 0)
        .map(|s| s.champion_id)
        .collect();
    let ally_comp = TeamComposition::from_champion_ids(&ally_ids, ctx.role_map);

    let my_pos = ctx.session.local_player.assigned_position.clone();

    // Lane opponent: exact same-position, or a single inferred laner when LCU
    // omits enemy positions (Blind/Normal). Shared by enemy_laner_name + opp_id.
    let opp_id: u32 = resolve_lane_opponent(ctx).map(|(id, _)| id).unwrap_or(0);
    let enemy_laner_name: Option<String> = if opp_id > 0 {
        all_champions
            .iter()
            .find(|c| c.champion_id == opp_id as i64)
            .map(|c| c.name.clone())
    } else {
        None
    };

    // `is_aram` covers all brawl modes (ARAM=450, Arena/Hexakill=1700) — they
    // share the no-lane property that disables matchup/role_fit scoring and
    // analyzer combo detection.
    let is_aram = matches!(ctx.session.queue_id, 450 | 1700);
    // Use parsed pick_order when available; fall back to the "no enemy picks yet" heuristic.
    let is_first_pick = if ctx.session.pick_order > 0 {
        ctx.session.pick_order == 1
    } else {
        ctx.session.their_team.iter().all(|s| s.champion_id == 0)
    };
    let is_late_pick = !is_aram && ctx.session.pick_order >= 4;

    let ally_id_keys_for_kb: Vec<(u32, String)> = ally_ids
        .iter()
        .filter_map(|&aid| {
            all_champions
                .iter()
                .find(|c| c.champion_id == aid as i64)
                .map(|c| (aid, c.key.clone()))
        })
        .collect();

    let enemy_archetypes_for_kb: Vec<&ChampionArchetype> = enemy_ids
        .iter()
        .filter_map(|&eid| {
            all_champions
                .iter()
                .find(|c| c.champion_id == eid as i64)
                .and_then(|c| kb.get_archetype(&c.key))
        })
        .collect();

    // Lane opponent's KB archetype — used by analyzer for lane-phase advice.
    let lane_opponent_kb: Option<&ChampionArchetype> = if opp_id > 0 {
        all_champions
            .iter()
            .find(|c| c.champion_id == opp_id as i64)
            .and_then(|c| kb.get_archetype(&c.key))
    } else {
        None
    };

    Precomputed {
        enemy_comp,
        ally_comp,
        enemy_summary,
        enemy_laner_name,
        is_aram,
        is_first_pick,
        is_late_pick,
        ally_id_keys_for_kb,
        enemy_archetypes_for_kb,
        my_pos,
        opp_id,
        lane_opponent_kb,
    }
}

/// Score one candidate champion into a `Recommendation`, returning it alongside
/// its combo bonus (used by the caller for the stretch-pick gate). Pure — no I/O.
/// Build/item fields are left empty; the command layer enriches them afterwards.
fn score_candidate(
    record: &ChampionRecord,
    ctx: &ScoringContext<'_>,
    pre: &Precomputed<'_>,
    items: &[ItemData],
    rune_trees: &[RuneTree],
    kb: &DraftKnowledgeBase,
) -> (Recommendation, f32) {
    let id = record.champion_id as u32;
    let w = &ctx.weights;

    // Base component scores
    let c = comfort_score(id, ctx);
    // Honest decomposition of comfort for the UI — these do NOT affect total_score,
    // they just let the card show mechanical mastery and draft win-rate separately.
    let mechanical = mechanical_comfort(id, ctx);
    let draft_wr = draft_winrate_signal(id, ctx);
    let mu = matchup_score(id, ctx);
    let tc_base = team_counter_score(id, ctx);
    let sy_base = synergy_score(id, ctx);
    let mt = meta_score(id, ctx);
    let rf = role_fit_score(id, ctx);
    let rk_base = risk_score(id, ctx);

    // Ally lists for the analyzer, excluding the candidate itself. The exclusion
    // matters only when scoring an already-locked ally (analyze_champion); in the
    // recommendation loop the candidate is never among the allies.
    let ally_id_keys: Vec<(u32, String)> = pre
        .ally_id_keys_for_kb
        .iter()
        .filter(|(aid, _)| *aid != id)
        .cloned()
        .collect();
    let ally_archetypes: Vec<&ChampionArchetype> = ally_id_keys
        .iter()
        .filter_map(|(_, k)| kb.get_archetype(k))
        .collect();

    // Draft IQ enrichment — ARAM'da da çalışır (E4): combo + team-need + damage
    // balance sabit 5v5 teamfight'ta tam anlamlı. Lane-merkezli draft_plan
    // prose'u ile blind-safety risk eki ise ARAM'da uygulanmaz (aşağıda) —
    // koridor tavsiyesi ARAM'da yanıltıcı olur.
    let (combo_bonus, analysis_opt) = if let Some(archetype) = kb.get_archetype(&record.key) {
        let input = AnalyzerInput {
            candidate_key: &record.key,
            candidate: archetype,
            ally_id_keys: ally_id_keys.clone(),
            ally_archetypes: ally_archetypes.clone(),
            enemy_archetypes: pre.enemy_archetypes_for_kb.clone(),
            combo_dir: &kb.combos,
            is_first_pick: pre.is_first_pick,
            is_late_pick: pre.is_late_pick,
            pick_order: ctx.session.pick_order,
            position: pre.my_pos.as_str(),
            lane_opponent: pre.lane_opponent_kb,
        };
        let result = analyze_pick(&input);
        let cb = result.combo_bonus;
        (cb, Some(result))
    } else {
        (0.0, None)
    };

    // KB-enriched component scores
    // synergy_score is boosted by both combo strength and team-need fill score
    let sy = analysis_opt
        .as_ref()
        .map(|r| {
            sy_base
                .max(r.combo_bonus)
                .max(r.team_need_score * 0.6)
                .min(1.0)
        })
        .unwrap_or(sy_base);
    let tc = analysis_opt
        .as_ref()
        .map(|r| ((tc_base + r.damage_balance) / 2.0).min(1.0))
        .unwrap_or(tc_base);
    // Blind-safety risk eki ARAM'da anlamsız (herkes rastgele) → yalnız SR'da.
    let rk = if pre.is_aram {
        rk_base
    } else {
        analysis_opt
            .as_ref()
            .map(|r| (rk_base + r.blind_unsafety).min(0.10))
            .unwrap_or(rk_base)
    };

    let pos_sum = w.comfort + w.matchup + w.team_counter + w.synergy + w.meta + w.role_fit;
    let base_total = if pos_sum > 0.0 {
        ((w.comfort * c
            + w.matchup * mu
            + w.team_counter * tc
            + w.synergy * sy
            + w.meta * mt
            + w.role_fit * rf)
            / pos_sum
            - w.risk * rk)
            .clamp(0.0, 1.0)
    } else {
        ((c + mu + tc + sy + mt + rf) / 6.0 - 0.05 * rk).clamp(0.0, 1.0)
    };

    // ARAM/Arena: utility-tag bonus (±0.20) from KB archetype.
    // Non-brawl: demote hard off-role picks so they can't rank top-5 — but exempt
    // champions whose KB archetype plausibly fits the lane (e.g. a mage support,
    // an APC bot) so valid off-meta-role picks aren't wrongly buried.
    let total = if pre.is_aram {
        let bonus = kb
            .get_archetype(&record.key)
            .map(aram_utility_bonus)
            .unwrap_or(0.0);
        (base_total + bonus).clamp(0.0, 1.0)
    } else {
        let archetype_fits = kb
            .get_archetype(&record.key)
            .map(|a| archetype_position_fit(&a.archetype, &pre.my_pos))
            .unwrap_or(false);
        if is_hard_role_mismatch(id, ctx) && !archetype_fits {
            base_total * ROLE_MISMATCH_FACTOR
        } else {
            base_total
        }
    };
    let total = ctx
        .feedback_signals
        .and_then(|signals| signals.get(&id))
        .map(|signal| apply_feedback(total, signal).clamp(0.0, 1.0))
        .unwrap_or(total);

    let champ_types: Vec<ChampionType> = ctx
        .role_map
        .get(&id)
        .map(|r| ChampionType::from_roles(r))
        .unwrap_or_default();

    let reason = build_reason(
        c,
        mu,
        tc,
        sy,
        mt,
        rf,
        pre.enemy_laner_name.as_deref(),
        &pre.enemy_comp,
        &pre.ally_comp,
        &champ_types,
    );

    let situational = situational_item_ids(&pre.enemy_comp, items);
    let (rune_tree_id, keystone_id) = suggest_rune_tree(&champ_types, rune_trees);

    let stat_row = ctx.stats.iter().find(|s| s.champion_id as u32 == id);
    let games = stat_row.map(|s| s.games).unwrap_or(0);
    let wins = stat_row.map(|s| s.wins).unwrap_or(0);
    // ARAM'da lane-merkezli plan prose'u atlanır (combo/team-need SKORU kaldı).
    let draft_plan = if pre.is_aram {
        None
    } else {
        analysis_opt.map(|r| r.plan)
    };
    let lane_plan = draft_plan
        .as_ref()
        .and_then(|p| p.lane_phase_advice.clone());
    let teamfight_plan = draft_plan.as_ref().and_then(|p| {
        if p.team_role.trim().is_empty() && p.win_condition.trim().is_empty() {
            None
        } else if p.team_role.trim().is_empty() {
            Some(p.win_condition.clone())
        } else if p.win_condition.trim().is_empty() {
            Some(p.team_role.clone())
        } else {
            Some(format!("{}: {}", p.team_role, p.win_condition))
        }
    });
    let risk_summary = draft_plan
        .as_ref()
        .and_then(|p| p.risk_note.clone().or_else(|| p.threats.first().cloned()))
        .or_else(|| {
            if rk >= 0.08 {
                Some(
                    "Yüksek ban/sample riski — kilitlemeden önce matchup'ı tekrar kontrol et"
                        .to_string(),
                )
            } else {
                None
            }
        });
    let mut why_not = Vec::new();
    if c < 0.10 {
        why_not
            .push("Konfor sinyali zayıf; mekanik veya matchup hatası pahalı olabilir".to_string());
    }
    if rf < 0.30 && !pre.is_aram {
        why_not.push("Rol uyumu düşük; off-role tempo kaybı yaratabilir".to_string());
    }
    if mt < 0.35 {
        why_not.push(
            "Canlı meta sinyali zayıf veya nötr; pick gücü daha çok kompozisyona bağlı".to_string(),
        );
    }
    if rk >= 0.08 {
        why_not
            .push("Risk skoru yüksek; düşük örneklem veya yüksek ban oranı olabilir".to_string());
    }
    let matchup_confidence = matchup_confidence_for(id, pre.opp_id, ctx);
    let (confidence, confidence_basis) =
        compute_confidence(games, c, mu, mt, sy, &matchup_confidence);
    let data_sources = build_trust_badges(id, pre.opp_id, &pre.my_pos, games, ctx);

    let phase_matchup = if pre.opp_id > 0 {
        let phases = phase_advantage(id, pre.opp_id, ctx);
        let all_neutral = phases.iter().all(|&v| (v - 0.5).abs() < 0.001);
        if all_neutral {
            None
        } else {
            Some(phases)
        }
    } else {
        None
    };

    let rec = Recommendation {
        champion_id: id,
        champion_key: record.key.clone(),
        champion_name: record.name.clone(),
        total_score: total,
        comfort_score: c,
        mechanical_comfort: mechanical,
        draft_winrate: draft_wr,
        matchup_score: mu,
        team_counter_score: tc,
        synergy_score: sy,
        meta_score: mt,
        role_fit_score: rf,
        risk_score: rk,
        reason,
        headline: None,
        decision_sentence: String::new(),
        score_breakdown: Vec::new(),
        model_score: total,
        rules_score: total,
        model_version: "legacy-rules".to_string(),
        build_source: "none".to_string(),
        build_confidence: "none".to_string(),
        build_note: None,
        matchup_confidence,
        missing_signals: Vec::new(),
        pro_presence: None,
        lane_form_score: None,
        lane_plan,
        mid_game_plan: None,
        teamfight_plan,
        teamfight_job: None,
        fallback_plan: None,
        risk_summary,
        why_not,
        data_sources,
        coach_depth: "legacy".to_string(),
        core_items: Vec::new(),
        situational_items: situational,
        primary_rune_tree: rune_tree_id,
        keystone: keystone_id,
        skill_order: None,
        summoner_spells: Vec::new(),
        secondary_runes: Vec::new(),
        stat_shards: Vec::new(),
        tier: Tier::from_score(total),
        confidence,
        confidence_basis,
        games_on_champ: games,
        wins_on_champ: wins,
        enemy_team_summary: pre.enemy_summary.clone(),
        lane_opponent_name: pre.enemy_laner_name.clone(),
        core_item_name: None, // command resolves item names from the item cache
        draft_plan,
        phase_matchup,
    };
    (rec, combo_bonus)
}

/// Analyze a single, specific champion — typically the local player's locked or
/// hovered pick — returning its full `Recommendation` (score breakdown + draft
/// plan) while bypassing the picked/banned exclusion used by
/// `compute_recommendations`.
///
/// This powers post-lock coaching: once you lock a champion it is removed from
/// the recommendation pool, so the UI must request *that* champion's plan
/// explicitly instead of showing `recommendations[0]` (a different champion).
///
/// Returns `None` when `champion_id` is not present in `all_champions`. Build
/// fields are left empty — the command layer enriches them from `builds_repo`.
pub fn analyze_champion(
    champion_id: u32,
    ctx: &ScoringContext<'_>,
    all_champions: &[ChampionRecord],
    items: &[ItemData],
    rune_trees: &[RuneTree],
    kb: &DraftKnowledgeBase,
) -> Option<Recommendation> {
    let record = all_champions
        .iter()
        .find(|c| c.champion_id as u32 == champion_id)?;
    let pre = precompute(ctx, all_champions, kb);
    let (rec, _) = score_candidate(record, ctx, &pre, items, rune_trees, kb);
    Some(rec)
}

#[allow(clippy::too_many_arguments)]
fn build_reason(
    comfort: f32,
    matchup: f32,
    team_counter: f32,
    synergy: f32,
    meta: f32,
    role_fit: f32,
    enemy_laner_name: Option<&str>,
    enemy_comp: &TeamComposition,
    ally_comp: &TeamComposition,
    champ_types: &[ChampionType],
) -> String {
    let mut parts: Vec<String> = Vec::new();

    let komfor_label = if comfort > 0.70 {
        "yüksek"
    } else if comfort > 0.40 {
        "orta"
    } else {
        "düşük"
    };
    parts.push(format!("Komfor: {komfor_label}"));

    let best_counter = f32::max(matchup, team_counter);
    if best_counter > 0.50 {
        let counter_label = if matchup >= team_counter {
            match enemy_laner_name {
                Some(name) => format!("{name} karşı güçlü"),
                None => "lane sayacı".to_string(),
            }
        } else if enemy_comp.is_ap_heavy {
            "AP'ye karşı etkili".to_string()
        } else if enemy_comp.is_ad_heavy {
            "AD'ye karşı etkili".to_string()
        } else if enemy_comp.assassins >= 2 {
            "assassin sayacı".to_string()
        } else if enemy_comp.tanks >= 2 {
            "tank eritici".to_string()
        } else {
            "takım sayacı".to_string()
        };
        parts.push(format!("Kontr: {counter_label}"));
    }

    if synergy > 0.55 {
        if let Some(gap) = find_team_gap(ally_comp, champ_types) {
            parts.push(format!("Takım eksiği: {gap}"));
        }
    }

    if meta > 0.70 {
        parts.push("Meta güçlü".to_string());
    }

    if role_fit < 0.30 {
        parts.push("Rol dışı".to_string());
    }

    parts.join(" · ")
}

fn find_team_gap<'a>(ally: &TeamComposition, types: &[ChampionType]) -> Option<&'a str> {
    for t in types {
        match t {
            ChampionType::Tank if ally.tanks == 0 => return Some("tank"),
            ChampionType::Support if ally.supports == 0 => return Some("support"),
            ChampionType::Marksman if ally.marksmen == 0 => return Some("ADC"),
            ChampionType::Mage if ally.mages == 0 => return Some("mage"),
            _ => {}
        }
    }
    None
}
