import { describe, it, expect } from 'vitest';
import type { FeedbackObservability } from './generated/FeedbackObservability';
import type { FeedbackObservabilityReport } from './generated/FeedbackObservabilityReport';
import type { FeedbackPersonalizationStatus } from './generated/FeedbackPersonalizationStatus';

// Compile-time contract guard for the feedback observability surface (Sprint C).
// If a Rust field is added/removed/renamed/retyped, ts-rs regenerates these types
// and `pnpm typecheck` (then this test) fails — catching backend/frontend drift
// before it reaches the quality card binding.

describe('FeedbackObservability contract', () => {
  it('has exactly the five counters, nothing more', () => {
    // Exhaustive key map: `keyof FeedbackObservability` forces exactly these keys —
    // a new field => missing key (compile error); a removed/renamed field => the
    // listed key is no longer assignable (compile error).
    const keys: Record<keyof FeedbackObservability, true> = {
      total: true,
      polar: true,
      neutral: true,
      active_champion_signals: true,
      pending_sync: true,
    };
    expect(Object.keys(keys).sort()).toEqual([
      'active_champion_signals',
      'neutral',
      'pending_sync',
      'polar',
      'total',
    ]);
  });

  it('every counter is number, not bigint', () => {
    // Compile-time: each field is typed `number`. If a field flips to i64 → bigint,
    // this literal stops compiling.
    const sample: FeedbackObservability = {
      total: 0,
      polar: 0,
      neutral: 0,
      active_champion_signals: 0,
      pending_sync: 0,
    };
    for (const value of Object.values(sample)) {
      expect(typeof value).toBe('number');
    }
  });
});

describe('FeedbackObservabilityReport contract', () => {
  it('wraps counters plus the Rust-derived status token', () => {
    const reportKeys: Record<keyof FeedbackObservabilityReport, true> = {
      counters: true,
      status: true,
    };
    const sample: FeedbackObservabilityReport = {
      counters: {
        total: 1,
        polar: 1,
        neutral: 0,
        active_champion_signals: 0,
        pending_sync: 0,
      },
      status: 'warming_up',
    };

    expect(Object.keys(reportKeys).sort()).toEqual(['counters', 'status']);
    expect(sample.status).toBe('warming_up');
  });
});

describe('FeedbackPersonalizationStatus contract', () => {
  it('is exactly the four canonical status tokens', () => {
    // Exhaustive over the generated union — a Rust variant add/remove breaks here.
    const tokens: Record<FeedbackPersonalizationStatus, true> = {
      no_signal: true,
      warming_up: true,
      active: true,
      needs_sync: true,
    };
    expect(Object.keys(tokens).sort()).toEqual([
      'active',
      'needs_sync',
      'no_signal',
      'warming_up',
    ]);
  });
});
