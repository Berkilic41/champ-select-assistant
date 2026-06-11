import { describe, it, expect } from 'vitest';
import type { DataSourceRegistryReport } from './generated/DataSourceRegistryReport';
import type { DataSourceEntry } from './generated/DataSourceEntry';
import type { ScoutingReport } from './generated/ScoutingReport';
import type { EnemyProfile } from './generated/EnemyProfile';
import type { ScoutBanTarget } from './generated/ScoutBanTarget';
import type { DataPack } from './generated/DataPack';
import type { DataPackQuality } from './generated/DataPackQuality';
import type { DraftBrainQualityReport } from './generated/DraftBrainQualityReport';
import type { PackSyncStatus } from './generated/PackSyncStatus';

// Compile-time contract for the generated Data Supremacy, scouting, and
// DraftBrain pack types. These typed literals must satisfy the generated shapes,
// so if a Rust struct field is added/removed/renamed/retyped, `pnpm typecheck`
// fails here and catches frontend/backend contract drift.
//
// ts-rs maps Rust `i64` → TS `bigint` and `u32` → `number`. After the parity
// audit, ALL `generated_at` fields are `u32` → `number` (registry unified with the
// pack family). The remaining `bigint` fields are `i64`: `DataSourceEntry.updated_at`
// (matches `DataSourceBadge.updated_at`) and `DraftBrainQualityReport.feedback_*`.

const entry: DataSourceEntry = {
  source: 'riot_match_v5',
  patch: '16.11',
  region: 'tr',
  sample_size: 1200,
  confidence: 'high',
  risk_level: 'medium',
  updated_at: BigInt(0),
  fallback_used: false,
};

const registry: DataSourceRegistryReport = {
  champion_rates_count: 0,
  matchup_count: 0,
  build_count: 0,
  primary_role_build_coverage: 0,
  meta_role_coverage: 0,
  exact_matchup_coverage: 0,
  stale_sources: [],
  high_risk_sources: [],
  fallback_active: false,
  confidence: 'low',
  sources: [entry],
  generated_at: 0, // u32 → TS number (unified with the pack family; see audit doc)
};

const banTarget: ScoutBanTarget = {
  champion_id: 238,
  champion_key: 'Zed',
  reason: 'Rakibin ana kozu',
  confidence: 'high',
};

const profile: EnemyProfile = {
  cell_id: 5,
  top_champion_id: 238,
  top_champion_key: 'Zed',
  play_rate: 0.7,
  game_count: 12,
  pool_tendency: 'otp',
  playstyle_tags: ['erken agresif'],
  threat: 'yuksek',
  threat_level: 'high',
  note: 'Zed OTP',
};

const scouting: ScoutingReport = {
  enemies: [profile],
  ban_targets: [banTarget],
  team_read: {
    win_condition: 'pick',
    high_threat_count: 1,
    primary_threat_key: 'Zed',
    primary_threat_cell_id: 5,
    strategy_note: 'Pick komp: vision basın.',
  },
  partial: false,
};

const dataPackQuality: DataPackQuality = {
  champion_rates: 172,
  matchups: 1200,
  builds: 172,
  feedback: 25,
  draft_samples: 40,
};

const dataPack: DataPack = {
  version: 'data-pack-16.10-tr1',
  patch: '16.10',
  region: 'tr1',
  sources: ['cloud_postgres', 'riot_match_v5'],
  quality: dataPackQuality,
  fallback: false,
  confidence: 'high',
  generated_at: 1_800_000_000,
};

const packSyncStatus: PackSyncStatus = {
  kind: 'data_pack',
  version: dataPack.version,
  source: 'cloud',
  cached: true,
  online: true,
  confidence: 'high',
  generated_at: 1_800_000_000,
  message: 'Data pack cloud kaynaktan cache edildi',
};

const draftBrainQuality: DraftBrainQualityReport = {
  feedback_total: BigInt(12),
  feedback_unsynced: BigInt(2),
  model_pack_version: 'draft-brain-rules-v2',
  data_pack_version: dataPack.version,
  data_pack_confidence: 'high',
  data_pack_generated_at: 1_800_000_000,
  data_pack_fresh: true,
  local_rules_version: 'draft-brain-rules-v2',
  cloud_configured: true,
  notes: [],
};

describe('data-supremacy generated TS contract', () => {
  it('DataSourceRegistryReport shape matches the Rust struct', () => {
    expect(registry.sources[0].source).toBe('riot_match_v5');
    expect(registry.confidence).toBe('low');
    // generated_at unified to u32 → number (was bigint); see audit doc.
    expect(typeof registry.generated_at).toBe('number');
    // updated_at stays i64 → bigint (matches DataSourceBadge.updated_at).
    expect(typeof entry.updated_at).toBe('bigint');
  });

  it('ScoutingReport shape matches the Rust struct', () => {
    expect(scouting.enemies[0].pool_tendency).toBe('otp');
    // Locale-free threat token mirrors the TR `threat` field (3b bridge).
    expect(scouting.enemies[0].threat_level).toBe('high');
    expect(scouting.ban_targets[0].champion_id).toBe(238);
    expect(scouting.partial).toBe(false);
    // Team-level read (scouting depth v2).
    expect(scouting.team_read.win_condition).toBe('pick');
    expect(scouting.team_read.primary_threat_key).toBe('Zed');
    expect(typeof scouting.team_read.high_threat_count).toBe('number');
  });

  it('DraftBrain pack and quality response shapes match the Rust structs', () => {
    expect(dataPack.quality.champion_rates).toBe(172);
    expect(typeof dataPack.generated_at).toBe('number');
    expect(packSyncStatus.confidence).toBe('high');
    expect(typeof draftBrainQuality.feedback_total).toBe('bigint');
    expect(draftBrainQuality.data_pack_fresh).toBe(true);
  });
});

// Feedback Loop v1 payload/vocabulary contract moved to its own source-of-truth:
// `src/types/feedback.ts` (+ `feedback.test.ts`, `feedback-vocabulary.json`).
