import { describe, it, expect } from 'vitest';
import type { CoverageRampReport } from './generated/CoverageRampReport';
import type { CoverageDeltas } from './generated/CoverageDeltas';
import type { DiscoveryFunnel } from './generated/DiscoveryFunnel';
import type { CoverageRampSnapshotView } from './generated/CoverageRampSnapshotView';
import type { DataPipelineRefreshSummary } from './generated/DataPipelineRefreshSummary';
import type { LiveCoverageRampReport } from './generated/LiveCoverageRampReport';
import type { DataTrajectoryView } from './generated/DataTrajectoryView';

// Compile-time contract guard for the Live Data Coverage Ramp output (Sprint). A
// Rust field add/remove/retype regenerates these and breaks `pnpm typecheck` here
// before Codex's measurement command can drift. No bigint — deltas are i32 → number,
// counts u32 → number, ratios f32 → number (the report is bigint-free by design).

const deltas: CoverageDeltas = {
  champion_rate_delta: 20,
  matchup_delta: 40,
  build_delta: 5,
  discovered_delta: -25,
  processed_delta: 20,
  crawled_player_delta: 4,
  elapsed_secs: 60,
};

const funnel: DiscoveryFunnel = {
  pending: 4,
  fetched: 0,
  processed: 20,
  failed: 0,
  process_ratio: 0.83,
  failure_ratio: 0,
  stalled_stage: null,
};

const snapshot: CoverageRampSnapshotView = {
  taken_at: 1_779_000_000,
  champion_rate_rows: 120,
  matchup_rows: 300,
  build_rows: 80,
  discovered_matches: 12,
  fetched_matches: 2,
  processed_matches: 10,
  failed_matches: 1,
  crawled_players: 5,
};

const refresh: DataPipelineRefreshSummary = {
  before_status: 'degraded',
  after_status: 'degraded',
  actions: ['refresh_matchups'],
  ddragon_champions: 172,
  meraki_rates: 172,
  builds_imported: 31,
  matchups_imported: 80,
  match_v5_matches: 4,
  match_v5_rates: 3,
  match_v5_matchups: 8,
  match_v5_builds: 2,
  match_v5_errors: 0,
  data_pack_cached: true,
  cache_action: 'promote',
  cache_promoted: true,
  errors: [],
};

describe('Coverage ramp contract', () => {
  it('CoverageRampReport bundles state/deltas/funnel/observations', () => {
    const keys: Record<keyof CoverageRampReport, true> = {
      ramp_state: true,
      deltas: true,
      funnel: true,
      observations: true,
      summary: true,
      data_growing: true,
    };
    expect(Object.keys(keys).sort()).toEqual([
      'data_growing',
      'deltas',
      'funnel',
      'observations',
      'ramp_state',
      'summary',
    ]);

    const report: CoverageRampReport = {
      ramp_state: 'progressing',
      deltas,
      funnel,
      observations: ['coverage_growing', 'processing_advancing'],
      summary: 's',
      data_growing: true,
    };
    expect(report.deltas.processed_delta).toBe(20);
    expect(report.funnel.stalled_stage).toBeNull();
    expect(typeof report.deltas.discovered_delta).toBe('number');
    expect(typeof report.funnel.process_ratio).toBe('number');
    expect(typeof report.data_growing).toBe('boolean');
  });

  it('nested shapes are exact', () => {
    const d: Record<keyof CoverageDeltas, true> = {
      champion_rate_delta: true, matchup_delta: true, build_delta: true,
      discovered_delta: true, processed_delta: true, crawled_player_delta: true,
      elapsed_secs: true,
    };
    const f: Record<keyof DiscoveryFunnel, true> = {
      pending: true, fetched: true, processed: true, failed: true,
      process_ratio: true, failure_ratio: true, stalled_stage: true,
    };
    expect(Object.keys(d).length).toBe(7);
    expect(Object.keys(f).length).toBe(7);
  });

  it('ramp_state / funnel stage / observation vocabularies (Rust-locked)', () => {
    const states: Array<CoverageRampReport['ramp_state']> = [
      'progressing', 'stalled', 'regressed', 'no_activity', 'no_budget',
    ];
    const stages: Array<NonNullable<DiscoveryFunnel['stalled_stage']>> = [
      'discovery', 'fetch', 'process',
    ];
    const observations = [
      'coverage_growing', 'coverage_flat', 'coverage_regressed',
      'new_matches_discovered', 'no_new_matches', 'fetch_backlog_growing',
      'processing_advancing', 'processing_below_expected',
      'high_failure_rate', 'champ_select_deferred',
    ];
    expect(states).toContain('progressing');
    expect(stages).toContain('fetch');
    expect(observations).toHaveLength(10);
  });

  it('LiveCoverageRampReport wraps before/after snapshots, refresh summary, and ramp verdict', () => {
    const snapshotKeys: Record<keyof CoverageRampSnapshotView, true> = {
      taken_at: true,
      champion_rate_rows: true,
      matchup_rows: true,
      build_rows: true,
      discovered_matches: true,
      fetched_matches: true,
      processed_matches: true,
      failed_matches: true,
      crawled_players: true,
    };
    const reportKeys: Record<keyof LiveCoverageRampReport, true> = {
      before: true,
      after: true,
      ramp: true,
      refresh: true,
      champ_select_active: true,
      crawl_budget: true,
    };
    const report: LiveCoverageRampReport = {
      before: snapshot,
      after: { ...snapshot, processed_matches: 12 },
      ramp: {
        ramp_state: 'progressing',
        deltas,
        funnel,
        observations: ['processing_advancing'],
        summary: 's',
        data_growing: true,
      },
      refresh,
      champ_select_active: false,
      crawl_budget: 3,
    };

    expect(Object.keys(snapshotKeys).length).toBe(9);
    expect(Object.keys(reportKeys).sort()).toEqual([
      'after',
      'before',
      'champ_select_active',
      'crawl_budget',
      'ramp',
      'refresh',
    ]);
    expect(typeof report.before.taken_at).toBe('number');
    expect(typeof report.crawl_budget).toBe('number');
    expect(report.refresh.match_v5_matches).toBe(4);
  });

  it('DataTrajectoryView fuses quality + ramp into one token (no bigint)', () => {
    const keys: Record<keyof DataTrajectoryView, true> = {
      trajectory: true,
      quality_status: true,
      ramp_state: true,
      data_growing: true,
      measured_at: true,
      riot_key_present: true,
      match_v5_enabled: true,
      match_v5_last_success_at: true,
      match_v5_age_secs: true,
    };
    expect(Object.keys(keys).sort()).toEqual([
      'data_growing',
      'match_v5_age_secs',
      'match_v5_enabled',
      'match_v5_last_success_at',
      'measured_at',
      'quality_status',
      'ramp_state',
      'riot_key_present',
      'trajectory',
    ]);

    const view: DataTrajectoryView = {
      trajectory: 'enriching',
      quality_status: 'healthy',
      ramp_state: 'progressing',
      data_growing: true,
      measured_at: 1_779_000_000,
      riot_key_present: true,
      match_v5_enabled: true,
      match_v5_last_success_at: 1_779_000_000,
      match_v5_age_secs: 3600,
    };
    expect(typeof view.measured_at).toBe('number'); // u32 → number, never bigint
    const unmeasured: DataTrajectoryView = {
      trajectory: 'unknown',
      quality_status: 'degraded',
      ramp_state: 'unknown',
      data_growing: false,
      measured_at: null,
      riot_key_present: false,
      match_v5_enabled: false,
      match_v5_last_success_at: null,
      match_v5_age_secs: null,
    };
    expect(unmeasured.measured_at).toBeNull();

    // Trajectory vocabulary (Rust-locked DATA_TRAJECTORIES + the `unknown` fallback).
    const trajectories = [
      'healthy', 'enriching', 'warming_up', 'stagnant', 'regressing', 'deferred', 'unknown',
    ];
    expect(trajectories).toContain(view.trajectory);
    expect(trajectories).toContain(unmeasured.trajectory);
  });
});
