import { describe, it, expect } from 'vitest';
import type { FeedbackFlushSummary } from './generated/FeedbackFlushSummary';

// Compile-time contract guard for the feedback flush summary (Sprint E). Codex's
// sync UX binds `sync_recommendation_feedback` → this shape. A Rust field
// add/remove/retype breaks `pnpm typecheck` here before the button binding drifts.

describe('FeedbackFlushSummary contract', () => {
  it('has the expected sync-run counters', () => {
    const keys: Record<keyof FeedbackFlushSummary, true> = {
      offline: true,
      attempted: true,
      synced: true,
      failed: true,
      skipped_no_hash: true,
    };
    expect(Object.keys(keys).sort()).toEqual([
      'attempted',
      'failed',
      'offline',
      'skipped_no_hash',
      'synced',
    ]);

    const sample: FeedbackFlushSummary = {
      offline: false,
      attempted: 3,
      synced: 2,
      failed: 1,
      skipped_no_hash: 0,
    };
    expect(typeof sample.offline).toBe('boolean');
    expect(sample.attempted).toBe(sample.synced + sample.failed);
  });
});
