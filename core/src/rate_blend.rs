//! Cross-source rate blending (multi-source aggressive ingestion, Faz B3).
//!
//! Once several sources (Meraki, u.gg, Riot Match-V5, …) populate `champion_rates`,
//! a champion+lane has **one row per source**. Collecting those straight into a
//! `HashMap` keyed by (champion, position) would silently keep whichever source
//! sorted last — an arbitrary pick. This module blends them honestly instead:
//!
//!   - **win_rate**: sample-weighted mean across sources (a tiny-sample outlier
//!     barely moves a large-sample consensus).
//!   - **ban_rate**: weighted mean over only the sources that actually carry a ban
//!     signal (u.gg / Match-V5 report `0` = *unknown*, not *0% banned*, so they're
//!     skipped — never dilute Meraki's real ban-rate toward zero).
//!   - **sample_size / confidence**: when ≥2 well-sampled sources **agree**
//!     (win-rate spread ≤ `AGREEMENT_TOL`), their samples are summed → higher
//!     confidence (independent cross-validation). When they **disagree**, we keep
//!     only the largest single sample → no false confidence boost; the heaviest,
//!     most trustworthy source anchors the estimate.
//!
//! Pure + engine-safe: no DB/network. The command layer maps `champion_rates`
//! rows into [`SourceRate`] and calls [`blend_rates`].
#![allow(dead_code)]

use crate::scoring::MetaRate;
use std::collections::{HashMap, HashSet};

/// A role must be at least this share of a champion's total games across all lanes to
/// count as a lane the champion is genuinely played in. Filters u.gg off-role noise
/// (e.g. ~2.6k "bottom" Olaf games ≈ 1% of his play) without dropping real flex picks
/// (Senna 45% bottom / 54% support; Pantheon 13–44% across four lanes). The egregious
/// "Olaf/Yi/Sion in the ADC pool" rows all sit at ≤2% share.
pub const ROLE_SHARE_MIN: f32 = 0.05;

/// From raw per-(champion, position) sample totals, return the set of (champion_id,
/// position) where that position is ≥ `min_share` of the champion's total games. A
/// champion absent from `champion_rates` is simply absent from the set — callers gate
/// such picks via the archetype/flex fallback, not this set.
pub fn role_played_set(
    position_samples: &[(u32, String, u32)],
    min_share: f32,
) -> HashSet<(u32, String)> {
    let mut totals: HashMap<u32, u64> = HashMap::new();
    for (id, _pos, n) in position_samples {
        *totals.entry(*id).or_default() += *n as u64;
    }
    position_samples
        .iter()
        .filter(|(id, _pos, n)| {
            let total = totals.get(id).copied().unwrap_or(0);
            total > 0 && (*n as f32 / total as f32) >= min_share
        })
        .map(|(id, pos, _)| (*id, pos.clone()))
        .collect()
}

/// A source must clear this sample to count toward the agreement check.
const MIN_CONSENSUS_SAMPLE: u32 = 100;
/// Max win-rate spread (between well-sampled sources) still treated as agreement.
const AGREEMENT_TOL: f32 = 0.06;

/// One per-source rate row for a (champion, position), pre-blend.
/// `Deserialize` is additive — the JSON API (`blended_meta_rates_from_json`)
/// accepts these rows straight from the JS host's `champion_rates` reads.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SourceRate {
    pub champion_id: u32,
    pub position: String,
    pub win_rate: f32,
    pub ban_rate: f32,
    pub sample_size: u32,
}

/// Blend every per-source row into one [`MetaRate`] per (champion, position).
pub fn blend_rates(rows: Vec<SourceRate>) -> HashMap<(u32, String), MetaRate> {
    let mut groups: HashMap<(u32, String), Vec<SourceRate>> = HashMap::new();
    for r in rows {
        groups
            .entry((r.champion_id, r.position.clone()))
            .or_default()
            .push(r);
    }
    groups
        .into_iter()
        .map(|(key, group)| (key, blend_group(&group)))
        .collect()
}

fn weight(sample: u32) -> f64 {
    // Min 1 so an unknown-sample (0) source still contributes a little to the mean
    // rather than being dropped entirely.
    sample.max(1) as f64
}

fn blend_group(group: &[SourceRate]) -> MetaRate {
    debug_assert!(!group.is_empty());

    let total_w: f64 = group.iter().map(|r| weight(r.sample_size)).sum();
    let win_rate = (group
        .iter()
        .map(|r| r.win_rate as f64 * weight(r.sample_size))
        .sum::<f64>()
        / total_w) as f32;

    // Ban rate: only sources that actually report one (ban_rate > 0).
    let ban_rate = {
        let ban_rows: Vec<&SourceRate> = group.iter().filter(|r| r.ban_rate > 0.0).collect();
        if ban_rows.is_empty() {
            0.0
        } else {
            let bw: f64 = ban_rows.iter().map(|r| weight(r.sample_size)).sum();
            (ban_rows
                .iter()
                .map(|r| r.ban_rate as f64 * weight(r.sample_size))
                .sum::<f64>()
                / bw) as f32
        }
    };

    let sum_sample: u64 = group.iter().map(|r| r.sample_size as u64).sum();
    let max_sample: u32 = group.iter().map(|r| r.sample_size).max().unwrap_or(0);

    // Agreement-aware confidence: reward independent, well-sampled sources that agree.
    let strong: Vec<f32> = group
        .iter()
        .filter(|r| r.sample_size >= MIN_CONSENSUS_SAMPLE)
        .map(|r| r.win_rate)
        .collect();
    let sample_size = if strong.len() >= 2 {
        let hi = strong.iter().cloned().fold(f32::MIN, f32::max);
        let lo = strong.iter().cloned().fold(f32::MAX, f32::min);
        if hi - lo <= AGREEMENT_TOL {
            sum_sample.min(u32::MAX as u64) as u32 // agreement → combined evidence
        } else {
            max_sample // disagreement → trust the heaviest source, no false confidence
        }
    } else {
        max_sample
    };

    MetaRate {
        win_rate,
        ban_rate,
        sample_size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sr(id: u32, wr: f32, ban: f32, n: u32) -> SourceRate {
        SourceRate {
            champion_id: id,
            position: "jungle".to_string(),
            win_rate: wr,
            ban_rate: ban,
            sample_size: n,
        }
    }

    #[test]
    fn single_source_passes_through_unchanged() {
        let out = blend_rates(vec![sr(64, 0.527, 0.08, 5000)]);
        let m = &out[&(64, "jungle".to_string())];
        assert!((m.win_rate - 0.527).abs() < 1e-5);
        assert!((m.ban_rate - 0.08).abs() < 1e-5);
        assert_eq!(m.sample_size, 5000);
    }

    #[test]
    fn agreeing_sources_average_and_sum_confidence() {
        // Meraki 0.52/n5000 + u.gg 0.53/n9000 agree → weighted mean, summed sample.
        let out = blend_rates(vec![sr(64, 0.52, 0.08, 5000), sr(64, 0.53, 0.0, 9000)]);
        let m = &out[&(64, "jungle".to_string())];
        let expected = (0.52 * 5000.0 + 0.53 * 9000.0) / 14000.0;
        assert!((m.win_rate - expected as f32).abs() < 1e-4);
        assert_eq!(m.sample_size, 14000, "agreement sums evidence");
        // ban from Meraki only (u.gg's 0 is 'unknown', not 0% ban).
        assert!((m.ban_rate - 0.08).abs() < 1e-5);
    }

    #[test]
    fn disagreeing_sources_keep_max_sample_not_sum() {
        // 0.50 vs 0.60 spread > tol → no confidence reward.
        let out = blend_rates(vec![sr(64, 0.50, 0.0, 5000), sr(64, 0.60, 0.0, 9000)]);
        let m = &out[&(64, "jungle".to_string())];
        assert_eq!(m.sample_size, 9000, "disagreement → largest single sample");
        // win_rate still weighted-mean (larger sample pulls it toward 0.60).
        assert!(m.win_rate > 0.55 && m.win_rate < 0.57);
    }

    #[test]
    fn tiny_outlier_barely_moves_large_sample_consensus() {
        // A 5-game 0.90 outlier next to a 9000-game 0.52 source.
        let out = blend_rates(vec![sr(64, 0.52, 0.0, 9000), sr(64, 0.90, 0.0, 5)]);
        let m = &out[&(64, "jungle".to_string())];
        assert!(m.win_rate < 0.525, "tiny sample barely shifts the mean");
        // Outlier has sample < MIN_CONSENSUS so not part of the agreement set →
        // strong has 1 member → sample = max single = 9000.
        assert_eq!(m.sample_size, 9000);
    }

    #[test]
    fn ban_rate_ignores_unknown_zeros() {
        let out = blend_rates(vec![sr(64, 0.50, 0.0, 9000), sr(64, 0.50, 0.12, 4000)]);
        let m = &out[&(64, "jungle".to_string())];
        assert!(
            (m.ban_rate - 0.12).abs() < 1e-5,
            "only the real ban signal counts"
        );
    }

    #[test]
    fn role_share_keeps_main_lanes_drops_off_role_noise() {
        // Real numbers (u.gg): Olaf is 84% top / 12% jungle but only 1% bottom; a real
        // ADC (Caitlyn) is 97% bottom; Senna flexes 45% bottom / 54% support.
        let samples = vec![
            (2u32, "top".to_string(), 199646),
            (2u32, "jungle".to_string(), 29358),
            (2u32, "bottom".to_string(), 2621),      // ~1% — noise
            (51u32, "bottom".to_string(), 1716021),  // 97% — real
            (51u32, "middle".to_string(), 22632),    // ~1% — noise
            (235u32, "utility".to_string(), 563955), // 54%
            (235u32, "bottom".to_string(), 468731),  // 45%
        ];
        let played = role_played_set(&samples, ROLE_SHARE_MIN);
        assert!(
            !played.contains(&(2, "bottom".to_string())),
            "Olaf ADC noise dropped"
        );
        assert!(played.contains(&(2, "top".to_string())), "Olaf top kept");
        assert!(
            played.contains(&(51, "bottom".to_string())),
            "Caitlyn ADC kept"
        );
        assert!(
            !played.contains(&(51, "middle".to_string())),
            "Caitlyn mid noise dropped"
        );
        assert!(
            played.contains(&(235, "utility".to_string())),
            "Senna support kept"
        );
        assert!(
            played.contains(&(235, "bottom".to_string())),
            "Senna ADC kept"
        );
    }
}
