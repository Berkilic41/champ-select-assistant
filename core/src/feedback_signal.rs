//! Local feedback aggregation (Feedback Loop v1 — engine-pure slice, Claude).
//!
//! Today feedback is **write-only**: `submit_recommendation_feedback` stores rows
//! in `recommendation_feedback`, the quality report counts them, but nothing reads
//! them back. This module closes the loop on the PURE side: it turns local
//! feedback rows into a per-champion `FeedbackSignal` — a small, sample-damped,
//! clearly-bounded nudge that the scoring layer MAY apply.
//!
//! Guardrails baked in (project rules):
//!   * **No fabrication / no over-promising.** A handful of clicks must not swing a
//!     recommendation. `suggested_delta` is capped at ±`MAX_DELTA` and damped to
//!     near-zero below `FULL_WEIGHT_SAMPLE`, so low-sample feedback is cosmetic.
//!   * **Privacy.** Pure function over already-local rows — no network, no PII;
//!     the caller passes rows it already read from the local DB.
//!   * **Engine purity.** No DB/IO here. The command layer reads rows and passes
//!     the aggregated map into `ScoringContext` (future wiring — see the audit doc
//!     `docs/audit/feedback-loop-v1.md`), exactly like `meta_rates`.
use std::collections::HashMap;

/// Below this many data points the signal is treated as anecdotal: confidence is
/// "low" and the delta is damped toward zero.
const MIN_SAMPLE: u32 = 3;
/// At/above this many data points the signal carries its full (still small) weight.
const FULL_WEIGHT_SAMPLE: u32 = 8;
/// Hard cap on how much aggregated feedback may move a score. Deliberately tiny:
/// feedback is a tie-breaker / personalization nudge, never a ranking driver.
pub const MAX_DELTA: f32 = 0.03;

/// Verdict parsed defensively from the free-form `feedback` string the UI stores.
/// Canonical v1 vocabulary: `helpful` / `not_helpful` / `picked` / `skipped`.
/// `Picked` is a *weak* positive (acting on a rec is a quieter endorsement than an
/// explicit thumbs-up); `Skipped`/`Unknown` are non-signals, never penalties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackVerdict {
    /// Strong positive — explicitly marked helpful.
    Helpful,
    /// Weak positive — the user picked the champ (acted on it).
    Picked,
    /// Strong negative — explicitly marked not helpful.
    NotHelpful,
    /// Shown but not chosen — deliberately neutral (no penalty).
    Skipped,
    /// Unrecognised string — ignored.
    Unknown,
}

impl FeedbackVerdict {
    /// Map the stored string to a verdict. Synonyms + thumb emojis are accepted so
    /// a UI wording change doesn't silently drop signal; anything else is `Unknown`.
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_lowercase().as_str() {
            "helpful" | "up" | "yes" | "good" | "like" | "useful" | "👍" => {
                FeedbackVerdict::Helpful
            }
            "picked" | "lock" | "locked" | "selected" => FeedbackVerdict::Picked,
            "not_helpful" | "not-helpful" | "down" | "no" | "bad" | "dislike" | "useless"
            | "👎" => FeedbackVerdict::NotHelpful,
            "skipped" | "skip" | "passed" => FeedbackVerdict::Skipped,
            _ => FeedbackVerdict::Unknown,
        }
    }

    /// Signed sentiment contribution: Helpful +1.0, Picked +0.5 (weak), NotHelpful
    /// −1.0, Skipped/Unknown 0.0. The weak `Picked` weight is what keeps "acted on
    /// it" from counting as loudly as an explicit endorsement.
    pub fn weight(self) -> f32 {
        match self {
            FeedbackVerdict::Helpful => 1.0,
            FeedbackVerdict::Picked => 0.5,
            FeedbackVerdict::NotHelpful => -1.0,
            FeedbackVerdict::Skipped | FeedbackVerdict::Unknown => 0.0,
        }
    }

    /// True when the verdict counts toward the polar sample. Skipped/Unknown do not
    /// — they are excluded entirely (no signal, no nudge).
    pub fn is_polar(self) -> bool {
        matches!(
            self,
            FeedbackVerdict::Helpful | FeedbackVerdict::Picked | FeedbackVerdict::NotHelpful
        )
    }
}

/// One already-local feedback row, reduced to what aggregation needs. The command
/// layer builds these from `recommendation_feedback` rows. `Deserialize` is
/// additive — the JSON API (`feedback_signals_from_json`) accepts raw rows.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FeedbackInput {
    pub champion_id: u32,
    /// Raw stored verdict string (parsed defensively).
    pub verdict: String,
}

/// Aggregated per-champion feedback read. `suggested_delta` is the only value the
/// scoring layer should consume; the counts are for transparency / UI.
/// `Serialize` is additive — `feedback_signals_from_json` returns these to the JS
/// host in exactly the shape `RecommendationsInput.feedback_signals` accepts back.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FeedbackSignal {
    pub champion_id: u32,
    pub positive: u32,
    pub negative: u32,
    /// Polarised data points (`positive + negative`; skipped/unknown excluded).
    pub sample: u32,
    /// Weighted sentiment `Σ weight / sample` in [-1.0, 1.0] (Helpful +1, Picked
    /// +0.5, NotHelpful −1). 0.0 when balanced. `positive`/`negative` are raw counts
    /// for transparency; this is what drives the nudge.
    pub net_sentiment: f32,
    /// "low" | "medium" | "high" — purely sample-size driven.
    pub confidence: &'static str,
    /// Bounded score nudge in [-MAX_DELTA, MAX_DELTA], damped by sample size.
    pub suggested_delta: f32,
}

fn confidence_for(sample: u32) -> &'static str {
    if sample >= FULL_WEIGHT_SAMPLE {
        "high"
    } else if sample >= MIN_SAMPLE {
        "medium"
    } else {
        "low"
    }
}

/// Sample-size damping in [0.0, 1.0]: 0 below `MIN_SAMPLE`, ramping linearly to 1
/// at `FULL_WEIGHT_SAMPLE`. Keeps a couple of clicks from moving anything.
fn damping(sample: u32) -> f32 {
    if sample < MIN_SAMPLE {
        return 0.0;
    }
    let span = (FULL_WEIGHT_SAMPLE - MIN_SAMPLE).max(1) as f32;
    (((sample - MIN_SAMPLE) as f32 / span) + (1.0 / span)).min(1.0)
}

/// Aggregate local feedback rows into a per-champion signal map. Neutral verdicts
/// are counted toward neither polarity. Champions with no polar feedback are
/// omitted (no signal == no nudge).
pub fn aggregate_feedback(rows: &[FeedbackInput]) -> HashMap<u32, FeedbackSignal> {
    // (positive count, negative count, weighted sentiment sum)
    let mut tally: HashMap<u32, (u32, u32, f32)> = HashMap::new();
    for row in rows {
        let verdict = FeedbackVerdict::parse(&row.verdict);
        if !verdict.is_polar() {
            continue; // Skipped / Unknown contribute nothing.
        }
        let entry = tally.entry(row.champion_id).or_insert((0, 0, 0.0));
        if verdict.weight() > 0.0 {
            entry.0 += 1;
        } else {
            entry.1 += 1;
        }
        entry.2 += verdict.weight();
    }

    tally
        .into_iter()
        .filter_map(|(champion_id, (positive, negative, weighted))| {
            let sample = positive + negative;
            if sample == 0 {
                return None;
            }
            let net_sentiment = (weighted / sample as f32).clamp(-1.0, 1.0);
            let suggested_delta =
                (net_sentiment * MAX_DELTA * damping(sample)).clamp(-MAX_DELTA, MAX_DELTA);
            Some((
                champion_id,
                FeedbackSignal {
                    champion_id,
                    positive,
                    negative,
                    sample,
                    net_sentiment,
                    confidence: confidence_for(sample),
                    suggested_delta,
                },
            ))
        })
        .collect()
}

/// Apply a signal's bounded nudge to a base score, keeping the result non-negative.
/// Pure — the scoring layer calls this when a champion has a signal.
pub fn apply_feedback(base_score: f32, signal: &FeedbackSignal) -> f32 {
    (base_score + signal.suggested_delta).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fb(champion_id: u32, verdict: &str) -> FeedbackInput {
        FeedbackInput {
            champion_id,
            verdict: verdict.to_string(),
        }
    }

    #[test]
    fn verdict_parsing_is_defensive() {
        assert_eq!(FeedbackVerdict::parse("helpful"), FeedbackVerdict::Helpful);
        assert_eq!(FeedbackVerdict::parse(" Picked "), FeedbackVerdict::Picked);
        assert_eq!(
            FeedbackVerdict::parse("not_helpful"),
            FeedbackVerdict::NotHelpful
        );
        assert_eq!(FeedbackVerdict::parse("👎"), FeedbackVerdict::NotHelpful);
        assert_eq!(FeedbackVerdict::parse("skipped"), FeedbackVerdict::Skipped);
        // Unknown / ambiguous never gets guessed into a polarity.
        assert_eq!(FeedbackVerdict::parse("meh"), FeedbackVerdict::Unknown);
        assert_eq!(FeedbackVerdict::parse(""), FeedbackVerdict::Unknown);
    }

    #[test]
    fn verdict_weights_encode_strength() {
        assert_eq!(FeedbackVerdict::Helpful.weight(), 1.0);
        assert_eq!(FeedbackVerdict::Picked.weight(), 0.5); // weak positive
        assert_eq!(FeedbackVerdict::NotHelpful.weight(), -1.0);
        assert_eq!(FeedbackVerdict::Skipped.weight(), 0.0);
        assert_eq!(FeedbackVerdict::Unknown.weight(), 0.0);
        assert!(FeedbackVerdict::Picked.is_polar());
        assert!(!FeedbackVerdict::Skipped.is_polar());
        assert!(!FeedbackVerdict::Unknown.is_polar());
    }

    #[test]
    fn picked_is_weaker_positive_than_helpful() {
        let helpful: Vec<FeedbackInput> = (0..8).map(|_| fb(1, "helpful")).collect();
        let picked: Vec<FeedbackInput> = (0..8).map(|_| fb(2, "picked")).collect();
        let h = aggregate_feedback(&helpful);
        let p = aggregate_feedback(&picked);
        let hs = &h[&1];
        let ps = &p[&2];
        // Both positive, both confident, but picked pulls less hard.
        assert!(ps.suggested_delta > 0.0, "picked is still positive");
        assert!(
            ps.suggested_delta < hs.suggested_delta,
            "picked ({}) must nudge less than helpful ({})",
            ps.suggested_delta,
            hs.suggested_delta
        );
        // 8 picked → net 0.5 → ~half of helpful's cap.
        assert!((ps.net_sentiment - 0.5).abs() < 1e-4);
        assert!((hs.net_sentiment - 1.0).abs() < 1e-4);
        // Picked counts as a positive data point for transparency.
        assert_eq!(ps.positive, 8);
        assert_eq!(ps.negative, 0);
    }

    #[test]
    fn skipped_only_yields_no_signal() {
        let sig = aggregate_feedback(&[fb(4, "skipped"), fb(4, "skip"), fb(4, "passed")]);
        assert!(
            !sig.contains_key(&4),
            "skipped is neutral → never a signal, never a penalty"
        );
    }

    #[test]
    fn unknown_verdict_is_ignored() {
        // Unknown strings must not be coerced into a polarity, even mixed with one
        // real positive (which alone is below MIN_SAMPLE → delta 0).
        let sig = aggregate_feedback(&[fb(6, "wat"), fb(6, "??"), fb(6, "helpful")]);
        let s = &sig[&6];
        assert_eq!(s.sample, 1, "only the one real positive counts");
        assert_eq!(s.positive, 1);
        assert_eq!(s.suggested_delta, 0.0, "1 polar point stays cosmetic");
    }

    #[test]
    fn low_sample_feedback_is_cosmetic() {
        // Two positives → below MIN_SAMPLE damping → delta is exactly zero.
        let sig = aggregate_feedback(&[fb(1, "helpful"), fb(1, "helpful")]);
        let s = &sig[&1];
        assert_eq!(s.sample, 2);
        assert_eq!(s.confidence, "low");
        assert_eq!(s.suggested_delta, 0.0, "2 clicks must not move the score");
        assert_eq!(s.net_sentiment, 1.0);
    }

    #[test]
    fn confident_positive_signal_nudges_up_within_cap() {
        let rows: Vec<FeedbackInput> = (0..10).map(|_| fb(7, "helpful")).collect();
        let sig = aggregate_feedback(&rows);
        let s = &sig[&7];
        assert_eq!(s.confidence, "high");
        assert!(s.suggested_delta > 0.0);
        assert!(
            s.suggested_delta <= MAX_DELTA + f32::EPSILON,
            "delta must never exceed the cap: {}",
            s.suggested_delta
        );
        // All-positive at full weight → essentially the cap.
        assert!((s.suggested_delta - MAX_DELTA).abs() < 1e-4);
    }

    #[test]
    fn mixed_feedback_nets_out() {
        let mut rows = vec![];
        for _ in 0..6 {
            rows.push(fb(3, "helpful"));
        }
        for _ in 0..6 {
            rows.push(fb(3, "not_helpful"));
        }
        let sig = aggregate_feedback(&rows);
        let s = &sig[&3];
        assert_eq!(s.sample, 12);
        assert_eq!(s.net_sentiment, 0.0);
        assert_eq!(s.suggested_delta, 0.0, "balanced feedback → no nudge");
    }

    #[test]
    fn negative_signal_nudges_down_and_clamps_nonnegative() {
        let rows: Vec<FeedbackInput> = (0..10).map(|_| fb(9, "not_helpful")).collect();
        let sig = aggregate_feedback(&rows);
        let s = &sig[&9];
        assert!(s.suggested_delta < 0.0);
        assert!(s.suggested_delta >= -MAX_DELTA - f32::EPSILON);
        // Applying to a tiny base must not go negative.
        assert_eq!(apply_feedback(0.01, s), 0.0);
        // Applying to a normal base subtracts the (small) delta.
        let adjusted = apply_feedback(0.50, s);
        assert!(adjusted < 0.50 && adjusted > 0.45);
    }

    #[test]
    fn neutral_only_champion_has_no_signal() {
        let sig = aggregate_feedback(&[fb(5, "skipped"), fb(5, "meh")]);
        assert!(
            !sig.contains_key(&5),
            "no polar feedback → no signal → no nudge"
        );
    }

    #[test]
    fn delta_is_bounded_across_a_fuzz_of_ratios() {
        // No combination of counts may push the delta past the cap.
        for pos in 0..30u32 {
            for neg in 0..30u32 {
                if pos + neg == 0 {
                    continue;
                }
                let mut rows = vec![];
                for _ in 0..pos {
                    rows.push(fb(1, "helpful"));
                }
                for _ in 0..neg {
                    rows.push(fb(1, "not_helpful"));
                }
                if let Some(s) = aggregate_feedback(&rows).get(&1) {
                    assert!(
                        s.suggested_delta.abs() <= MAX_DELTA + 1e-6,
                        "pos={pos} neg={neg} delta={} exceeded cap",
                        s.suggested_delta
                    );
                }
            }
        }
    }
}
