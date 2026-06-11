import { describe, it, expect } from 'vitest';
import type { PipelineQualityReport } from './generated/PipelineQualityReport';
import type { PipelineCoverage } from './generated/PipelineCoverage';
import type { DataGap } from './generated/DataGap';
import type { PipelineAction } from './generated/PipelineAction';
import type { SourceFreshness } from './generated/SourceFreshness';
import type { DataPipelineRefreshSummary } from './generated/DataPipelineRefreshSummary';

// Compile-time contract guard for the Data Pipeline Quality core (Sprint). A Rust
// field add/remove/retype regenerates these types and breaks `pnpm typecheck` here
// before any data-quality panel binding can drift. No bigint anywhere.

const source: SourceFreshness = {
  source: 'riot_match_v5',
  age_hours: 2,
  stale: false,
  risk_level: 'low',
};

const coverage: PipelineCoverage = {
  champion_rate_coverage: 0.98,
  build_coverage: 0.97,
  matchup_coverage: 1,
  role_coverage: 0.93,
  patch_fresh: true,
  sources_fresh: true,
  fallback_available: true,
  last_good_cache_available: true,
  has_high_risk_source: false,
  sources: [source],
};

const gap: DataGap = { dimension: 'matchup_coverage_low', severity: 'medium', note: 'n' };
const action: PipelineAction = { action: 'refresh_matchups', reason: 'r' };

describe('PipelineQualityReport contract', () => {
  it('has exactly the expected top-level fields', () => {
    const keys: Record<keyof PipelineQualityReport, true> = {
      status: true,
      confidence: true,
      coverage: true,
      gaps: true,
      actions: true,
      summary: true,
    };
    expect(Object.keys(keys).sort()).toEqual([
      'actions',
      'confidence',
      'coverage',
      'gaps',
      'status',
      'summary',
    ]);
  });

  it('assembles a full report', () => {
    const report: PipelineQualityReport = {
      status: 'degraded',
      confidence: 'medium',
      coverage,
      gaps: [gap],
      actions: [action],
      summary: 's',
    };
    expect(report.coverage.sources[0].source).toBe('riot_match_v5');
    expect(report.gaps[0].dimension).toBe('matchup_coverage_low');
    expect(report.actions[0].action).toBe('refresh_matchups');
    // Coverage fractions are number (not bigint).
    expect(typeof report.coverage.matchup_coverage).toBe('number');
    expect(typeof report.coverage.sources[0].age_hours).toBe('number');
  });

  it('PipelineCoverage / DataGap / PipelineAction / SourceFreshness shapes are exact', () => {
    const cov: Record<keyof PipelineCoverage, true> = {
      champion_rate_coverage: true,
      build_coverage: true,
      matchup_coverage: true,
      role_coverage: true,
      patch_fresh: true,
      sources_fresh: true,
      fallback_available: true,
      last_good_cache_available: true,
      has_high_risk_source: true,
      sources: true,
    };
    const dg: Record<keyof DataGap, true> = { dimension: true, severity: true, note: true };
    const pa: Record<keyof PipelineAction, true> = { action: true, reason: true };
    const sf: Record<keyof SourceFreshness, true> = {
      source: true,
      age_hours: true,
      stale: true,
      risk_level: true,
    };
    expect(Object.keys(cov).length).toBe(10);
    expect(Object.keys(dg).length).toBe(3);
    expect(Object.keys(pa).length).toBe(2);
    expect(Object.keys(sf).length).toBe(4);
  });

  it('DataPipelineRefreshSummary shape is exact and number-only for counts', () => {
    const summary: DataPipelineRefreshSummary = {
      before_status: 'degraded',
      after_status: 'healthy',
      actions: ['refresh_rates'],
      ddragon_champions: 172,
      meraki_rates: 860,
      builds_imported: 31,
      matchups_imported: 80,
      match_v5_matches: 20,
      match_v5_rates: 100,
      match_v5_matchups: 100,
      match_v5_builds: 100,
      match_v5_errors: 0,
      data_pack_cached: true,
      cache_action: 'promote',
      cache_promoted: true,
      errors: [],
    };
    const keys: Record<keyof DataPipelineRefreshSummary, true> = {
      before_status: true,
      after_status: true,
      actions: true,
      ddragon_champions: true,
      meraki_rates: true,
      builds_imported: true,
      matchups_imported: true,
      match_v5_matches: true,
      match_v5_rates: true,
      match_v5_matchups: true,
      match_v5_builds: true,
      match_v5_errors: true,
      data_pack_cached: true,
      cache_action: true,
      cache_promoted: true,
      errors: true,
    };
    expect(Object.keys(keys).length).toBe(16);
    expect(typeof summary.ddragon_champions).toBe('number');
    expect(typeof summary.match_v5_matches).toBe('number');
    expect(typeof summary.data_pack_cached).toBe('boolean');
    expect(typeof summary.cache_promoted).toBe('boolean');
  });
});
