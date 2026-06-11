import { describe, it, expect } from 'vitest';
import type { DraftFork } from './generated/DraftFork';
import type { DraftSimResult } from './generated/DraftSimResult';
import type { DraftSimRisk } from './generated/DraftSimRisk';
import type { DraftSimPlanShift } from './generated/DraftSimPlanShift';

// Compile-time contract guard for the Draft Fork response (Sprint H). The
// DraftForkPanel binds this shape; a Rust field add/remove/retype regenerates the
// type and breaks `pnpm typecheck` here before the panel drifts.

const risk: DraftSimRisk = { level: 'medium', summary: 's', factors: ['execution_risk'] };
const plan: DraftSimPlanShift = { before: 'dengeli', after: 'engage/dalış', note: 'n' };

function simResult(id: number, key: string): DraftSimResult {
  return {
    champion_id: id,
    champion_key: key,
    score_delta: 0.1,
    improved_factors: ['engage'],
    worsened_factors: [],
    deltas: [{ factor: 'engage', before: 0.1, after: 0.5, delta: 0.4 }],
    risk,
    plan_shift: plan,
    coach_sentence: 'c',
    why_this_move: 'w',
    why_not_alternative: 'a',
  };
}

describe('DraftFork contract', () => {
  it('has the expected top-level fork fields', () => {
    const keys: Record<keyof DraftFork, true> = {
      option_a: true,
      option_b: true,
      plan_divergence: true,
      risk_divergence: true,
      shared_factors: true,
      diverging_factors: true,
      recommendation: true,
    };
    expect(Object.keys(keys).sort()).toEqual([
      'diverging_factors',
      'option_a',
      'option_b',
      'plan_divergence',
      'recommendation',
      'risk_divergence',
      'shared_factors',
    ]);
  });

  it('embeds two DraftSimResult options and string verdicts', () => {
    const fork: DraftFork = {
      option_a: simResult(238, 'Zed'),
      option_b: simResult(99, 'Lux'),
      plan_divergence: 'Zed → engage/dalış; Lux → poke/obje.',
      risk_divergence: 'Risk seviyesi benzer (low).',
      shared_factors: ['damage_balance'],
      diverging_factors: ['engage'],
      recommendation: 'İki seçenek yakın; tercih konfor ve role göre.',
    };
    expect(fork.option_a.champion_id).toBe(238);
    expect(Array.isArray(fork.shared_factors)).toBe(true);
    expect(typeof fork.recommendation).toBe('string');
    // No bigint anywhere — score_delta + deltas are number.
    expect(typeof fork.option_a.score_delta).toBe('number');
    expect(typeof fork.option_b.deltas[0].delta).toBe('number');
  });
});

describe('DraftSimResult contract', () => {
  it('has exactly the simulator result fields', () => {
    const keys: Record<keyof DraftSimResult, true> = {
      champion_id: true,
      champion_key: true,
      score_delta: true,
      improved_factors: true,
      worsened_factors: true,
      deltas: true,
      risk: true,
      plan_shift: true,
      coach_sentence: true,
      why_this_move: true,
      why_not_alternative: true,
    };
    expect(Object.keys(keys).length).toBe(11);
  });
});
