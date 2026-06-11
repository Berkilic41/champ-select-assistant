//! Feedback observability summary (Feedback Loop v1 — Sprint C, Claude).
//!
//! A small, pure read over local feedback rows for the data-quality surface: how
//! much feedback exists, how much of it is *polar* (actually signals something),
//! how many champions carry an aggregated signal, and how much is awaiting cloud
//! sync. Designed to bind to the DraftBrain quality panel later.
//!
//! Pure — no IO. The command layer (`commands/data_quality.rs`) reads the rows +
//! the pending-sync count and calls `summarize_observability`. Reuses the verdict
//! classifier from `feedback_signal` so the polar/neutral split can never drift
//! from the scoring path.

use crate::feedback_signal::{aggregate_feedback, FeedbackInput, FeedbackVerdict};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Observability counters for the feedback loop. All `u32` → all `number` in TS
/// (no `serde_json::Value`, so this exports cleanly unlike the input struct).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct FeedbackObservability {
    /// Every stored feedback row.
    pub total: u32,
    /// Rows whose verdict carries polarity (helpful / picked / not_helpful).
    pub polar: u32,
    /// Rows the parser treats as non-signals (skipped / unknown).
    pub neutral: u32,
    /// Champions that end up with an aggregated `FeedbackSignal`.
    pub active_champion_signals: u32,
    /// Rows not yet flushed to the cloud (`synced_at IS NULL`).
    pub pending_sync: u32,
}

/// Build the observability summary from already-local rows + the pending count.
/// `pending_sync` is passed in because sync state (`synced_at`) is a DB column, not
/// part of the pure `FeedbackInput`.
pub fn summarize_observability(rows: &[FeedbackInput], pending_sync: u32) -> FeedbackObservability {
    let total = rows.len() as u32;
    let polar = rows
        .iter()
        .filter(|r| FeedbackVerdict::parse(&r.verdict).is_polar())
        .count() as u32;
    let active_champion_signals = aggregate_feedback(rows).len() as u32;
    FeedbackObservability {
        total,
        polar,
        neutral: total - polar,
        active_champion_signals,
        pending_sync,
    }
}

/// Canonical, UI-facing status token for the "is personalization working?" card.
/// Generated as a TS union (`"no_signal" | "warming_up" | "active" | "needs_sync"`)
/// so the quality card can switch on it without re-deriving the priority in TS.
/// type + tests are the deliverable; see docs/audit/feedback-loop-v1.md §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub enum FeedbackPersonalizationStatus {
    /// No polar feedback at all — nothing to learn from yet.
    NoSignal,
    /// Polar feedback exists but not enough on any one champion to clear the
    /// sample gate → no active nudge yet (it's warming up).
    WarmingUp,
    /// At least one champion carries an aggregated signal nudging recommendations.
    Active,
    /// Local feedback is waiting to flush to the cloud. Takes PRIORITY in the UI:
    /// "is there data pending sync?" is answered before "is personalization on?".
    NeedsSync,
}

/// Map observability counters to the canonical status. Priority (deliberate, see
/// the audit doc): `pending_sync` first, then active signals, then warming up,
/// then no signal. Pure — the command/UI layer reads the same token.
pub fn personalization_status(obs: &FeedbackObservability) -> FeedbackPersonalizationStatus {
    if obs.pending_sync > 0 {
        FeedbackPersonalizationStatus::NeedsSync
    } else if obs.active_champion_signals > 0 {
        FeedbackPersonalizationStatus::Active
    } else if obs.polar > 0 {
        FeedbackPersonalizationStatus::WarmingUp
    } else {
        FeedbackPersonalizationStatus::NoSignal
    }
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
    fn summary_counts_polarity_and_signals() {
        let rows = vec![
            fb(1, "helpful"),
            fb(1, "helpful"),
            fb(1, "helpful"), // champ 1 → 3 polar → a signal
            fb(2, "skipped"), // neutral
            fb(2, "wat"),     // unknown → neutral
            fb(3, "not_helpful"),
            fb(3, "not_helpful"),
            fb(3, "not_helpful"), // champ 3 → 3 polar → a signal
        ];
        let s = summarize_observability(&rows, 4);
        assert_eq!(s.total, 8);
        assert_eq!(s.polar, 6);
        assert_eq!(s.neutral, 2);
        assert_eq!(s.active_champion_signals, 2, "champs 1 and 3 carry signals");
        assert_eq!(s.pending_sync, 4);
    }

    #[test]
    fn empty_feedback_is_all_zero() {
        let s = summarize_observability(&[], 0);
        assert_eq!(
            s,
            FeedbackObservability {
                total: 0,
                polar: 0,
                neutral: 0,
                active_champion_signals: 0,
                pending_sync: 0,
            }
        );
    }

    #[test]
    fn neutral_only_has_no_active_signals() {
        let rows = vec![fb(1, "skipped"), fb(2, "passed"), fb(3, "unknown-token")];
        let s = summarize_observability(&rows, 0);
        assert_eq!(s.total, 3);
        assert_eq!(s.polar, 0);
        assert_eq!(s.neutral, 3);
        assert_eq!(s.active_champion_signals, 0);
    }

    fn obs(total: u32, polar: u32, active: u32, pending: u32) -> FeedbackObservability {
        FeedbackObservability {
            total,
            polar,
            neutral: total - polar,
            active_champion_signals: active,
            pending_sync: pending,
        }
    }

    #[test]
    fn status_no_signal_when_empty() {
        assert_eq!(
            personalization_status(&obs(0, 0, 0, 0)),
            FeedbackPersonalizationStatus::NoSignal
        );
    }

    #[test]
    fn status_warming_up_when_polar_but_no_active_signal() {
        // Polar feedback exists (e.g. 2 helpful on one champ) but below the sample
        // gate → no active signal, nothing pending.
        assert_eq!(
            personalization_status(&obs(2, 2, 0, 0)),
            FeedbackPersonalizationStatus::WarmingUp
        );
    }

    #[test]
    fn status_active_when_a_champion_carries_a_signal() {
        assert_eq!(
            personalization_status(&obs(10, 10, 1, 0)),
            FeedbackPersonalizationStatus::Active
        );
    }

    #[test]
    fn status_needs_sync_takes_priority_over_active() {
        // Even with an active signal, pending sync wins (UI answers "data waiting
        // to sync?" before "personalization on?").
        assert_eq!(
            personalization_status(&obs(10, 10, 1, 3)),
            FeedbackPersonalizationStatus::NeedsSync
        );
        // And over warming-up / no-signal too.
        assert_eq!(
            personalization_status(&obs(2, 2, 0, 2)),
            FeedbackPersonalizationStatus::NeedsSync
        );
    }

    /// Cross-language drift guard: the canonical verdict vocabulary is shared with
    /// the TS contract (`src/types/feedback-vocabulary.json`). A verdict added to
    /// the UI vocabulary that the Rust parser doesn't recognise turns this red, and
    /// vice-versa the TS side (`feedback.test.ts`) guards the union ↔ JSON match.
    #[test]
    fn rust_parser_recognizes_every_canonical_verdict() {
        const VOCAB: &str = include_str!("../../src/types/feedback-vocabulary.json");
        let parsed: serde_json::Value = serde_json::from_str(VOCAB).expect("vocab json parses");
        let verdicts = parsed["verdicts"].as_array().expect("verdicts is an array");
        assert_eq!(verdicts.len(), 4, "canonical vocabulary size");
        for v in verdicts {
            let token = v.as_str().expect("verdict is a string");
            assert_ne!(
                FeedbackVerdict::parse(token),
                FeedbackVerdict::Unknown,
                "Rust parser must recognise canonical verdict '{token}'"
            );
        }
        // Lock the polarity the scoring path depends on for the contract tokens.
        assert_eq!(FeedbackVerdict::parse("helpful").weight(), 1.0);
        assert_eq!(FeedbackVerdict::parse("not_helpful").weight(), -1.0);
        assert_eq!(FeedbackVerdict::parse("picked").weight(), 0.5);
        assert!(!FeedbackVerdict::parse("skipped").is_polar());
    }
}
