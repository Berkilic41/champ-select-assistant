// Canonical Feedback Loop v1 contract (Claude, Sprint C) — the single source the
// UI imports when calling `submit_recommendation_feedback`.
//
// Why hand-authored: the Rust `RecommendationFeedbackInput` (commands/draft_brain.rs)
// is `Deserialize`-only. Its `payload: Option<serde_json::Value>` field blocks a
// clean `#[derive(TS)]` export, so we model the payload here with the TS-safe
// `Record<string, unknown>` mapping. The proposed Rust-side fix is documented in
// docs/audit/feedback-loop-v1.md (`#[ts(type = "Record<string, unknown> | null")]`).
//
// `feedback-vocabulary.json` is the shared source of truth: the Rust verdict parser
// (recommendation/feedback_observability.rs test) and this union both check against
// it, so the vocabularies cannot drift apart silently.
import vocabulary from './feedback-vocabulary.json';

/** Canonical verdict the UI must send. Synonyms/emojis are accepted by the Rust
 *  parser but the UI should emit exactly these. */
export type FeedbackVerdict = 'helpful' | 'not_helpful' | 'picked' | 'skipped';

/** Runtime list of canonical verdicts (shared JSON; drift-guarded by tests). */
export const FEEDBACK_VERDICTS = vocabulary.verdicts as readonly FeedbackVerdict[];

/** Payload for `invoke('submit_recommendation_feedback', { feedback })`.
 *  camelCase — the Rust struct uses serde `rename_all = "camelCase"`. */
export interface RecommendationFeedbackPayload {
  championId: number;
  championKey: string;
  feedback: FeedbackVerdict;
  sessionHash?: string;
  modelVersion?: string;
  score?: number;
  payload?: Record<string, unknown>;
}

/** Local offline-queue state for a stored feedback row. `pending` until flushed to
 *  `/v1/recommendation-feedback`; `failed` after a sync attempt errored (retryable). */
export type FeedbackQueueState = 'pending' | 'synced' | 'failed';

/** A locally-queued feedback row. Mirrors `recommendation_feedback`: `syncedAt`
 *  null == not yet flushed. `state` derives from `syncedAt` + last sync result. */
export interface QueuedFeedback {
  payload: RecommendationFeedbackPayload;
  createdAt: number;
  syncedAt: number | null;
  state: FeedbackQueueState;
}
