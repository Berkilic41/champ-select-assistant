import { describe, it, expect } from 'vitest';
import type { CoverageExpansionPlan } from './generated/CoverageExpansionPlan';
import type { CoverageTarget } from './generated/CoverageTarget';
import type { CoverageFrontier } from './generated/CoverageFrontier';
import type { ExpansionRisk } from './generated/ExpansionRisk';

// Compile-time contract guard for the coverage expansion policy output (Sprint). A
// Rust field add/remove/retype regenerates these and breaks `pnpm typecheck` here
// before Codex's expansion binding can drift. No bigint (samples/priority number;
// champion_id is number | null).

const frontier: CoverageFrontier = {
  region: 'euw1',
  patch: '14.11',
  role: 'middle',
  champion_id: null,
  current_samples: 100,
  target_samples: 1000,
  deficit: 900,
  coverage_ratio: 0.1,
};

const target: CoverageTarget = {
  frontier,
  priority: 4,
  needed_samples: 900,
  rationale: 'r',
};

const risk: ExpansionRisk = { level: 'high', factors: ['thin_data'], summary: 's' };

describe('Coverage expansion contract', () => {
  it('CoverageExpansionPlan bundles targets/risk/counts', () => {
    const keys: Record<keyof CoverageExpansionPlan, true> = {
      data_state: true,
      targets: true,
      risk: true,
      total_deficit: true,
      frontier_count: true,
    };
    expect(Object.keys(keys).sort()).toEqual([
      'data_state',
      'frontier_count',
      'risk',
      'targets',
      'total_deficit',
    ]);

    const plan: CoverageExpansionPlan = {
      data_state: 'thin',
      targets: [target],
      risk,
      total_deficit: 900,
      frontier_count: 1,
    };
    expect(plan.targets[0].frontier.role).toBe('middle');
    expect(plan.targets[0].frontier.champion_id).toBeNull();
    expect(typeof plan.total_deficit).toBe('number');
    expect(typeof plan.targets[0].frontier.coverage_ratio).toBe('number');
  });

  it('nested shapes are exact', () => {
    const f: Record<keyof CoverageFrontier, true> = {
      region: true, patch: true, role: true, champion_id: true, current_samples: true,
      target_samples: true, deficit: true, coverage_ratio: true,
    };
    const t: Record<keyof CoverageTarget, true> = {
      frontier: true, priority: true, needed_samples: true, rationale: true,
    };
    const r: Record<keyof ExpansionRisk, true> = { level: true, factors: true, summary: true };
    expect(Object.keys(f).length).toBe(8);
    expect(Object.keys(t).length).toBe(4);
    expect(Object.keys(r).length).toBe(3);
    // Risk factor / level / data_state token vocabulary (Rust-locked too).
    const levels: Array<ExpansionRisk['level']> = ['low', 'medium', 'high'];
    expect(levels).toContain(risk.level);
  });
});
