//! Champion pool builder — suggests champions to LEARN for a given role.
//!
//! **Meta-led, learnability-balanced:** ranks KB champions that fit the role (and
//! aren't already owned) primarily by **current meta strength** (win-rate from the
//! blended `champion_rates`), tempered by learnability — low execution difficulty +
//! blind safety + synergy with the player's existing pool (combos). When no meta
//! data exists for a champion it falls back to a neutral score, so the list degrades
//! gracefully to learnability-led. Measured language; this is a "solid starting
//! point", not a "best pick". No fabrication: a tier/"meta strong" label only
//! appears when the sample clears `MIN_META_SAMPLE`.

use super::champion_types::{archetype_position_fit, flex_positions};
use super::draft_iq::archetype::ChampionArchetype;
use super::draft_iq::combos::ComboDirectory;
use super::scoring::{shrunk_meta_wr, MetaRate};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Below this sample size, meta is treated as unknown (neutral, no tier/label).
const MIN_META_SAMPLE: u32 = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PoolSuggestion {
    pub champion_id: u32,
    pub champion_key: String,
    pub archetype: String,
    pub reason: String,
    /// 1 (hard) .. 5 (easy) — inverse of execution difficulty.
    pub ease: u8,
    /// Meta tier (S/A/B/C from win-rate) when the sample is sufficient, else None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta_tier: Option<String>,
}

/// Normalized meta fit in [0, 1] from the Bayesian-shrunk win-rate (same shape
/// as `scoring::meta_score`): 0.48 → 0.0, 0.55 → 1.0. `None` when missing or
/// under-sampled.
fn meta_fit(meta: &HashMap<(u32, String), MetaRate>, id: u32, role: &str) -> Option<f32> {
    let r = meta.get(&(id, role.to_string()))?;
    if r.sample_size < MIN_META_SAMPLE {
        return None;
    }
    let wr = shrunk_meta_wr(r.win_rate, r.sample_size);
    Some(((wr - 0.48) / 0.07).clamp(0.0, 1.0))
}

/// Coarse S/A/B/C tier from the Bayesian-shrunk win-rate, only when the sample
/// clears the floor — a 60-game 55% spike tiers as A, not S.
fn meta_tier(meta: &HashMap<(u32, String), MetaRate>, id: u32, role: &str) -> Option<String> {
    let r = meta.get(&(id, role.to_string()))?;
    if r.sample_size < MIN_META_SAMPLE {
        return None;
    }
    let wr = shrunk_meta_wr(r.win_rate, r.sample_size);
    let tier = if wr >= 0.53 {
        "S"
    } else if wr >= 0.51 {
        "A"
    } else if wr >= 0.49 {
        "B"
    } else {
        "C"
    };
    Some(tier.to_string())
}

/// Suggest up to 6 champions to learn for `role`.
/// `owned` = (id, key) of mastered champions (excluded; their keys drive synergy).
/// `candidates` = all KB champions as (id, key, archetype).
/// `meta_rates` = blended `champion_rates` for the role (None → meta-independent).
pub fn suggest_pool(
    role: &str,
    owned: &[(u32, String)],
    candidates: &[(u32, String, &ChampionArchetype)],
    combos: &ComboDirectory,
    meta_rates: Option<&HashMap<(u32, String), MetaRate>>,
) -> Vec<PoolSuggestion> {
    let owned_ids: HashSet<u32> = owned.iter().map(|(id, _)| *id).collect();
    let owned_keys: Vec<&str> = owned.iter().map(|(_, k)| k.as_str()).collect();

    // Role fit: when we have real play data (champion_rates for this role), a
    // champion only belongs in the pool if it's ACTUALLY played there — the data
    // sources already drop off-role noise (Meraki skips ~0% lanes, u.gg trims
    // <200-game roles), so e.g. a mage like Brand shows up for mid/support but not
    // bottom. Without data (offline/fresh install) fall back to the (lenient)
    // archetype map. This is what fixes "wrong-role pool suggestions".
    // A champion fits the role if it is ACTUALLY played there (real meta data with a
    // meaningful sample — so a handful of off-role troll games don't qualify it) OR its
    // KB archetype plausibly fits the lane (works offline when no meta is synced). This
    // is what stops e.g. ADC Olaf / Master Yi / Sion in the bottom-lane pool.
    let role_fits = |id: u32, archetype: &str| -> bool {
        let meta_present = meta_rates
            .and_then(|m| m.get(&(id, role.to_string())))
            .is_some_and(|r| r.sample_size >= MIN_META_SAMPLE);
        meta_present
            || flex_positions(id).contains(&role)
            || archetype_position_fit(archetype, role)
    };

    let mut scored: Vec<(f32, PoolSuggestion)> = candidates
        .iter()
        .filter(|(id, _, _)| !owned_ids.contains(id))
        .filter(|(id, _, a)| role_fits(*id, &a.archetype))
        .map(|(id, key, a)| {
            let ease_score = ((5i32 - a.execution_difficulty as i32).max(0)) as f32 / 4.0;
            let blind = a.blind_safety;
            let has_synergy = !combos.find_for_ally(key, &owned_keys).is_empty();
            let fit = meta_rates.and_then(|m| meta_fit(m, *id, role));
            // Meta leads; learnability tempers. Unknown meta → neutral 0.5 so the
            // list degrades gracefully to learnability-led ranking.
            let meta_component = fit.unwrap_or(0.5);
            let synergy_bonus = if has_synergy { 1.0 } else { 0.0 };
            let score =
                0.42 * meta_component + 0.25 * ease_score + 0.20 * blind + 0.13 * synergy_bonus;

            // Lead with two ALWAYS-present, champion-specific signals (power-curve
            // tempo + win-condition) so a pick never degenerates to a single generic
            // tag like "rolüne uygun". Archetype is already shown as its own chip, so
            // we don't repeat it here. Data-driven tags (meta/ease/safety/synergy)
            // then refine the read.
            let pc = &a.power_curve;
            let tempo = if pc.early >= 0.65 && pc.early > pc.late {
                "erken oyun baskısı"
            } else if pc.late >= 0.70 && pc.late > pc.early {
                "geç oyun ölçeği"
            } else if pc.mid >= 0.70 {
                "orta oyun spike'ı"
            } else {
                "dengeli güç eğrisi"
            };
            let wc_tag = match a.win_condition.as_str() {
                "teamfight" => "teamfight kompu",
                "split_push" => "split baskısı",
                "pick" => "pick potansiyeli",
                "poke" => "poke/zone kontrol",
                "siege" => "kuşatma baskısı",
                "protect" => "carry koruma",
                "skirmish" => "skirmish tempolu",
                _ => "esnek oyun planı",
            };
            let mut parts: Vec<&str> = vec![tempo, wc_tag];
            match fit {
                Some(f) if f >= 0.6 => parts.push("meta güçlü"),
                Some(f) if f <= 0.3 => parts.push("meta zayıf"),
                _ => {}
            }
            if a.execution_difficulty <= 2 {
                parts.push("kolay öğrenilir");
            }
            if blind >= 0.70 {
                parts.push("blind güvenli");
            }
            if has_synergy {
                parts.push("havuzunla sinerji");
            }

            (
                score,
                PoolSuggestion {
                    champion_id: *id,
                    champion_key: key.clone(),
                    archetype: a.archetype.clone(),
                    reason: parts.join(" · "),
                    ease: 6u8.saturating_sub(a.execution_difficulty).max(1),
                    meta_tier: meta_rates.and_then(|m| meta_tier(m, *id, role)),
                },
            )
        })
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(6).map(|(_, s)| s).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft_iq::archetype::{CcProfile, DamageProfile, PowerCurve};
    use crate::draft_iq::combos::ComboDirectory;

    fn arch(archetype: &str, exec: u8, blind: f32) -> ChampionArchetype {
        ChampionArchetype {
            champion_id: 0,
            archetype: archetype.to_string(),
            damage_profile: DamageProfile {
                ad: 0.5,
                ap: 0.5,
                true_damage: 0.0,
            },
            cc: CcProfile {
                has_hard_cc: false,
                hard_cc_count: 0,
                primary_cc: vec![],
            },
            mobility: "medium".to_string(),
            engage_role: "none".to_string(),
            peel_capability: "low".to_string(),
            blind_safety: blind,
            execution_difficulty: exec,
            win_condition: "teamfight".to_string(),
            ult_type: "x".to_string(),
            confidence: "high".to_string(),
            power_curve: PowerCurve {
                early: 0.6,
                mid: 0.6,
                late: 0.6,
            },
            counters_archetypes: vec![],
            utility_tags: vec![],
        }
    }

    fn rate(win_rate: f32, sample: u32) -> MetaRate {
        MetaRate {
            win_rate,
            ban_rate: 0.0,
            sample_size: sample,
        }
    }

    #[test]
    fn middle_role_returns_mages_not_marksmen() {
        // Synthetic ids in the 9000s so they never collide with a real champion_id
        // that has a flex_positions override (e.g. 3 = Galio flexes mid).
        let mage = arch("control_mage", 3, 0.6);
        let asn = arch("assassin", 4, 0.4);
        let adc = arch("marksman", 2, 0.7);
        let candidates = vec![
            (9001u32, "MageChamp".to_string(), &mage),
            (9002u32, "AsnChamp".to_string(), &asn),
            (9003u32, "AdcChamp".to_string(), &adc),
        ];
        let out = suggest_pool(
            "middle",
            &[],
            &candidates,
            &ComboDirectory::new(vec![]),
            None,
        );
        let ids: Vec<u32> = out.iter().map(|s| s.champion_id).collect();
        assert!(
            ids.contains(&9001) && ids.contains(&9002),
            "mid → mage + assassin"
        );
        assert!(!ids.contains(&9003), "mid → marksman dönmemeli");
    }

    #[test]
    fn owned_excluded_and_easy_ranks_first_without_meta() {
        let easy = arch("control_mage", 1, 0.8); // easy + safe
        let hard = arch("burst_mage", 5, 0.4); // hard
        let owned_champ = arch("control_mage", 2, 0.7);
        let candidates = vec![
            (1u32, "Easy".to_string(), &easy),
            (2u32, "Hard".to_string(), &hard),
            (9u32, "Owned".to_string(), &owned_champ),
        ];
        let owned = vec![(9u32, "Owned".to_string())];
        // No meta → learnability-led; easy+safe still ranks first.
        let out = suggest_pool(
            "middle",
            &owned,
            &candidates,
            &ComboDirectory::new(vec![]),
            None,
        );
        let ids: Vec<u32> = out.iter().map(|s| s.champion_id).collect();
        assert!(!ids.contains(&9), "sahip olunan hariç tutulmalı");
        assert_eq!(out[0].champion_id, 1, "kolay+güvenli olan üstte");
        assert_eq!(out[0].ease, 5, "exec 1 → ease 5");
        assert!(out[0].meta_tier.is_none(), "meta yokken tier gösterilmez");
    }

    #[test]
    fn meta_led_promotes_strong_winrate_over_easier_pick() {
        // Two equal-archetype mids: a harder champ that is meta-strong vs an easier,
        // meta-weak one. Meta should lift the strong pick to the top.
        let strong = arch("control_mage", 4, 0.5); // harder, but meta-strong
        let easy_weak = arch("control_mage", 1, 0.7); // easy, meta-weak
        let candidates = vec![
            (1u32, "Strong".to_string(), &strong),
            (2u32, "EasyWeak".to_string(), &easy_weak),
        ];
        let mut meta: HashMap<(u32, String), MetaRate> = HashMap::new();
        meta.insert((1, "middle".to_string()), rate(0.545, 4000)); // ~tier S, fit ~0.93
        meta.insert((2, "middle".to_string()), rate(0.475, 4000)); // weak, fit 0
        let out = suggest_pool(
            "middle",
            &[],
            &candidates,
            &ComboDirectory::new(vec![]),
            Some(&meta),
        );
        assert_eq!(out[0].champion_id, 1, "meta-strong pick leads");
        assert_eq!(out[0].meta_tier.as_deref(), Some("S"));
        assert!(out[0].reason.contains("meta güçlü"));
    }

    #[test]
    fn off_role_archetype_excluded_even_with_low_sample_meta() {
        // The real wrong-role bug: a juggernaut (Olaf-style) has only a tiny off-role
        // meta row for bottom (a handful of troll games) — it must NOT qualify for the
        // ADC pool. A real marksman with no meta still fits via its archetype.
        let off_role = arch("juggernaut", 2, 0.7); // archetype does NOT fit bottom
        let real_adc = arch("marksman", 2, 0.7); // archetype fits bottom
        let candidates = vec![
            (1u32, "OffRole".to_string(), &off_role),
            (2u32, "RealAdc".to_string(), &real_adc),
        ];
        let mut meta: HashMap<(u32, String), MetaRate> = HashMap::new();
        meta.insert((1, "bottom".to_string()), rate(0.50, 10)); // troll sample < MIN
        let out = suggest_pool(
            "bottom",
            &[],
            &candidates,
            &ComboDirectory::new(vec![]),
            Some(&meta),
        );
        let ids: Vec<u32> = out.iter().map(|s| s.champion_id).collect();
        assert!(
            !ids.contains(&1),
            "off-role archetype with low-sample meta is excluded"
        );
        assert!(ids.contains(&2), "real marksman fits via archetype");
    }

    #[test]
    fn high_sample_off_archetype_meta_still_qualifies() {
        // A champion the archetype map wouldn't place here but that IS genuinely played
        // in the role (real meta, big sample — e.g. a marksman support) must stay.
        let flex = arch("marksman", 3, 0.6); // archetype says bottom, not utility
        let candidates = vec![(1u32, "FlexSup".to_string(), &flex)];
        let mut meta: HashMap<(u32, String), MetaRate> = HashMap::new();
        meta.insert((1, "utility".to_string()), rate(0.51, 2000)); // real support data
        let out = suggest_pool(
            "utility",
            &[],
            &candidates,
            &ComboDirectory::new(vec![]),
            Some(&meta),
        );
        assert_eq!(out.len(), 1, "flex pick with real role meta is kept");
    }

    #[test]
    fn flex_pick_kept_via_override_without_meta() {
        // Senna (id 235) is a `marksman` (archetype → bottom only) but mostly support.
        // With NO meta synced (fresh install), the flex_positions override must keep
        // her in the utility pool — while a true off-role champ stays excluded.
        let senna = arch("marksman", 3, 0.6); // archetype says bottom, not utility
        let olaf = arch("juggernaut", 2, 0.7); // top/jungle — never support
        let candidates = vec![
            (235u32, "Senna".to_string(), &senna),
            (2u32, "Olaf".to_string(), &olaf),
        ];
        let out = suggest_pool(
            "utility",
            &[],
            &candidates,
            &ComboDirectory::new(vec![]),
            None, // no meta at all
        );
        let ids: Vec<u32> = out.iter().map(|s| s.champion_id).collect();
        assert!(
            ids.contains(&235),
            "Senna kept in support pool via flex override"
        );
        assert!(!ids.contains(&2), "Olaf still excluded from support");
    }

    #[test]
    fn under_sampled_meta_is_ignored() {
        let champ = arch("control_mage", 2, 0.7);
        let candidates = vec![(1u32, "Champ".to_string(), &champ)];
        let mut meta: HashMap<(u32, String), MetaRate> = HashMap::new();
        meta.insert((1, "middle".to_string()), rate(0.60, 10)); // n=10 < floor
        let out = suggest_pool(
            "middle",
            &[],
            &candidates,
            &ComboDirectory::new(vec![]),
            Some(&meta),
        );
        assert!(out[0].meta_tier.is_none(), "düşük örneklem → tier yok");
        assert!(
            !out[0].reason.contains("meta"),
            "düşük örneklem → meta etiketi yok"
        );
    }
}
