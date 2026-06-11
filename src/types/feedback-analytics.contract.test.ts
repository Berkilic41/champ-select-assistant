import { describe, it, expect } from 'vitest';
import type { FeedbackAnalytics } from './generated/FeedbackAnalytics';
import type { ChampionFeedbackTrend } from './generated/ChampionFeedbackTrend';

// Compile-time contract guard for the read-only feedback analytics surface
// (Sprint D). ts-rs regenerates these on a Rust struct change; a field
// added/removed/renamed/retyped then fails `pnpm typecheck` (and this test) before
// it reaches the analytics card binding. All counters must stay `number` (not bigint).

describe('ChampionFeedbackTrend contract', () => {
  it('has exactly the expected fields with numeric counters', () => {
    const keys: Record<keyof ChampionFeedbackTrend, true> = {
      champion_id: true,
      champion_key: true,
      helpful: true,
      picked: true,
      not_helpful: true,
      sample: true,
      net_sentiment: true,
      recent_count: true,
    };
    expect(Object.keys(keys).sort()).toEqual([
      'champion_id',
      'champion_key',
      'helpful',
      'net_sentiment',
      'not_helpful',
      'picked',
      'recent_count',
      'sample',
    ]);

    const trend: ChampionFeedbackTrend = {
      champion_id: 238,
      champion_key: 'Zed',
      helpful: 2,
      picked: 1,
      not_helpful: 0,
      sample: 3,
      net_sentiment: 0.83,
      recent_count: 3,
    };
    for (const numeric of [
      trend.champion_id,
      trend.helpful,
      trend.picked,
      trend.not_helpful,
      trend.sample,
      trend.net_sentiment,
      trend.recent_count,
    ]) {
      expect(typeof numeric).toBe('number');
    }
  });
});

describe('FeedbackAnalytics contract', () => {
  it('wraps the window meta plus trend lists', () => {
    const keys: Record<keyof FeedbackAnalytics, true> = {
      window_days: true,
      total_events: true,
      recent_signal_count: true,
      trends: true,
      disliked: true,
    };
    const sample: FeedbackAnalytics = {
      window_days: 7,
      total_events: 0,
      recent_signal_count: 0,
      trends: [],
      disliked: [],
    };
    expect(Object.keys(keys).sort()).toEqual([
      'disliked',
      'recent_signal_count',
      'total_events',
      'trends',
      'window_days',
    ]);
    expect(typeof sample.window_days).toBe('number');
    expect(Array.isArray(sample.disliked)).toBe(true);
  });
});
