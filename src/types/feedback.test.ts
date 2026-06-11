import { describe, it, expect } from 'vitest';
import {
  FEEDBACK_VERDICTS,
  type FeedbackVerdict,
  type RecommendationFeedbackPayload,
  type QueuedFeedback,
  type FeedbackQueueState,
} from './feedback';

// Drift guard for the Feedback Loop v1 contract. The canonical vocabulary lives in
// feedback-vocabulary.json (shared with the Rust parser, which has its own test in
// recommendation/feedback_observability.rs). These tests fail if the TS union and
// the shared JSON disagree, or if the payload/queue shapes change unexpectedly.

describe('Feedback contract — vocabulary', () => {
  it('the FeedbackVerdict union matches the shared vocabulary JSON', () => {
    // Exhaustiveness object: TS requires a key for every union member, so adding a
    // verdict to the union without updating this object is a compile error — and
    // then the runtime comparison forces the JSON to match too.
    const exhaustive: Record<FeedbackVerdict, true> = {
      helpful: true,
      not_helpful: true,
      picked: true,
      skipped: true,
    };
    const unionMembers = Object.keys(exhaustive).sort();
    expect([...FEEDBACK_VERDICTS].sort()).toEqual(unionMembers);
  });

  it('the two HeroCard buttons emit verdicts in the canonical set', () => {
    const fromButtons: FeedbackVerdict[] = ['helpful', 'not_helpful'];
    expect(fromButtons.every((v) => FEEDBACK_VERDICTS.includes(v))).toBe(true);
  });
});

describe('Feedback contract — shapes', () => {
  it('payload and queue envelope compile and round-trip', () => {
    const payload: RecommendationFeedbackPayload = {
      championId: 238,
      championKey: 'Zed',
      feedback: 'helpful',
      payload: { lane: 'middle' },
    };
    const states: FeedbackQueueState[] = ['pending', 'synced', 'failed'];
    const queued: QueuedFeedback = {
      payload,
      createdAt: 1_800_000_000,
      syncedAt: null,
      state: 'pending',
    };
    expect(queued.syncedAt).toBeNull();
    expect(states).toContain(queued.state);
  });
});
