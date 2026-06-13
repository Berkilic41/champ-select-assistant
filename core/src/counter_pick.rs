//! Counter-pick suggestions from the player's own champion pool.
//!
//! When the lane opponent is visible, this ranks the player's mastered champions
//! by how well they counter that opponent (reusing the existing `matchup_score`
//! heuristic + KB `counters_archetypes`). Pure — no I/O.

use super::draft_iq::DraftKnowledgeBase;
use super::scoring::{matchup_score, resolve_lane_opponent, ScoringContext};
use crate::types::ChampionRecord;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CounterPickHint {
    pub champion_id: u32,
    pub champion_key: String,
    pub champion_name: String,
    /// 0..1 — how well this pick counters the visible lane opponent (matchup_score).
    pub advantage: f32,
    pub reason: String,
    pub games_on_champ: u32,
    /// Power-curve advantage vs the opponent per phase `[early, mid, late]` (0.5 =
    /// even). Lets the UI say WHEN the counter is strongest. Neutral when unknown.
    pub phase_advantage: [f32; 3],
}

/// Threshold above the neutral 0.5 matchup midpoint required to surface a pick.
const COUNTER_THRESHOLD: f32 = 0.50;

/// Share of a phase that belongs to `a` vs `b` (0.5 even). Neutral when both ~0.
fn phase_adv(a: f32, b: f32) -> f32 {
    let s = a + b;
    if s < 0.01 {
        0.5
    } else {
        (a / s).clamp(0.0, 1.0)
    }
}

/// Rank the player's pool against the visible lane opponent. Returns the top
/// (≤3) counters. Empty when no lane opponent is visible or nothing in the pool
/// clears the neutral threshold. Excludes already picked/banned champions and
/// the opponent itself.
pub fn compute_counter_picks(
    ctx: &ScoringContext<'_>,
    all_champions: &[ChampionRecord],
    kb: &DraftKnowledgeBase,
    pool_ids: &[u32],
) -> Vec<CounterPickHint> {
    // Lane opponent: same assigned position, or a single inferred laner in
    // Blind/Normal where LCU omits enemy positions.
    let opponent_id = resolve_lane_opponent(ctx).map(|(id, _)| id).unwrap_or(0);
    if opponent_id == 0 {
        return vec![];
    }

    let opp_record = all_champions
        .iter()
        .find(|c| c.champion_id == opponent_id as i64);
    let opp_name = opp_record
        .map(|c| c.name.clone())
        .unwrap_or_else(|| "Rakip".to_string());
    let opp_archetype = opp_record
        .and_then(|c| kb.get_archetype(&c.key))
        .map(|a| a.archetype.clone());
    let opp_pc = opp_record
        .and_then(|c| kb.get_archetype(&c.key))
        .map(|a| a.power_curve.clone());

    // Already picked / banned champions can't be picked.
    let excluded: HashSet<u32> = ctx
        .session
        .my_team
        .iter()
        .chain(ctx.session.their_team.iter())
        .filter(|s| s.champion_id > 0)
        .map(|s| s.champion_id)
        .chain(ctx.session.my_bans.iter().copied())
        .chain(ctx.session.their_bans.iter().copied())
        .collect();

    let mut hints: Vec<CounterPickHint> = pool_ids
        .iter()
        .copied()
        .filter(|id| *id != opponent_id && !excluded.contains(id))
        .filter_map(|id| {
            let record = all_champions.iter().find(|c| c.champion_id == id as i64)?;
            let advantage = matchup_score(id, ctx);
            if advantage <= COUNTER_THRESHOLD {
                return None;
            }
            let cand_arch = kb.get_archetype(&record.key);
            // Highlight a hard archetype counter when the KB lists it.
            let counters_arch = match (&opp_archetype, cand_arch) {
                (Some(oa), Some(cand)) => cand.counters_archetypes.iter().any(|a| a == oa),
                _ => false,
            };
            // Per-phase power-curve advantage vs the opponent (neutral when unknown).
            let phase_advantage = match (cand_arch, &opp_pc) {
                (Some(c), Some(o)) => [
                    phase_adv(c.power_curve.early, o.early),
                    phase_adv(c.power_curve.mid, o.mid),
                    phase_adv(c.power_curve.late, o.late),
                ],
                _ => [0.5, 0.5, 0.5],
            };
            let reason = if counters_arch {
                format!("{opp_name} karşısında avantajlı — arketipini sayar")
            } else {
                format!("{opp_name} karşısında iyi eşleşme")
            };
            let stat = ctx.stats.iter().find(|s| s.champion_id as u32 == id);
            Some(CounterPickHint {
                champion_id: id,
                champion_key: record.key.clone(),
                champion_name: record.name.clone(),
                advantage,
                reason,
                games_on_champ: stat.map(|s| s.games).unwrap_or(0),
                phase_advantage,
            })
        })
        .collect();

    hints.sort_by(|a, b| {
        b.advantage
            .partial_cmp(&a.advantage)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hints.truncate(3);
    hints
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring::ScoringWeights;
    use crate::types::{ChampSelectState, TeamSlot};
    use std::collections::HashMap;

    fn champ(id: i64, key: &str) -> ChampionRecord {
        ChampionRecord {
            champion_id: id,
            key: key.to_string(),
            name: key.to_string(),
            title: String::new(),
        }
    }
    fn slot(cell: i32, champion_id: u32, pos: &str) -> TeamSlot {
        TeamSlot {
            cell_id: cell,
            champion_id,
            intent_champion_id: 0,
            assigned_position: pos.to_string(),
            is_locked: champion_id > 0,
        }
    }
    fn session(my_team: Vec<TeamSlot>, their_team: Vec<TeamSlot>) -> ChampSelectState {
        ChampSelectState {
            my_cell_id: 0,
            local_player: slot(0, 0, "middle"),
            my_team,
            their_team,
            my_bans: vec![],
            their_bans: vec![],
            phase: "BAN_PICK".into(),
            time_left_ms: 30_000,
            action_type: "pick".into(),
            queue_id: 420,
            pick_order: 0,
        }
    }

    use crate::scoring::MetaRate;
    fn ctx<'a>(
        s: &'a ChampSelectState,
        role_map: &'a HashMap<u32, Vec<String>>,
        meta: &'a HashMap<(u32, String), MetaRate>,
    ) -> ScoringContext<'a> {
        ScoringContext {
            session: s,
            mastery: &[],
            stats: &[],
            role_map,
            weights: ScoringWeights::default(),
            meta_rates: meta,
            matchups: None,
            power_curves: None,
            feedback_signals: None,
        }
    }

    #[test]
    fn assassin_pool_counters_mage_opponent() {
        let kb = DraftKnowledgeBase::load().expect("KB load");
        let all = vec![champ(103, "Ahri"), champ(238, "Zed"), champ(86, "Garen")];
        // Opponent Ahri (mage) at middle.
        let s = session(vec![], vec![slot(5, 103, "middle")]);
        let mut role_map: HashMap<u32, Vec<String>> = HashMap::new();
        role_map.insert(103, vec!["mage".into()]);
        role_map.insert(238, vec!["assassin".into()]);
        role_map.insert(86, vec!["fighter".into(), "tank".into()]);
        let meta = HashMap::new();
        let c = ctx(&s, &role_map, &meta);

        let picks = compute_counter_picks(&c, &all, &kb, &[238, 86]);
        // Zed (assassin, type_counter vs mage = 0.80) clears threshold; Garen does not.
        assert!(!picks.is_empty(), "assassin should counter mage opponent");
        assert_eq!(picks[0].champion_id, 238);
        assert!(picks[0].advantage > 0.5);
        // Phase advantage is computed per band and stays in range.
        assert!(
            picks[0]
                .phase_advantage
                .iter()
                .all(|&v| (0.0..=1.0).contains(&v)),
            "phase_advantage in range: {:?}",
            picks[0].phase_advantage
        );
        assert!(
            picks.iter().all(|p| p.champion_id != 86),
            "fighter shouldn't clear threshold vs mage"
        );
    }

    #[test]
    fn no_opponent_yields_empty() {
        let kb = DraftKnowledgeBase::load().expect("KB load");
        let all = vec![champ(238, "Zed")];
        let s = session(vec![], vec![]); // no enemy locked
        let role_map: HashMap<u32, Vec<String>> = HashMap::new();
        let meta = HashMap::new();
        let c = ctx(&s, &role_map, &meta);
        assert!(compute_counter_picks(&c, &all, &kb, &[238]).is_empty());
    }

    #[test]
    fn excludes_picked_pool_champion() {
        let kb = DraftKnowledgeBase::load().expect("KB load");
        let all = vec![champ(103, "Ahri"), champ(238, "Zed")];
        // Zed already picked by an ally; opponent Ahri mid.
        let s = session(vec![slot(1, 238, "top")], vec![slot(5, 103, "middle")]);
        let mut role_map: HashMap<u32, Vec<String>> = HashMap::new();
        role_map.insert(103, vec!["mage".into()]);
        role_map.insert(238, vec!["assassin".into()]);
        let meta = HashMap::new();
        let c = ctx(&s, &role_map, &meta);
        let picks = compute_counter_picks(&c, &all, &kb, &[238]);
        assert!(
            picks.iter().all(|p| p.champion_id != 238),
            "picked champ excluded from counters"
        );
    }
}
