import { describe, it, expect } from 'vitest';
import type { CanonicalRowSet } from './generated/CanonicalRowSet';
import type { CanonicalRateRow } from './generated/CanonicalRateRow';
import type { CanonicalMatchupRow } from './generated/CanonicalMatchupRow';
import type { CanonicalBuildRow } from './generated/CanonicalBuildRow';
import type { CachePromotionDecision } from './generated/CachePromotionDecision';

// Compile-time contract guard for the ingestion canonical rows + last-good cache
// decision (Sprint). A Rust field add/remove/retype regenerates these and breaks
// `pnpm typecheck` here before Codex's DB-upsert / cache mapping can drift. The
// promotion `action` token vocabulary is locked too. No bigint anywhere.

const rate: CanonicalRateRow = {
  region: 'euw1',
  patch: '14.11',
  champion_id: 1,
  position: 'top',
  win_rate: 0.5,
  pick_rate: 0.1,
  ban_rate: 0,
  sample_size: 2,
  source: 'riot_match_v5',
  confidence: 'low',
};

const matchup: CanonicalMatchupRow = {
  region: 'euw1',
  patch: '14.11',
  champion_id: 3,
  opponent_id: 13,
  position: 'middle',
  games: 1,
  wins: 1,
  win_rate: 1,
  sample_size: 1,
  source: 'riot_match_v5',
  confidence: 'low',
};

const build: CanonicalBuildRow = {
  region: 'euw1',
  patch: '14.11',
  champion_id: 1,
  position: 'top',
  item_ids: [1001, 3006],
  rune_ids: [8005],
  summoner_spells: [4, 12],
  games: 3,
  win_rate: 1,
  pick_rate: 0.1,
  sample_size: 3,
  source: 'riot_match_v5',
  confidence: 'low',
};

describe('Ingestion canonical contract', () => {
  it('CanonicalRowSet bundles region + rates/matchups/builds', () => {
    const keys: Record<keyof CanonicalRowSet, true> = {
      region: true,
      rates: true,
      matchups: true,
      builds: true,
    };
    expect(Object.keys(keys).sort()).toEqual(['builds', 'matchups', 'rates', 'region']);
    const set: CanonicalRowSet = { region: 'euw1', rates: [rate], matchups: [matchup], builds: [build] };
    expect(set.rates[0].source).toBe('riot_match_v5');
    expect(typeof set.rates[0].ban_rate).toBe('number');
    expect(Array.isArray(set.builds[0].item_ids)).toBe(true);
  });

  it('canonical row shapes are exact', () => {
    const r: Record<keyof CanonicalRateRow, true> = {
      region: true, patch: true, champion_id: true, position: true, win_rate: true,
      pick_rate: true, ban_rate: true, sample_size: true, source: true, confidence: true,
    };
    const m: Record<keyof CanonicalMatchupRow, true> = {
      region: true, patch: true, champion_id: true, opponent_id: true, position: true,
      games: true, wins: true, win_rate: true, sample_size: true, source: true, confidence: true,
    };
    const b: Record<keyof CanonicalBuildRow, true> = {
      region: true, patch: true, champion_id: true, position: true, item_ids: true,
      rune_ids: true, summoner_spells: true, games: true, win_rate: true, pick_rate: true,
      sample_size: true, source: true, confidence: true,
    };
    expect(Object.keys(r).length).toBe(10);
    expect(Object.keys(m).length).toBe(11);
    expect(Object.keys(b).length).toBe(13);
  });

  it('CachePromotionDecision shape + action token vocabulary', () => {
    const keys: Record<keyof CachePromotionDecision, true> = {
      action: true,
      promoted: true,
      reason: true,
    };
    expect(Object.keys(keys).sort()).toEqual(['action', 'promoted', 'reason']);
    // The only actions the engine emits.
    const actions: Array<CachePromotionDecision['action']> = ['promote', 'keep_current', 'reject'];
    const sample: CachePromotionDecision = { action: 'promote', promoted: true, reason: 'r' };
    expect(actions).toContain(sample.action);
    expect(typeof sample.promoted).toBe('boolean');
  });
});
