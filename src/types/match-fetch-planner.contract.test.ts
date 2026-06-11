import { describe, it, expect } from 'vitest';
import type { MatchFetchPlan } from './generated/MatchFetchPlan';
import type { MatchFetchDecision } from './generated/MatchFetchDecision';

// Compile-time contract guard for the match fetch planner output (Sprint). A Rust
// field add/remove/retype regenerates these and breaks `pnpm typecheck` here before
// Codex's batch-fetch binding can drift. No bigint (counts/priority are number;
// timestamps live in the Rust-only input).

const decision: MatchFetchDecision = {
  match_id: 'EUW1_1',
  decision: 'fetch',
  reason: 'r',
  priority: 5,
};

describe('Match fetch planner contract', () => {
  it('MatchFetchDecision shape + decision token', () => {
    const keys: Record<keyof MatchFetchDecision, true> = {
      match_id: true,
      decision: true,
      reason: true,
      priority: true,
    };
    expect(Object.keys(keys).sort()).toEqual(['decision', 'match_id', 'priority', 'reason']);
    const decisions: Array<MatchFetchDecision['decision']> = [
      'fetch',
      'skip_already_fetched',
      'skip_rate_limited',
      'skip_champ_select',
      'skip_batch_full',
      'skip_invalid',
      'skip_no_gap',
    ];
    expect(decisions).toContain(decision.decision);
    expect(typeof decision.priority).toBe('number');
  });

  it('MatchFetchPlan bundles to_fetch + decisions + counts', () => {
    const keys: Record<keyof MatchFetchPlan, true> = {
      to_fetch: true,
      decisions: true,
      batch_limit: true,
      selected_count: true,
      skipped_count: true,
    };
    expect(Object.keys(keys).sort()).toEqual([
      'batch_limit',
      'decisions',
      'selected_count',
      'skipped_count',
      'to_fetch',
    ]);

    const plan: MatchFetchPlan = {
      to_fetch: ['EUW1_1'],
      decisions: [decision],
      batch_limit: 6,
      selected_count: 1,
      skipped_count: 0,
    };
    expect(Array.isArray(plan.to_fetch)).toBe(true);
    expect(plan.selected_count + plan.skipped_count).toBe(1);
    expect(typeof plan.batch_limit).toBe('number');
  });
});
