use std::collections::{HashMap, HashSet};

use super::build_advisor::{situational_item_ids, suggest_rune_tree};
use super::champion_types::ChampionType;
use super::draft_iq::{
    analyzer::{analyze_pick, AnalyzerInput},
    archetype::ChampionArchetype,
    DraftKnowledgeBase,
};
use super::models::{Recommendation, Tier};
use super::scoring::{
    aram_utility_bonus, comfort_score, matchup_score, meta_score, phase_advantage, risk_score,
    role_fit_score, synergy_score, team_counter_score, ScoringContext,
};
use super::team_analysis::TeamComposition;
use crate::db::champion_repo::ChampionRecord;
use crate::ddragon::cdragon::{ItemData, RuneTree};

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

    // ── 2. Team compositions (once, outside the loop) ────────────────────────
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

    let my_pos = ctx.session.local_player.assigned_position.as_str();
    let enemy_laner_name: Option<String> = ctx
        .session
        .their_team
        .iter()
        .find(|s| s.assigned_position == my_pos && s.champion_id > 0)
        .and_then(|s| {
            all_champions
                .iter()
                .find(|c| c.champion_id == s.champion_id as i64)
        })
        .map(|c| c.name.clone());

    // ── 3. Draft IQ pre-computation (outside the loop) ───────────────────────
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

    let ally_archetypes_for_kb: Vec<&ChampionArchetype> = ally_id_keys_for_kb
        .iter()
        .filter_map(|(_, k)| kb.get_archetype(k))
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

    // Opponent laner id — used to compute per-phase matchup advantage per candidate.
    let opp_id: u32 = ctx
        .session
        .their_team
        .iter()
        .find(|s| s.assigned_position.to_lowercase() == my_pos.to_lowercase() && s.champion_id > 0)
        .map(|s| s.champion_id)
        .unwrap_or(0);

    // Lane opponent's KB archetype — used by analyzer for lane-phase advice.
    let lane_opponent_kb: Option<&ChampionArchetype> = if opp_id > 0 {
        all_champions
            .iter()
            .find(|c| c.champion_id == opp_id as i64)
            .and_then(|c| kb.get_archetype(&c.key))
    } else {
        None
    };

    let w = &ctx.weights;

    // ── 4. Score every non-excluded champion ─────────────────────────────────
    let mut combo_bonuses: HashMap<u32, f32> = HashMap::new();

    let mut scored: Vec<Recommendation> = all_champions
        .iter()
        .filter(|c| !picked.contains(&(c.champion_id as u32)))
        .map(|record| {
            let id = record.champion_id as u32;

            // Base component scores (unchanged from pre-DI-4b)
            let c = comfort_score(id, ctx);
            let mu = matchup_score(id, ctx);
            let tc_base = team_counter_score(id, ctx);
            let sy_base = synergy_score(id, ctx);
            let mt = meta_score(id, ctx);
            let rf = role_fit_score(id, ctx);
            let rk_base = risk_score(id, ctx);

            // Draft IQ enrichment — skipped for ARAM (combo logic meaningless there)
            let (combo_bonus, analysis_opt) = if !is_aram {
                if let Some(archetype) = kb.get_archetype(&record.key) {
                    let input = AnalyzerInput {
                        candidate_key: &record.key,
                        candidate: archetype,
                        ally_id_keys: ally_id_keys_for_kb.clone(),
                        ally_archetypes: ally_archetypes_for_kb.clone(),
                        enemy_archetypes: enemy_archetypes_for_kb.clone(),
                        combo_dir: &kb.combos,
                        is_first_pick,
                        is_late_pick,
                        pick_order: ctx.session.pick_order,
                        position: my_pos,
                        lane_opponent: lane_opponent_kb,
                    };
                    let result = analyze_pick(&input);
                    let cb = result.combo_bonus;
                    (cb, Some(result))
                } else {
                    (0.0, None)
                }
            } else {
                (0.0, None)
            };

            combo_bonuses.insert(id, combo_bonus);

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
            let rk = analysis_opt
                .as_ref()
                .map(|r| (rk_base + r.blind_unsafety).min(0.10))
                .unwrap_or(rk_base);

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

            // ARAM/Arena: apply utility-tag bonus (±0.20) from KB archetype.
            // No-op for non-brawl queues or champions missing KB data.
            let total = if is_aram {
                let bonus = kb
                    .get_archetype(&record.key)
                    .map(aram_utility_bonus)
                    .unwrap_or(0.0);
                (base_total + bonus).clamp(0.0, 1.0)
            } else {
                base_total
            };

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
                enemy_laner_name.as_deref(),
                &enemy_comp,
                &ally_comp,
                &champ_types,
            );

            let situational = situational_item_ids(&enemy_comp, items);
            let (rune_tree_id, keystone_id) = suggest_rune_tree(&champ_types, rune_trees);

            let stat_row = ctx.stats.iter().find(|s| s.champion_id as u32 == id);
            let games = stat_row.map(|s| s.games).unwrap_or(0);
            let wins = stat_row.map(|s| s.wins).unwrap_or(0);
            let confidence = match games {
                0..=2 => "low".to_string(),
                3..=10 => "medium".to_string(),
                _ => "high".to_string(),
            };

            let draft_plan = analysis_opt.map(|r| r.plan);

            let phase_matchup = if opp_id > 0 {
                let phases = phase_advantage(id, opp_id, ctx);
                let all_neutral = phases.iter().all(|&v| (v - 0.5).abs() < 0.001);
                if all_neutral {
                    None
                } else {
                    Some(phases)
                }
            } else {
                None
            };

            Recommendation {
                champion_id: id,
                champion_key: record.key.clone(),
                champion_name: record.name.clone(),
                total_score: total,
                comfort_score: c,
                matchup_score: mu,
                team_counter_score: tc,
                synergy_score: sy,
                meta_score: mt,
                role_fit_score: rf,
                risk_score: rk,
                reason,
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
                games_on_champ: games,
                wins_on_champ: wins,
                enemy_team_summary: enemy_summary.clone(),
                draft_plan,
                phase_matchup,
            }
        })
        .collect();

    // ── 5. Sort and apply stretch pick gate ──────────────────────────────────
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
