import { describe, it, expect } from 'vitest';
import tr from './tr.json';
import en from './en.json';

// Guardrail (Claude) for the DraftBrain/Coach i18n migration. Goes red if a future
// edit drifts tr/en apart, drops a coach label, or breaks {{placeholder}} parity.
// See docs/audit/i18n-parity.md. Hot UI components are NOT touched here.

type Json = Record<string, unknown>;

function flatten(obj: Json, prefix = ''): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(obj)) {
    const key = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === 'object' && !Array.isArray(v)) {
      Object.assign(out, flatten(v as Json, key));
    } else {
      out[key] = String(v);
    }
  }
  return out;
}

function placeholders(value: string): string[] {
  return (value.match(/\{\{\s*[^}]+?\s*\}\}/g) ?? [])
    .map((p) => p.replace(/\s+/g, ''))
    .sort();
}

const trFlat = flatten(tr as Json);
const enFlat = flatten(en as Json);
const trKeys = Object.keys(trFlat);
const enKeys = Object.keys(enFlat);

// Keys the migration introduced — must never silently disappear.
const REQUIRED_KEYS = [
  'performance.eyebrow',
  'performance.title',
  'performance.thinData',
  'performance.tiltRisk',
  'performance.games',
  'performance.form',
  'performance.champLine',
  'performance.unknownRole',
  'feedbackAnalytics.eyebrow',
  'feedbackAnalytics.title',
  'feedbackAnalytics.window',
  'feedbackAnalytics.recentSignals',
  'feedbackAnalytics.totalEvents',
  'feedbackAnalytics.trackedChampions',
  'feedbackAnalytics.dislikedTitle',
  'feedbackAnalytics.dislikedLine',
  'feedbackAnalytics.noDisliked',
  'draftSimulator.eyebrow',
  'draftSimulator.title',
  'draftSimulator.localOnly',
  'draftSimulator.scoreDelta',
  'draftSimulator.risk',
  'draftSimulator.whyNot',
  'draftSimulator.riskLevel.low',
  'draftSimulator.riskLevel.medium',
  'draftSimulator.riskLevel.high',
  'draftSimulator.factor.damage_balance',
  'draftSimulator.factor.engage',
  'draftSimulator.factor.disengage',
  'draftSimulator.factor.frontline',
  'draftSimulator.factor.peel',
  'draftSimulator.factor.scaling',
  'draftSimulator.factor.lane_pressure',
  'draftSimulator.factor.objective_identity',
  'draftSimulator.factor.execution_risk',
  'draftSimulator.factor.blind_safety',
  'draftSimulator.factor.synergy',
  'draftFork.eyebrow',
  'draftFork.title',
  'draftFork.optionLine',
  'draftFork.shared',
  'draftFork.diverging',
  'dataPipeline.chip',
  'dataPipeline.gaps',
  'dataPipeline.actions',
  'dataPipeline.status.healthy',
  'dataPipeline.status.degraded',
  'dataPipeline.status.stale',
  'dataPipeline.status.insufficient',
  'dataPipeline.confidence.high',
  'dataPipeline.confidence.medium',
  'dataPipeline.confidence.low',
  'dataPipeline.gap.matchup_coverage_low',
  'dataPipeline.gap.build_coverage_low',
  'dataPipeline.gap.rate_coverage_low',
  'dataPipeline.gap.role_coverage_low',
  'dataPipeline.gap.patch_stale',
  'dataPipeline.gap.source_stale',
  'dataPipeline.gap.high_risk_source',
  'dataPipeline.severity.low',
  'dataPipeline.severity.medium',
  'dataPipeline.severity.high',
  'dataPipeline.action.refresh_rates',
  'dataPipeline.action.refresh_builds',
  'dataPipeline.action.refresh_matchups',
  'dataPipeline.action.use_last_good_cache',
  'dataPipeline.action.manual_seed_required',
  'overlay.title',
  'overlay.waiting',
  'overlay.reminders',
  'overlay.now',
  'overlay.objective.dragon',
  'overlay.objective.baron',
  'overlay.objective.herald',
  'overlay.objective.grubs',
  'overlay.state.up',
  'overlay.state.soon',
  'overlay.state.respawning',
  'overlay.state.pending_first',
  'overlay.phase.early',
  'overlay.phase.mid',
  'overlay.phase.late',
  'pickClarity.clear',
  'pickClarity.edge',
  'pickClarity.close',
  'champSelect.scoutGameplan',
  'champSelect.scoutPrimaryThreat',
  'champSelect.scoutWinCondition.pick',
  'champSelect.scoutWinCondition.teamfight',
  'champSelect.scoutWinCondition.protect',
  'champSelect.scoutWinCondition.poke',
  'champSelect.scoutWinCondition.split',
  'champSelect.scoutWinCondition.mixed',
  'dataPipeline.cacheAction.promote',
  'dataPipeline.cacheAction.keep_current',
  'dataPipeline.cacheAction.reject',
  'dataPipeline.dataTrajectory.healthy',
  'dataPipeline.dataTrajectory.enriching',
  'dataPipeline.dataTrajectory.warming_up',
  'dataPipeline.dataTrajectory.stagnant',
  'dataPipeline.dataTrajectory.regressing',
  'dataPipeline.dataTrajectory.deferred',
  'dataPipeline.dataTrajectory.unknown',
  'dataPipeline.coverageExpansion.risk.champ_select_active',
  'dataPipeline.coverageExpansion.risk.single_player_overload',
  'dataPipeline.coverageExpansion.risk.thin_data',
  'dataPipeline.coverageExpansion.risk.no_open_frontier',
  'dataPipeline.coverageExpansion.level.low',
  'dataPipeline.coverageExpansion.level.medium',
  'dataPipeline.coverageExpansion.level.high',
  'dataPipeline.coverageExpansion.dataState.rich',
  'dataPipeline.coverageExpansion.dataState.thin',
  'dataPipeline.coverageExpansion.dataState.insufficient',
  'poolCoach.eyebrow',
  'poolCoach.title',
  'poolCoach.coverageTitle',
  'poolCoach.dataState.rich',
  'poolCoach.dataState.thin',
  'poolCoach.thinHint',
  'poolCoach.coverage.ok',
  'poolCoach.coverage.missing',
  'poolCoach.coverage.role_covered',
  'poolCoach.coverage.has_blind_safe',
  'poolCoach.coverage.has_counter_flex',
  'poolCoach.coverage.has_comfort',
  'poolCoach.coverage.identity_variety',
  'poolCoach.coverage.execution_risk',
  'poolCoach.executionRisk.low',
  'poolCoach.executionRisk.medium',
  'poolCoach.executionRisk.high',
  'poolCoach.gapsTitle',
  'poolCoach.noGaps',
  'poolCoach.noTraining',
  'poolCoach.severity.low',
  'poolCoach.severity.medium',
  'poolCoach.severity.high',
  'poolCoach.gap.role_coverage',
  'poolCoach.gap.blind_safe',
  'poolCoach.gap.counter_flex',
  'poolCoach.gap.comfort',
  'poolCoach.gap.execution_risk',
  'poolCoach.gap.identity_variety',
  'poolCoach.trainingTitle',
  'poolCoach.training.blind_safe',
  'poolCoach.training.counter_pick',
  'poolCoach.training.meta_pick',
  'settings.syncFeedbackBtn',
  'settings.syncFeedbackSyncing',
  'settings.syncFeedbackSummary',
  'settings.syncFeedbackOffline',
  'settings.syncFeedbackNothing',
  'settings.syncFeedbackFail',
  'settings.syncPipelineSummary',
  'settings.syncPipelineFail',
  'champSelect.packStale',
  'champSelect.packStaleHint',
  'champSelect.packNoTimestamp',
  'champSelect.packConfidence',
  'champSelect.localFallback',
  'champSelect.localFallbackHint',
  'champSelect.dataConfidence',
  'champSelect.dataCoverageHint',
  'champSelect.personalization.no_signal',
  'champSelect.personalization.warming_up',
  'champSelect.personalization.active',
  'champSelect.personalization.needs_sync',
  'champSelect.personalizationHint.no_signal',
  'champSelect.personalizationHint.warming_up',
  'champSelect.personalizationHint.active',
  'champSelect.personalizationHint.needs_sync',
  'champSelect.noRecommendationCoach',
  'heroCard.planLane',
  'heroCard.planMid',
  'heroCard.planFight',
  'heroCard.planFallback',
  'heroCard.planWin',
  'heroCard.planSpike',
  'heroCard.planCombo',
  'heroCard.planClash',
  'heroCard.lossRiskLabel',
  'heroCard.detailCoachTitle',
  'heroCard.pillarWhy',
  'heroCard.pillarLane',
  'heroCard.pillarMidGame',
  'heroCard.pillarTeamfight',
  'heroCard.pillarFallback',
  'heroCard.pillarRisk',
  'heroCard.pillarData',
  'heroCard.dataSourceSummary',
  'heroCard.matchupConfidence',
  'heroCard.buildConfidence',
  'heroCard.feedbackLabel',
  'heroCard.feedbackThanks',
  'heroCard.feedbackGood',
  'heroCard.feedbackBad',
  'heroCard.feedbackPool',
  'heroCard.feedbackRisk',
  // PerformancePanel reuses these instead of redefining role labels.
  'champSelect.roles.top',
  'champSelect.roles.jungle',
  'champSelect.roles.middle',
  'champSelect.roles.bottom',
  'champSelect.roles.utility',
  // Lobby Scouting / Ban Coach static labels (BanSuggestionList).
  'champSelect.banComputing',
  'champSelect.banSuggestions',
  'champSelect.banOtp',
  'champSelect.scoutingTitle',
  'champSelect.scoutingPartial',
  'champSelect.scoutingBanTargets',
  'champSelect.scoutingProfiles',
  'champSelect.confidence.high',
  'champSelect.confidence.medium',
  'champSelect.confidence.low',
  'champSelect.confidence.unknown',
  'champSelect.threat.high',
  'champSelect.threat.medium',
  'champSelect.threat.low',
];

describe('i18n tr/en parity', () => {
  it('has identical key sets across tr and en (no orphans)', () => {
    const onlyTr = trKeys.filter((k) => !(k in enFlat));
    const onlyEn = enKeys.filter((k) => !(k in trFlat));
    expect(onlyTr, `keys only in tr: ${onlyTr.join(', ')}`).toEqual([]);
    expect(onlyEn, `keys only in en: ${onlyEn.join(', ')}`).toEqual([]);
  });

  it('has the same key count in both locales', () => {
    expect(trKeys.length).toBe(enKeys.length);
  });

  it('contains every coach/data-status key the UI binds', () => {
    for (const key of REQUIRED_KEYS) {
      expect(key in trFlat, `missing in tr: ${key}`).toBe(true);
      expect(key in enFlat, `missing in en: ${key}`).toBe(true);
    }
  });

  it('has no empty string values', () => {
    const blankTr = trKeys.filter((k) => trFlat[k].trim() === '');
    const blankEn = enKeys.filter((k) => enFlat[k].trim() === '');
    expect(blankTr, `blank tr values: ${blankTr.join(', ')}`).toEqual([]);
    expect(blankEn, `blank en values: ${blankEn.join(', ')}`).toEqual([]);
  });

  it('keeps {{placeholder}} sets identical between tr and en per key', () => {
    const mismatches = trKeys
      .filter((k) => k in enFlat)
      .filter((k) => placeholders(trFlat[k]).join(',') !== placeholders(enFlat[k]).join(','))
      .map((k) => `${k}: tr[${placeholders(trFlat[k]).join(' ')}] en[${placeholders(enFlat[k]).join(' ')}]`);
    expect(mismatches, mismatches.join(' | ')).toEqual([]);
  });
});
