//! Ban advisor — suggests up to 3 high-threat champions to ban during the
//! champ-select ban phase.
//!
//! Threat score formula (per candidate champion):
//! ```text
//!   threat = meta_component        * 0.40   (win_rate, when meta data present)
//!           + ban_rate_component   * 0.30   (ban_rate, when meta data present)
//!           + pool_counter_component * 0.30 (type counter vs our pool)
//!           + pool_weak_component  (+0.10)  (KB counters_archetypes ∩ our pool)
//!           + enemy_intent_component (+0.62)(enemy is hovering this champion)
//! ```
//!
//! - `meta_component` / `ban_rate_component` derive from the optional `MetaRate`
//!   (win_rate mapped 0.48→0.55, ban_rate 30 % → 1.0). These are the strongest
//!   signals when present, but the `champion_rates` table is empty offline (until
//!   the stats backend is wired), so the advisor must not depend on them.
//! - `pool_counter_component` averages `type_counter_score(candidate → mine)`
//!   across the player's top-mastery pool — candidates that COUNTER our pool.
//! - `pool_weak_component` fires when the candidate's KB `counters_archetypes`
//!   intersect the archetypes in our pool — it structurally beats what we play.
//! - `enemy_intent_component` fires when an enemy is currently hovering this
//!   champion (`championPickIntent`) — a direct "deny this pick" signal that
//!   works fully offline.
//!
//! Inclusion gate: a candidate is suggested only when it has at least one
//! concrete signal (meta data, enemy hover, KB pool-counter, or a strong
//! type-counter vs our pool). This keeps offline mode from ranking all ~170
//! champions on noise while still surfacing real threats without meta data.
//!
//! ARAM (queue_id 450): the caller passes `"aram"` as the lane key when fetching
//! meta_rates so this module needs no special-case branch.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use ts_rs::TS;

use crate::types::ChampionRecord;
use crate::types::MasteryRow;
use crate::types::ChampSelectState;
use crate::champion_types::{type_counter_score, ChampionType};
use crate::draft_iq::DraftKnowledgeBase;
use crate::scoring::{shrunk_meta_wr, MetaRate};

/// A single ban recommendation surfaced to the UI during the ban phase.
///
/// `threat_score` is in [0.0, 1.0] — higher = more threatening, ban first.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct BanSuggestion {
    pub champion_id: u32,
    pub champion_key: String,
    pub champion_name: String,
    pub threat_score: f32,
    /// Short Turkish rationale, e.g. "Tehlikeli · havuzunuza karşı güçlü · meta güçlü".
    pub reason: String,
}

/// Compute up to 3 ban suggestions, ranked by `threat_score` descending.
///
/// Excluded from the candidate pool:
/// - already-banned champions (both teams)
/// - already-picked champions (both teams)
/// - champions without a `meta_rates` row for the current lane (no signal)
pub fn compute_ban_suggestions(
    session: &ChampSelectState,
    all_champions: &[ChampionRecord],
    meta_rates: &HashMap<(u32, String), MetaRate>,
    my_pool: &[MasteryRow],
    role_map: &HashMap<u32, Vec<String>>,
    kb: &DraftKnowledgeBase,
) -> Vec<BanSuggestion> {
    // Exclude already banned / picked from both teams.
    let excluded: HashSet<u32> = session
        .my_bans
        .iter()
        .chain(session.their_bans.iter())
        .copied()
        .chain(
            session
                .my_team
                .iter()
                .chain(session.their_team.iter())
                .filter(|s| s.champion_id > 0)
                .map(|s| s.champion_id),
        )
        .collect();

    // ChampionType lists for the player's top-mastery pool — used as the
    // "defender" side when computing how well a candidate counters us.
    let pool_types: Vec<Vec<ChampionType>> = my_pool
        .iter()
        .filter_map(|m| {
            role_map
                .get(&(m.champion_id as u32))
                .map(|r| ChampionType::from_roles(r))
        })
        .filter(|v| !v.is_empty())
        .collect();

    // Lane key for meta_rates lookup. ARAM uses the synthetic "aram" key so
    // the caller can persist ARAM-wide rates without a real lane column.
    let lane_key: String = if session.queue_id == 450 {
        "aram".to_string()
    } else {
        session.local_player.assigned_position.to_lowercase()
    };

    // KB archetypes present in the player's mastery pool — used to detect
    // candidates that hard-counter our pool by archetype (counters_archetypes).
    let pool_archetypes: HashSet<String> = my_pool
        .iter()
        .filter_map(|m| {
            all_champions
                .iter()
                .find(|c| c.champion_id == m.champion_id)
        })
        .filter_map(|c| kb.get_archetype(&c.key))
        .map(|a| a.archetype.clone())
        .collect();

    // Early phase = no enemy champion locked yet (we ban before counter-picking
    // is possible, so meta tyrants deserve a small priority bump).
    let is_early = session.their_team.iter().all(|s| s.champion_id == 0);

    // Enemy hover/intent targets — a direct "deny this pick" signal that works
    // fully offline. Already-picked enemies are in `excluded`; this catches the
    // ones still being hovered (championPickIntent set, championId still 0).
    let enemy_intents: HashSet<u32> = session
        .their_team
        .iter()
        .map(|s| s.intent_champion_id)
        .filter(|&id| id > 0)
        .collect();

    let mut suggestions: Vec<BanSuggestion> = all_champions
        .iter()
        .filter(|c| !excluded.contains(&(c.champion_id as u32)))
        .filter_map(|champ| {
            let id = champ.champion_id as u32;

            // ── Signals (any subset may be absent — meta is empty offline) ────
            // Meta signal: optional. Only counts when a real sample backs it.
            let rate = meta_rates
                .get(&(id, lane_key.clone()))
                .filter(|r| r.sample_size > 0);
            let has_meta = rate.is_some();

            // Enemy is hovering this champion right now.
            let enemy_hovering = enemy_intents.contains(&id);

            // KB pool-weakness: candidate's `counters_archetypes` intersects the
            // archetypes in our mastery pool → it structurally beats what we play.
            let counters_pool = kb
                .get_archetype(&champ.key)
                .map(|a| {
                    a.counters_archetypes
                        .iter()
                        .any(|x| pool_archetypes.contains(x))
                })
                .unwrap_or(false);

            // Type-based pool counter: average advantage of `candidate` vs each of
            // our pool entries (candidate = attacker, pool entry = defender).
            // `None` when either side lacks role data — not a real signal then.
            let candidate_types = role_map
                .get(&id)
                .map(|r| ChampionType::from_roles(r))
                .unwrap_or_default();
            let pool_norm: Option<f32> = if pool_types.is_empty() || candidate_types.is_empty() {
                None
            } else {
                let mut total = 0.0_f32;
                let mut count = 0_u32;
                for pool_entry in &pool_types {
                    for at in &candidate_types {
                        for dt in pool_entry {
                            total += type_counter_score(at, dt);
                            count += 1;
                        }
                    }
                }
                (count > 0).then(|| (total / count as f32).min(1.0))
            };
            let strong_pool_counter = pool_norm.is_some_and(|v| v > 0.55);

            // Inclusion gate: at least one concrete signal. Without this, offline
            // mode (no meta data) would rank every champion on noise.
            if !(has_meta || enemy_hovering || counters_pool || strong_pool_counter) {
                return None;
            }

            // ── Components ────────────────────────────────────────────────────
            // Meta (win_rate 0.48→0.55, weight 0.40) + ban_rate (30% → max, 0.30)
            // + an early-phase nudge toward meta tyrants. All zero without meta.
            // Win-rate is Bayesian-shrunk so a low-sample spike can't claim a
            // ban slot from a genuinely oppressive high-sample pick.
            let (meta_norm, win_rate, ban_rate) = rate
                .map(|r| {
                    let wr = shrunk_meta_wr(r.win_rate, r.sample_size);
                    (((wr - 0.48) / 0.07).clamp(0.0, 1.0), wr, r.ban_rate)
                })
                .unwrap_or((0.0, 0.0, 0.0));
            let meta_component = meta_norm * 0.40;
            let ban_component = (ban_rate / 0.30).clamp(0.0, 1.0) * 0.30;
            let early_component = if is_early { meta_norm * 0.08 } else { 0.0 };

            let pool_counter_component = pool_norm.unwrap_or(0.0) * 0.30;
            let pool_weak_component = if counters_pool { 0.10 } else { 0.0 };
            // Strong, fully-offline signal: a hovered enemy pick lands at least in
            // the "Tehlikeli" band (≥0.60) on its own.
            let enemy_intent_component = if enemy_hovering { 0.62 } else { 0.0 };

            let threat_score = (meta_component
                + ban_component
                + pool_counter_component
                + pool_weak_component
                + early_component
                + enemy_intent_component)
                .clamp(0.0, 1.0);

            // Build the rationale string.
            let mut reasons: Vec<&str> = Vec::new();
            if threat_score >= 0.80 {
                reasons.push("Çok tehlikeli");
            } else if threat_score >= 0.60 {
                reasons.push("Tehlikeli");
            } else {
                reasons.push("Dikkat");
            }
            if enemy_hovering {
                reasons.push("rakip hedefliyor");
            }
            if ban_rate > 0.20 {
                reasons.push("yüksek ban önceliği");
            }
            if counters_pool {
                reasons.push("havuz arketipini sayar");
            }
            if pool_norm.is_some_and(|v| v > 0.55) {
                reasons.push("havuzunuza karşı güçlü");
            }
            if win_rate > 0.52 {
                reasons.push("meta güçlü");
            }
            if is_early && win_rate > 0.52 {
                reasons.push("erken hedef");
            }

            Some(BanSuggestion {
                champion_id: id,
                champion_key: champ.key.clone(),
                champion_name: champ.name.clone(),
                threat_score,
                reason: reasons.join(" · "),
            })
        })
        .collect();

    suggestions.sort_by(|a, b| {
        b.threat_score
            .partial_cmp(&a.threat_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    suggestions.truncate(3);
    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TeamSlot;
    use crate::draft_iq::DraftKnowledgeBase;

    fn empty_session() -> ChampSelectState {
        ChampSelectState {
            my_cell_id: 0,
            local_player: TeamSlot {
                cell_id: 0,
                champion_id: 0,
                intent_champion_id: 0,
                assigned_position: "middle".into(),
                is_locked: false,
            },
            my_team: vec![],
            their_team: vec![],
            my_bans: vec![],
            their_bans: vec![],
            phase: "BAN_PICK".into(),
            time_left_ms: 30_000,
            action_type: "ban".into(),
            queue_id: 420,
            pick_order: 0,
        }
    }

    fn champ(id: i64, key: &str, name: &str) -> ChampionRecord {
        ChampionRecord {
            champion_id: id,
            key: key.into(),
            name: name.into(),
            title: "".into(),
        }
    }

    #[test]
    fn excludes_already_banned_and_picked() {
        let mut session = empty_session();
        session.my_bans = vec![1];
        session.their_bans = vec![2];
        session.my_team = vec![TeamSlot {
            cell_id: 0,
            champion_id: 3,
            intent_champion_id: 3,
            assigned_position: "middle".into(),
            is_locked: true,
        }];
        session.their_team = vec![TeamSlot {
            cell_id: 5,
            champion_id: 4,
            intent_champion_id: 4,
            assigned_position: "middle".into(),
            is_locked: true,
        }];

        let champs = vec![
            champ(1, "A", "A"),
            champ(2, "B", "B"),
            champ(3, "C", "C"),
            champ(4, "D", "D"),
            champ(5, "E", "E"),
        ];
        let mut rates = HashMap::new();
        for id in [1, 2, 3, 4, 5] {
            rates.insert(
                (id, "middle".into()),
                MetaRate {
                    win_rate: 0.55,
                    ban_rate: 0.1,
                    sample_size: 1000,
                },
            );
        }
        let role_map = HashMap::new();
        let kb = DraftKnowledgeBase::load().expect("KB yüklenemedi");
        let result = compute_ban_suggestions(&session, &champs, &rates, &[], &role_map, &kb);
        let ids: Vec<u32> = result.iter().map(|s| s.champion_id).collect();
        assert!(!ids.contains(&1), "banlı şampiyon önerilmemeli");
        assert!(!ids.contains(&2), "rakip banı önerilmemeli");
        assert!(!ids.contains(&3), "müttefik pick önerilmemeli");
        assert!(!ids.contains(&4), "rakip pick önerilmemeli");
        assert!(ids.contains(&5), "uygun aday gelmeli");
    }

    #[test]
    fn skips_champions_without_meta_rate() {
        let session = empty_session();
        let champs = vec![champ(10, "X", "X"), champ(11, "Y", "Y")];
        let mut rates = HashMap::new();
        // Only champ 10 has data
        rates.insert(
            (10, "middle".into()),
            MetaRate {
                win_rate: 0.55,
                ban_rate: 0.1,
                sample_size: 1000,
            },
        );
        let role_map = HashMap::new();
        let kb = DraftKnowledgeBase::load().expect("KB yüklenemedi");
        let result = compute_ban_suggestions(&session, &champs, &rates, &[], &role_map, &kb);
        let ids: Vec<u32> = result.iter().map(|s| s.champion_id).collect();
        assert_eq!(
            ids,
            vec![10],
            "yalnızca meta verisi olan şampiyon önerilmeli"
        );
    }

    #[test]
    fn empty_meta_rates_returns_empty() {
        let session = empty_session();
        let champs = vec![champ(10, "X", "X")];
        let rates = HashMap::new();
        let role_map = HashMap::new();
        let kb = DraftKnowledgeBase::load().expect("KB yüklenemedi");
        let result = compute_ban_suggestions(&session, &champs, &rates, &[], &role_map, &kb);
        assert!(result.is_empty(), "meta_rates boşken sonuç boş olmalı");
    }

    #[test]
    fn caps_to_three_results() {
        let session = empty_session();
        let mut champs = Vec::new();
        let mut rates = HashMap::new();
        for id in 1..=10u32 {
            champs.push(champ(id as i64, &format!("K{id}"), &format!("N{id}")));
            rates.insert(
                (id, "middle".into()),
                MetaRate {
                    win_rate: 0.55,
                    ban_rate: 0.10,
                    sample_size: 1000,
                },
            );
        }
        let role_map = HashMap::new();
        let kb = DraftKnowledgeBase::load().expect("KB yüklenemedi");
        let result = compute_ban_suggestions(&session, &champs, &rates, &[], &role_map, &kb);
        assert_eq!(result.len(), 3, "en fazla 3 öneri dönmeli");
    }

    #[test]
    fn results_are_sorted_descending_by_threat() {
        let session = empty_session();
        let champs = vec![
            champ(1, "Low", "Low"),
            champ(2, "Mid", "Mid"),
            champ(3, "Hi", "Hi"),
        ];
        let mut rates = HashMap::new();
        rates.insert(
            (1, "middle".into()),
            MetaRate {
                win_rate: 0.48,
                ban_rate: 0.05,
                sample_size: 500,
            },
        ); // low
        rates.insert(
            (2, "middle".into()),
            MetaRate {
                win_rate: 0.51,
                ban_rate: 0.05,
                sample_size: 500,
            },
        ); // mid
        rates.insert(
            (3, "middle".into()),
            MetaRate {
                win_rate: 0.55,
                ban_rate: 0.05,
                sample_size: 500,
            },
        ); // high
        let role_map = HashMap::new();
        let kb = DraftKnowledgeBase::load().expect("KB yüklenemedi");
        let result = compute_ban_suggestions(&session, &champs, &rates, &[], &role_map, &kb);
        assert!(result.len() >= 2);
        for window in result.windows(2) {
            assert!(
                window[0].threat_score >= window[1].threat_score,
                "threat_score azalan sırada olmalı"
            );
        }
    }

    #[test]
    fn ban_rate_high_elevates_threat() {
        // Two champs with identical winrate; one has high ban_rate, one low.
        // The high-ban champion should rank first.
        let session = empty_session();
        let champs = vec![
            champ(10, "HighBan", "HighBan"),
            champ(11, "LowBan", "LowBan"),
        ];
        let mut rates = HashMap::new();
        rates.insert(
            (10, "middle".into()),
            MetaRate {
                win_rate: 0.52,
                ban_rate: 0.35,
                sample_size: 1000,
            },
        );
        rates.insert(
            (11, "middle".into()),
            MetaRate {
                win_rate: 0.52,
                ban_rate: 0.02,
                sample_size: 1000,
            },
        );
        let role_map = HashMap::new();
        let kb = DraftKnowledgeBase::load().expect("KB yüklenemedi");
        let result = compute_ban_suggestions(&session, &champs, &rates, &[], &role_map, &kb);
        assert_eq!(
            result[0].champion_id, 10,
            "yüksek ban_rate'li şampiyon önce gelmeli"
        );
        assert!(
            result[0].reason.contains("ban önceliği"),
            "yüksek ban_rate için reason 'ban önceliği' içermeli, got: {}",
            result[0].reason
        );
        assert!(
            !result[1].reason.contains("ban önceliği"),
            "düşük ban_rate'li şampiyonda 'ban önceliği' olmamalı"
        );
    }

    #[test]
    fn aram_uses_aram_lane_key() {
        let mut session = empty_session();
        session.queue_id = 450;

        let champs = vec![champ(10, "X", "X")];
        let mut rates = HashMap::new();
        // Wrong key → no result
        rates.insert(
            (10, "middle".into()),
            MetaRate {
                win_rate: 0.55,
                ban_rate: 0.1,
                sample_size: 1000,
            },
        );
        let role_map = HashMap::new();
        let kb = DraftKnowledgeBase::load().expect("KB yüklenemedi");
        let result = compute_ban_suggestions(&session, &champs, &rates, &[], &role_map, &kb);
        assert!(
            result.is_empty(),
            "ARAM için 'middle' anahtarlı veri kullanılmamalı"
        );

        // Correct ARAM key → returns the candidate
        rates.insert(
            (10, "aram".into()),
            MetaRate {
                win_rate: 0.55,
                ban_rate: 0.1,
                sample_size: 1000,
            },
        );
        let result = compute_ban_suggestions(&session, &champs, &rates, &[], &role_map, &kb);
        assert_eq!(result.len(), 1, "ARAM anahtarlı veri kullanılmalı");
    }

    #[test]
    fn pool_weakness_via_counters_archetypes_elevates_and_tags() {
        // Pool = Jinx (marksman). Aatrox (juggernaut) counters ["marksman",..] →
        // pool-weakness fires; Lux (burst_mage) counters ["diver","vanguard"] → no.
        let kb = DraftKnowledgeBase::load().expect("KB yüklenemedi");
        let session = empty_session();
        let champs = vec![
            champ(266, "Aatrox", "Aatrox"),
            champ(99, "Lux", "Lux"),
            champ(222, "Jinx", "Jinx"), // pool champ — present for key→archetype lookup
        ];
        let mut rates = HashMap::new();
        for id in [266u32, 99] {
            rates.insert(
                (id, "middle".into()),
                MetaRate {
                    win_rate: 0.50,
                    ban_rate: 0.05,
                    sample_size: 1000,
                },
            );
        }
        let pool = vec![MasteryRow {
            champion_id: 222,
            level: 7,
            points: 100_000,
            last_play_time: None,
        }];
        let role_map = HashMap::new();
        let result = compute_ban_suggestions(&session, &champs, &rates, &pool, &role_map, &kb);

        let aatrox = result
            .iter()
            .find(|s| s.champion_id == 266)
            .expect("Aatrox aday olmalı");
        assert!(
            aatrox.reason.contains("havuz arketipini sayar"),
            "Aatrox havuzdaki marksman'i sayar, reason: {}",
            aatrox.reason
        );
        let a_score = aatrox.threat_score;
        if let Some(lux) = result.iter().find(|s| s.champion_id == 99) {
            assert!(
                a_score > lux.threat_score,
                "pool-weakness Aatrox'u ({a_score:.3}) Lux'un ({:.3}) üstüne çıkarmalı",
                lux.threat_score
            );
        }
    }

    // ── Offline fallback (meta_rates empty) ───────────────────────────────────

    #[test]
    fn offline_enemy_hover_suggested_without_meta() {
        // No meta data at all (champion_rates empty offline). An enemy is hovering
        // Zed (championPickIntent) — the advisor must still suggest banning it.
        let mut session = empty_session();
        session.their_team = vec![TeamSlot {
            cell_id: 5,
            champion_id: 0,
            intent_champion_id: 238, // Zed hovered, not yet locked
            assigned_position: "middle".into(),
            is_locked: false,
        }];

        let champs = vec![champ(238, "Zed", "Zed"), champ(1, "Filler", "Filler")];
        let rates = HashMap::new(); // ← empty: no meta signal anywhere
        let role_map = HashMap::new();
        let kb = DraftKnowledgeBase::load().expect("KB yüklenemedi");
        let result = compute_ban_suggestions(&session, &champs, &rates, &[], &role_map, &kb);

        let zed = result
            .iter()
            .find(|s| s.champion_id == 238)
            .expect("Rakip hover'ladığı şampiyon meta olmadan da önerilmeli");
        assert!(
            zed.reason.contains("rakip hedefliyor"),
            "enemy-hover reason 'rakip hedefliyor' içermeli, got: {}",
            zed.reason
        );
        assert!(
            zed.threat_score >= 0.60,
            "hover'lanan rakip en az 'Tehlikeli' bandında olmalı, got {}",
            zed.threat_score
        );
        // The filler champion has no signal whatsoever → must not be suggested.
        assert!(
            !result.iter().any(|s| s.champion_id == 1),
            "sinyali olmayan şampiyon offline önerilmemeli"
        );
    }

    #[test]
    fn offline_pool_counter_suggested_without_meta() {
        // No meta data; pool = Jinx (marksman). Aatrox (counters marksman via KB
        // counters_archetypes) must surface as a ban even with zero meta rows.
        let kb = DraftKnowledgeBase::load().expect("KB yüklenemedi");
        let session = empty_session(); // no enemy intent
        let champs = vec![
            champ(266, "Aatrox", "Aatrox"),
            champ(99, "Lux", "Lux"), // burst_mage — does not counter marksman
            champ(222, "Jinx", "Jinx"), // pool champ, for key→archetype lookup
        ];
        let rates = HashMap::new(); // ← empty
        let pool = vec![MasteryRow {
            champion_id: 222,
            level: 7,
            points: 100_000,
            last_play_time: None,
        }];
        let role_map = HashMap::new();
        let result = compute_ban_suggestions(&session, &champs, &rates, &pool, &role_map, &kb);

        let aatrox = result
            .iter()
            .find(|s| s.champion_id == 266)
            .expect("Havuzu sayan arketip meta olmadan da önerilmeli");
        assert!(
            aatrox.reason.contains("havuz arketipini sayar"),
            "reason 'havuz arketipini sayar' içermeli, got: {}",
            aatrox.reason
        );
        // Lux does not counter our marksman pool and has no other signal → excluded.
        assert!(
            !result.iter().any(|s| s.champion_id == 99),
            "havuzu saymayan, sinyalsiz şampiyon offline önerilmemeli"
        );
    }
}
