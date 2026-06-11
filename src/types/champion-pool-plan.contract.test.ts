import { describe, it, expect } from 'vitest';
import type { ChampionPoolPlan } from './generated/ChampionPoolPlan';
import type { PoolCoverage } from './generated/PoolCoverage';
import type { PoolGap } from './generated/PoolGap';
import type { TrainingRecommendation } from './generated/TrainingRecommendation';

// Compile-time contract guard for the Champion Pool Coach response (Sprint I). A
// Rust field add/remove/retype regenerates these types and breaks `pnpm typecheck`
// here before any pool-coach panel binding can drift. No bigint anywhere.

const coverage: PoolCoverage = {
  role_covered: true,
  has_blind_safe: true,
  has_counter_flex: false,
  has_comfort: true,
  identity_variety: false,
  execution_risk: 'medium',
};

const gap: PoolGap = { dimension: 'blind_safe', severity: 'high', note: 'n' };

const training: TrainingRecommendation = {
  champion_id: 54,
  champion_key: 'Malphite',
  role_in_plan: 'blind_safe',
  reason: 'r',
  confidence: 'high',
  drills: ['Skillshot pratiği'],
};

describe('ChampionPoolPlan contract', () => {
  it('has exactly the expected plan fields', () => {
    const keys: Record<keyof ChampionPoolPlan, true> = {
      role: true,
      data_state: true,
      pool_size: true,
      coverage: true,
      gaps: true,
      training: true,
      summary: true,
    };
    expect(Object.keys(keys).sort()).toEqual([
      'coverage',
      'data_state',
      'gaps',
      'pool_size',
      'role',
      'summary',
      'training',
    ]);
  });

  it('assembles a full plan with nested coverage/gaps/training', () => {
    const plan: ChampionPoolPlan = {
      role: 'middle',
      data_state: 'rich',
      pool_size: 3,
      coverage,
      gaps: [gap],
      training: [training],
      summary: 's',
    };
    expect(typeof plan.pool_size).toBe('number');
    expect(plan.coverage.execution_risk).toBe('medium');
    expect(plan.gaps[0].dimension).toBe('blind_safe');
    expect(plan.training[0].role_in_plan).toBe('blind_safe');
  });

  it('coverage flags are booleans and execution_risk is a string', () => {
    const keys: Record<keyof PoolCoverage, true> = {
      role_covered: true,
      has_blind_safe: true,
      has_counter_flex: true,
      has_comfort: true,
      identity_variety: true,
      execution_risk: true,
    };
    expect(Object.keys(keys).length).toBe(6);
    expect(typeof coverage.role_covered).toBe('boolean');
    expect(typeof coverage.execution_risk).toBe('string');
  });

  it('PoolGap has exactly dimension/severity/note', () => {
    const keys: Record<keyof PoolGap, true> = {
      dimension: true,
      severity: true,
      note: true,
    };
    expect(Object.keys(keys).sort()).toEqual(['dimension', 'note', 'severity']);
    expect(typeof gap.dimension).toBe('string');
  });

  it('TrainingRecommendation has exactly the plan-slot fields', () => {
    const keys: Record<keyof TrainingRecommendation, true> = {
      champion_id: true,
      champion_key: true,
      role_in_plan: true,
      reason: true,
      confidence: true,
      drills: true,
    };
    expect(Object.keys(keys).sort()).toEqual([
      'champion_id',
      'champion_key',
      'confidence',
      'drills',
      'reason',
      'role_in_plan',
    ]);
    expect(typeof training.champion_id).toBe('number');
  });
});
