import { describe, it, expect } from 'vitest';
import type { AggregationResult } from './generated/AggregationResult';
import type { AggregatedChampionRate } from './generated/AggregatedChampionRate';
import type { AggregatedMatchup } from './generated/AggregatedMatchup';
import type { AggregatedBuild } from './generated/AggregatedBuild';
import type { AggregationQuality } from './generated/AggregationQuality';

// Compile-time contract guard for the Match-V5 aggregation output (Sprint). A Rust
// field add/remove/retype regenerates these and breaks `pnpm typecheck` here before
// Codex's DB-upsert mapping can drift. All counts/rates are number (no bigint).

const rate: AggregatedChampionRate = {
  champion_id: 1,
  position: 'top',
  games: 2,
  wins: 1,
  win_rate: 0.5,
  pick_rate: 0.1,
  ban_rate: 0.0,
  sample_size: 2,
  patch: '14.11',
  source: 'riot_match_v5',
  confidence: 'low',
};

const matchup: AggregatedMatchup = {
  champion_id: 3,
  opponent_id: 13,
  position: 'middle',
  games: 1,
  wins: 1,
  win_rate: 1,
  patch: '14.11',
  source: 'riot_match_v5',
  confidence: 'low',
};

const build: AggregatedBuild = {
  champion_id: 1,
  position: 'top',
  patch: '14.11',
  core_items: [1001, 3006],
  situational_items: [],
  rune_ids: [8005],
  summoner_spells: [4, 12],
  games: 3,
  win_rate: 1,
  source: 'riot_match_v5',
  confidence: 'low',
};

const quality: AggregationQuality = {
  match_count: 2,
  champion_rate_count: 10,
  matchup_count: 8,
  build_count: 10,
  skipped_matches: 0,
  warnings: [],
};

describe('Match-V5 aggregation contract', () => {
  it('AggregationResult bundles rates/matchups/builds/quality', () => {
    const keys: Record<keyof AggregationResult, true> = {
      rates: true,
      matchups: true,
      builds: true,
      quality: true,
    };
    expect(Object.keys(keys).sort()).toEqual(['builds', 'matchups', 'quality', 'rates']);
    const result: AggregationResult = {
      rates: [rate],
      matchups: [matchup],
      builds: [build],
      quality,
    };
    expect(result.rates[0].source).toBe('riot_match_v5');
    expect(typeof result.rates[0].win_rate).toBe('number');
    expect(typeof result.quality.match_count).toBe('number');
    expect(Array.isArray(result.builds[0].core_items)).toBe(true);
  });

  it('aggregate row shapes are exact', () => {
    const r: Record<keyof AggregatedChampionRate, true> = {
      champion_id: true, position: true, games: true, wins: true, win_rate: true,
      pick_rate: true, ban_rate: true, sample_size: true, patch: true, source: true, confidence: true,
    };
    const b: Record<keyof AggregatedBuild, true> = {
      champion_id: true, position: true, patch: true, core_items: true, situational_items: true,
      rune_ids: true, summoner_spells: true, games: true, win_rate: true, source: true, confidence: true,
    };
    const q: Record<keyof AggregationQuality, true> = {
      match_count: true, champion_rate_count: true, matchup_count: true,
      build_count: true, skipped_matches: true, warnings: true,
    };
    expect(Object.keys(r).length).toBe(11);
    expect(Object.keys(b).length).toBe(11);
    expect(Object.keys(q).length).toBe(6);
  });
});
