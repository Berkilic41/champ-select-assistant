import { describe, it, expect } from 'vitest';
import { assessPickClarity } from './pickClarity';

const r = (key: string, score: number) => ({ champion_key: key, total_score: score });

describe('assessPickClarity', () => {
  it('returns null for an empty list', () => {
    expect(assessPickClarity([])).toBeNull();
  });

  it('a single recommendation is clear with no runner-up', () => {
    expect(assessPickClarity([r('Zed', 0.7)])).toEqual({
      level: 'clear',
      gap: 1,
      runnerUpKey: null,
    });
  });

  it('a big gap to #2 is a clear pick', () => {
    const c = assessPickClarity([r('Zed', 0.82), r('Talon', 0.70)]);
    expect(c?.level).toBe('clear');
    expect(c?.runnerUpKey).toBe('Talon');
  });

  it('a moderate gap is an edge', () => {
    expect(assessPickClarity([r('Zed', 0.74), r('Talon', 0.71)])?.level).toBe('edge');
  });

  it('a tiny gap is a close call (near tie)', () => {
    const c = assessPickClarity([r('Zed', 0.715), r('Talon', 0.71)]);
    expect(c?.level).toBe('close');
    expect(c?.runnerUpKey).toBe('Talon');
  });
});
