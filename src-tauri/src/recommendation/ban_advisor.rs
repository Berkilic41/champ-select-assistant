//! Ban advisor — suggests up to 3 high-threat champions to ban during the
//! champ-select ban phase.
//!
//! Threat score formula (per candidate champion):
//! ```text
//!   threat = meta_component * 0.40
//!           + ban_rate_component * 0.30
//!           + pool_counter_component * 0.30
//! ```
//!
//! - `meta_component` derives from `MetaRate.win_rate` mapped linearly between
//!   0.48 and 0.55 (same shape as `meta_score` in `scoring.rs`). Candidates
//!   without any meta-rate row are excluded entirely — we don't ban what we
//!   cannot reason about.
//! - `ban_rate_component` derives from `MetaRate.ban_rate`; a 30 % ban rate maps
//!   to 1.0. This is the single most direct "pros target this champion" signal and
//!   was previously unused despite being available in the DB row.
//! - `pool_counter_component` averages `type_counter_score(candidate → mine)`
//!   across the player's top-mastery pool. The intuition is reversed from
//!   pick scoring: here we want champions that COUNTER our pool, so we treat
//!   the candidate as attacker and our pool entries as defenders.
//!
//! ARAM (queue_id 450): the caller passes `"aram"` as the lane key when fetching
//! meta_rates so this module needs no special-case branch.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use ts_rs::TS;

use crate::db::champion_repo::ChampionRecord;
use crate::db::mastery_repo::MasteryRow;
use crate::lcu::session::ChampSelectState;
use crate::recommendation::champion_types::{type_counter_score, ChampionType};
use crate::recommendation::scoring::MetaRate;

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

    let mut suggestions: Vec<BanSuggestion> = all_champions
        .iter()
        .filter(|c| !excluded.contains(&(c.champion_id as u32)))
        .filter_map(|champ| {
            let id = champ.champion_id as u32;

            // Only consider champions with meta data — bans without signal are noise.
            let rate = meta_rates.get(&(id, lane_key.clone()))?;
            if rate.sample_size == 0 {
                return None;
            }

            // Meta component: same 0.48–0.55 mapping as meta_score, weight 0.40.
            let meta_norm = ((rate.win_rate - 0.48) / 0.07).clamp(0.0, 1.0);
            let meta_component = meta_norm * 0.40;

            // Ban-rate component: 30% ban rate → max. Weight 0.30.
            // Previously unused despite being in MetaRate — now the primary "priority ban" signal.
            let ban_norm = (rate.ban_rate / 0.30).clamp(0.0, 1.0);
            let ban_component = ban_norm * 0.30;

            // Pool counter component: average advantage of `candidate` vs each of
            // our pool entries (candidate = attacker, pool entry = defender). Weight 0.30.
            let candidate_types = role_map
                .get(&id)
                .map(|r| ChampionType::from_roles(r))
                .unwrap_or_default();

            let pool_norm = if pool_types.is_empty() || candidate_types.is_empty() {
                0.3
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
                if count == 0 {
                    0.3
                } else {
                    (total / count as f32).min(1.0)
                }
            };
            let pool_counter_component = pool_norm * 0.30;

            let threat_score =
                (meta_component + ban_component + pool_counter_component).clamp(0.0, 1.0);

            // Build the rationale string.
            let mut reasons: Vec<&str> = Vec::new();
            if threat_score >= 0.80 {
                reasons.push("Çok tehlikeli");
            } else if threat_score >= 0.60 {
                reasons.push("Tehlikeli");
            } else {
                reasons.push("Dikkat");
            }
            if rate.ban_rate > 0.20 {
                reasons.push("yüksek ban önceliği");
            }
            if pool_norm > 0.55 {
                reasons.push("havuzunuza karşı güçlü");
            }
            if rate.win_rate > 0.52 {
                reasons.push("meta güçlü");
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
    use crate::lcu::session::TeamSlot;

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
        let result = compute_ban_suggestions(&session, &champs, &rates, &[], &role_map);
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
        let result = compute_ban_suggestions(&session, &champs, &rates, &[], &role_map);
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
        let result = compute_ban_suggestions(&session, &champs, &rates, &[], &role_map);
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
        let result = compute_ban_suggestions(&session, &champs, &rates, &[], &role_map);
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
        let result = compute_ban_suggestions(&session, &champs, &rates, &[], &role_map);
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
        let result = compute_ban_suggestions(&session, &champs, &rates, &[], &role_map);
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
        let result = compute_ban_suggestions(&session, &champs, &rates, &[], &role_map);
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
        let result = compute_ban_suggestions(&session, &champs, &rates, &[], &role_map);
        assert_eq!(result.len(), 1, "ARAM anahtarlı veri kullanılmalı");
    }
}
