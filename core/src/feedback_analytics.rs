//! Read-only feedback analytics (Feedback Loop — Sprint D, Claude).
//!
//! Pure aggregations over local `recommendation_feedback` rows for a transparency
//! report: per-champion sentiment trend, a recent-window signal count, and a
//! "which recommendations is the user disliking?" list. No IO — the command layer
//! (`commands/data_quality.rs`) reads the rows and calls `analyze_feedback`.
//!
//! Reuses the `feedback_signal` verdict classifier + weights so analytics can never
//! disagree with the scoring path about what a verdict means. Privacy-first: local
//! rows only, no PII, no network.

use crate::feedback_signal::FeedbackVerdict;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

/// Minimum polar data points before a champion can be called "disliked" — keeps a
/// single bad click from branding a recommendation.
const DISLIKE_MIN_SAMPLE: u32 = 3;
/// Net sentiment must be at least this negative to count as disliked.
const DISLIKE_NET_MAX: f32 = -0.20;

/// One feedback row with timing. The command layer maps DB rows to these.
/// `Deserialize` is additive — `feedback_analytics_from_json` accepts raw rows.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FeedbackEvent {
    pub champion_id: u32,
    pub champion_key: String,
    pub verdict: String,
    /// Unix seconds (local DB `created_at`).
    pub created_at: i64,
}

/// Per-champion sentiment breakdown. All counts → `number` in TS (no `i64`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct ChampionFeedbackTrend {
    pub champion_id: u32,
    pub champion_key: String,
    pub helpful: u32,
    pub picked: u32,
    pub not_helpful: u32,
    /// Polar data points (`helpful + picked + not_helpful`).
    pub sample: u32,
    /// Weighted `Σweight / sample` in [-1, 1] (Helpful +1, Picked +0.5, NotHelpful −1).
    pub net_sentiment: f32,
    /// Polar events within the analysis window.
    pub recent_count: u32,
}

/// Full read-only analytics snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct FeedbackAnalytics {
    pub window_days: u32,
    /// All stored feedback events (any verdict).
    pub total_events: u32,
    /// Polar events within the window (the "last N days signal count").
    pub recent_signal_count: u32,
    /// Every champion with polar feedback, most-feedback first.
    pub trends: Vec<ChampionFeedbackTrend>,
    /// Champions the user is disliking (net ≤ −0.20, sample ≥ 3), worst first.
    pub disliked: Vec<ChampionFeedbackTrend>,
}

#[derive(Default)]
struct Tally {
    champion_key: String,
    helpful: u32,
    picked: u32,
    not_helpful: u32,
    weighted: f32,
    recent: u32,
}

/// Aggregate feedback events into the analytics snapshot. `now` + `window_days`
/// define the recency window for `recent_signal_count` / `recent_count`.
pub fn analyze_feedback(events: &[FeedbackEvent], now: i64, window_days: u32) -> FeedbackAnalytics {
    let window_start = now - i64::from(window_days) * 86_400;
    let mut tallies: HashMap<u32, Tally> = HashMap::new();
    let mut recent_signal_count = 0u32;

    for ev in events {
        let verdict = FeedbackVerdict::parse(&ev.verdict);
        if !verdict.is_polar() {
            continue;
        }
        let recent = ev.created_at >= window_start;
        if recent {
            recent_signal_count += 1;
        }
        let t = tallies.entry(ev.champion_id).or_default();
        t.champion_key = ev.champion_key.clone();
        t.weighted += verdict.weight();
        if recent {
            t.recent += 1;
        }
        match verdict {
            FeedbackVerdict::Helpful => t.helpful += 1,
            FeedbackVerdict::Picked => t.picked += 1,
            FeedbackVerdict::NotHelpful => t.not_helpful += 1,
            FeedbackVerdict::Skipped | FeedbackVerdict::Unknown => {}
        }
    }

    let mut trends: Vec<ChampionFeedbackTrend> = tallies
        .into_iter()
        .map(|(champion_id, t)| {
            let sample = t.helpful + t.picked + t.not_helpful;
            let net_sentiment = if sample == 0 {
                0.0
            } else {
                (t.weighted / sample as f32).clamp(-1.0, 1.0)
            };
            ChampionFeedbackTrend {
                champion_id,
                champion_key: t.champion_key,
                helpful: t.helpful,
                picked: t.picked,
                not_helpful: t.not_helpful,
                sample,
                net_sentiment,
                recent_count: t.recent,
            }
        })
        .collect();

    // Most feedback first; tie-break by champion_id for deterministic output.
    trends.sort_by(|a, b| {
        b.sample
            .cmp(&a.sample)
            .then(a.champion_id.cmp(&b.champion_id))
    });

    let mut disliked: Vec<ChampionFeedbackTrend> = trends
        .iter()
        .filter(|t| t.sample >= DISLIKE_MIN_SAMPLE && t.net_sentiment <= DISLIKE_NET_MAX)
        .cloned()
        .collect();
    // Worst sentiment first; tie-break by larger sample then champion_id.
    disliked.sort_by(|a, b| {
        a.net_sentiment
            .partial_cmp(&b.net_sentiment)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.sample.cmp(&a.sample))
            .then(a.champion_id.cmp(&b.champion_id))
    });

    FeedbackAnalytics {
        window_days,
        total_events: events.len() as u32,
        recent_signal_count,
        trends,
        disliked,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400;

    fn ev(champion_id: u32, key: &str, verdict: &str, created_at: i64) -> FeedbackEvent {
        FeedbackEvent {
            champion_id,
            champion_key: key.to_string(),
            verdict: verdict.to_string(),
            created_at,
        }
    }

    #[test]
    fn empty_is_all_zero() {
        let a = analyze_feedback(&[], 1_000_000, 7);
        assert_eq!(a.total_events, 0);
        assert_eq!(a.recent_signal_count, 0);
        assert!(a.trends.is_empty());
        assert!(a.disliked.is_empty());
    }

    #[test]
    fn neutral_verdicts_excluded_from_trends_but_counted_in_total() {
        let now = 10 * DAY;
        let a = analyze_feedback(
            &[ev(1, "Zed", "skipped", now), ev(1, "Zed", "wat", now)],
            now,
            7,
        );
        assert_eq!(a.total_events, 2, "total counts every row");
        assert_eq!(a.recent_signal_count, 0, "no polar events");
        assert!(a.trends.is_empty(), "neutral-only champ has no trend");
    }

    #[test]
    fn recent_window_only_counts_events_inside_it() {
        let now = 30 * DAY;
        let events = vec![
            ev(1, "Zed", "helpful", now - DAY),      // recent
            ev(1, "Zed", "helpful", now - 2 * DAY),  // recent
            ev(1, "Zed", "helpful", now - 20 * DAY), // old
        ];
        let a = analyze_feedback(&events, now, 7);
        assert_eq!(a.recent_signal_count, 2);
        let zed = &a.trends[0];
        assert_eq!(zed.sample, 3, "all three are polar");
        assert_eq!(zed.recent_count, 2, "only two within 7 days");
    }

    #[test]
    fn disliked_surfaces_negative_champions_worst_first() {
        let now = 5 * DAY;
        let events = vec![
            // Champ 1: strongly disliked (3 not_helpful) → net -1.0
            ev(1, "Yasuo", "not_helpful", now),
            ev(1, "Yasuo", "not_helpful", now),
            ev(1, "Yasuo", "not_helpful", now),
            // Champ 2: mildly disliked (2 not_helpful, 1 helpful) → net -1/3 ≈ -0.33
            ev(2, "Zed", "not_helpful", now),
            ev(2, "Zed", "not_helpful", now),
            ev(2, "Zed", "helpful", now),
            // Champ 3: liked → not disliked
            ev(3, "Lux", "helpful", now),
            ev(3, "Lux", "helpful", now),
            ev(3, "Lux", "helpful", now),
        ];
        let a = analyze_feedback(&events, now, 7);
        assert_eq!(a.disliked.len(), 2, "champs 1 and 2 disliked, not 3");
        assert_eq!(a.disliked[0].champion_id, 1, "worst (most negative) first");
        assert_eq!(a.disliked[1].champion_id, 2);
        assert!((a.disliked[0].net_sentiment - (-1.0)).abs() < 1e-4);
    }

    #[test]
    fn low_sample_negative_is_not_disliked() {
        let now = 5 * DAY;
        // Only 2 not_helpful → below DISLIKE_MIN_SAMPLE → not flagged.
        let a = analyze_feedback(
            &[
                ev(1, "Zed", "not_helpful", now),
                ev(1, "Zed", "not_helpful", now),
            ],
            now,
            7,
        );
        assert_eq!(a.trends.len(), 1, "still trended");
        assert!(a.disliked.is_empty(), "2 points is not enough to brand it");
    }

    #[test]
    fn picked_is_weak_positive_in_net() {
        let now = DAY;
        // 4 picked → net +0.5 (weak), not +1.0.
        let events: Vec<FeedbackEvent> = (0..4).map(|_| ev(1, "Zed", "picked", now)).collect();
        let a = analyze_feedback(&events, now, 7);
        let zed = &a.trends[0];
        assert_eq!(zed.picked, 4);
        assert!((zed.net_sentiment - 0.5).abs() < 1e-4);
        assert!(a.disliked.is_empty());
    }

    #[test]
    fn trends_sorted_by_sample_desc() {
        let now = DAY;
        let mut events = vec![ev(1, "Zed", "helpful", now)];
        for _ in 0..3 {
            events.push(ev(2, "Lux", "helpful", now));
        }
        let a = analyze_feedback(&events, now, 7);
        assert_eq!(a.trends[0].champion_id, 2, "more feedback first");
        assert_eq!(a.trends[1].champion_id, 1);
    }
}
