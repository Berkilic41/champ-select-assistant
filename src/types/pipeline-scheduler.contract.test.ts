import { describe, it, expect } from 'vitest';
import type { RefreshPlan } from './generated/RefreshPlan';
import type { RefreshSourceDecision } from './generated/RefreshSourceDecision';
import type { RateLimitBudget } from './generated/RateLimitBudget';
import type { SourceFetchHealth } from './generated/SourceFetchHealth';
import type { FetchLogSummary } from './generated/FetchLogSummary';
import type { PipelineSchedulerStatus } from './generated/PipelineSchedulerStatus';

// Compile-time contract guard for the scheduler policy outputs (Sprint). A Rust
// field add/remove/retype regenerates these and breaks `pnpm typecheck` here before
// any scheduler/observability panel binding can drift.
//
// ts-rs maps the i64 unix-second fields (`next_allowed_at`, `window_secs`,
// `last_success_at`, `last_attempt_at`) to `bigint`; counts are `number`.

const decision: RefreshSourceDecision = {
  source: 'meraki',
  decision: 'refresh',
  reason: 'r',
  next_allowed_at: null,
};

const health: SourceFetchHealth = {
  source: 'meraki',
  total: 5,
  success: 4,
  failed: 1,
  success_streak: 2,
  fail_streak: 0,
  last_success_at: BigInt(1_800_000_000),
  last_attempt_at: BigInt(1_800_000_000),
  health: 'healthy',
};

describe('Scheduler policy contract', () => {
  it('RefreshPlan + RefreshSourceDecision shapes', () => {
    const planKeys: Record<keyof RefreshPlan, true> = {
      decisions: true,
      refresh_count: true,
      champ_select_blocked: true,
    };
    const decKeys: Record<keyof RefreshSourceDecision, true> = {
      source: true,
      decision: true,
      reason: true,
      next_allowed_at: true,
    };
    expect(Object.keys(planKeys).sort()).toEqual(['champ_select_blocked', 'decisions', 'refresh_count']);
    expect(Object.keys(decKeys).length).toBe(4);

    const plan: RefreshPlan = { decisions: [decision], refresh_count: 1, champ_select_blocked: false };
    expect(typeof plan.refresh_count).toBe('number');
    expect(typeof plan.champ_select_blocked).toBe('boolean');
    expect(plan.decisions[0].next_allowed_at).toBeNull();
  });

  it('RateLimitBudget shape + bigint timestamp', () => {
    const keys: Record<keyof RateLimitBudget, true> = {
      max_requests: true,
      used: true,
      remaining: true,
      window_secs: true,
      next_allowed_at: true,
    };
    const budget: RateLimitBudget = {
      max_requests: 20,
      used: 5,
      remaining: 15,
      window_secs: BigInt(3600),
      next_allowed_at: null,
    };
    expect(Object.keys(keys).length).toBe(5);
    expect(typeof budget.remaining).toBe('number');
    expect(typeof budget.window_secs).toBe('bigint');
  });

  it('SourceFetchHealth + FetchLogSummary shapes', () => {
    const hKeys: Record<keyof SourceFetchHealth, true> = {
      source: true, total: true, success: true, failed: true, success_streak: true,
      fail_streak: true, last_success_at: true, last_attempt_at: true, health: true,
    };
    const sKeys: Record<keyof FetchLogSummary, true> = {
      sources: true, total_entries: true, healthy_count: true, degraded_count: true,
    };
    expect(Object.keys(hKeys).length).toBe(9);
    expect(Object.keys(sKeys).length).toBe(4);

    const summary: FetchLogSummary = {
      sources: [health],
      total_entries: 5,
      healthy_count: 1,
      degraded_count: 0,
    };
    expect(summary.sources[0].health).toBe('healthy');
    expect(typeof summary.sources[0].last_success_at).toBe('bigint');
    expect(typeof summary.total_entries).toBe('number');
  });

  it('PipelineSchedulerStatus wraps rate_limit + fetch_logs + plan', () => {
    // The get_pipeline_scheduler_status command response. Locks that it composes
    // the policy outputs (a Rust field change breaks here before the panel drifts).
    const keys: Record<keyof PipelineSchedulerStatus, true> = {
      champ_select_active: true,
      rate_limit: true,
      fetch_logs: true,
      plan: true,
    };
    expect(Object.keys(keys).sort()).toEqual([
      'champ_select_active',
      'fetch_logs',
      'plan',
      'rate_limit',
    ]);

    const status: PipelineSchedulerStatus = {
      champ_select_active: false,
      rate_limit: { max_requests: 6, used: 1, remaining: 5, window_secs: BigInt(3600), next_allowed_at: null },
      fetch_logs: { sources: [health], total_entries: 5, healthy_count: 1, degraded_count: 0 },
      plan: { decisions: [decision], refresh_count: 1, champ_select_blocked: false },
    };
    expect(typeof status.champ_select_active).toBe('boolean');
    expect(status.rate_limit.max_requests).toBe(6);
    expect(status.plan.decisions[0].decision).toBe('refresh');
  });
});
