import { describe, it, expect } from 'vitest';
import { tierFromScore, TIER_LABELS, TIER_COLORS } from './tier';

describe('tierFromScore', () => {
  it('maps scores to tiers at the documented thresholds', () => {
    expect(tierFromScore(0.90)).toBe('s');
    expect(tierFromScore(0.75)).toBe('s'); // boundary inclusive
    expect(tierFromScore(0.7499)).toBe('a');
    expect(tierFromScore(0.65)).toBe('a'); // boundary inclusive
    expect(tierFromScore(0.6499)).toBe('b');
    expect(tierFromScore(0.55)).toBe('b'); // boundary inclusive
    expect(tierFromScore(0.5499)).toBe('c');
    expect(tierFromScore(0)).toBe('c');
  });

  it('every tier has a label and a color', () => {
    for (const t of ['s', 'a', 'b', 'c'] as const) {
      expect(TIER_LABELS[t]).toBeTruthy();
      expect(TIER_COLORS[t]).toMatch(/^#[0-9A-Fa-f]{6}$/);
    }
  });
});
