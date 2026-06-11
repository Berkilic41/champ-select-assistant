import React from 'react';
import { useTranslation } from 'react-i18next';
import type { Recommendation, LaneMatchup } from '../../types/recommendation';
import type { DataSourceRegistryReport } from '../../types/generated/DataSourceRegistryReport';
import type { DraftBrainQualityReport } from '../../types/generated/DraftBrainQualityReport';
import type { FeedbackObservabilityReport } from '../../types/generated/FeedbackObservabilityReport';
import type { FeedbackPersonalizationStatus } from '../../types/generated/FeedbackPersonalizationStatus';
import type { PipelineQualityReport } from '../../types/generated/PipelineQualityReport';
import type { DataTrajectoryView } from '../../types/generated/DataTrajectoryView';
import './DataStatusBadges.css';

interface Props {
  recommendations: Recommendation[];
  laneMatchup?: LaneMatchup | null;
  registryReport?: DataSourceRegistryReport | null;
  pipelineReport?: PipelineQualityReport | null;
  trajectoryReport?: DataTrajectoryView | null;
  qualityReport?: DraftBrainQualityReport | null;
  feedbackReport?: FeedbackObservabilityReport | null;
}

type ChipKind = 'neutral' | 'good' | 'warning' | 'danger';

interface Chip {
  key: string;
  label: string;
  hint?: string;
  kind?: ChipKind;
}

function confidenceLabel(confidence?: string | null): string {
  if (confidence === 'high') return 'high';
  if (confidence === 'medium') return 'medium';
  if (confidence === 'low') return 'low';
  return confidence || 'unknown';
}

function formatUnixSeconds(value?: number | null): string {
  if (value == null) return '';
  return new Date(value * 1000).toLocaleString();
}

/** Short, locale-agnostic age: "45 dk", "3 sa", "2 gün". */
function formatAgeShort(secs: number): string {
  if (secs < 3600) return `${Math.max(1, Math.round(secs / 60))} dk`;
  if (secs < 86400) return `${Math.round(secs / 3600)} sa`;
  return `${Math.round(secs / 86400)} gün`;
}

function feedbackStatusKind(status: FeedbackPersonalizationStatus): ChipKind {
  if (status === 'active') return 'good';
  if (status === 'needs_sync') return 'warning';
  return 'neutral';
}

function pipelineStatusKind(status?: string): ChipKind {
  if (status === 'healthy') return 'good';
  if (status === 'degraded') return 'warning';
  if (status === 'stale' || status === 'insufficient') return 'danger';
  return 'neutral';
}

function trajectoryKind(trajectory: string): ChipKind {
  if (trajectory === 'healthy' || trajectory === 'enriching') return 'good';
  if (trajectory === 'regressing' || trajectory === 'stagnant') return 'danger';
  return 'neutral'; // warming_up / deferred / unknown — informational
}

export const DataStatusBadges: React.FC<Props> = ({
  recommendations,
  laneMatchup,
  registryReport,
  pipelineReport,
  trajectoryReport,
  qualityReport,
  feedbackReport,
}) => {
  const { t } = useTranslation();
  const chips: Chip[] = [];

  if (qualityReport?.data_pack_fresh === false) {
    chips.push({
      key: 'pack-stale',
      label: t('champSelect.packStale'),
      hint: qualityReport.data_pack_generated_at
        ? t('champSelect.packStaleHint', {
            time: formatUnixSeconds(qualityReport.data_pack_generated_at),
          })
        : t('champSelect.packNoTimestamp'),
      kind: 'danger',
    });
  } else if (qualityReport?.data_pack_confidence) {
    const conf = confidenceLabel(qualityReport.data_pack_confidence);
    chips.push({
      key: 'pack-confidence',
      label: t('champSelect.packConfidence', { conf }),
      hint: qualityReport.data_pack_version ?? undefined,
      kind: conf === 'high' ? 'good' : conf === 'low' ? 'warning' : 'neutral',
    });
  }

  if (registryReport?.fallback_active) {
    chips.push({
      key: 'registry-fallback',
      label: t('champSelect.localFallback'),
      hint: t('champSelect.localFallbackHint'),
      kind: 'warning',
    });
  } else if (registryReport?.confidence && registryReport.confidence !== 'high') {
    chips.push({
      key: 'registry-confidence',
      label: t('champSelect.dataConfidence', { conf: registryReport.confidence }),
      hint: t('champSelect.dataCoverageHint', {
        matchups: registryReport.matchup_count,
        builds: registryReport.build_count,
      }),
      kind: registryReport.confidence === 'low' ? 'warning' : 'neutral',
    });
  }

  if (pipelineReport) {
    const topGaps = pipelineReport.gaps
      .slice(0, 2)
      .map((gap) => t(`dataPipeline.gap.${gap.dimension}`, { defaultValue: gap.dimension }));
    const topActions = pipelineReport.actions
      .slice(0, 2)
      .map((action) => t(`dataPipeline.action.${action.action}`, { defaultValue: action.action }));
    // Prefer the fused trajectory (quality + recent ramp motion) as the label — it
    // reads "growing toward target" instead of a static "degraded". Falls back to the
    // plain status until the first background tick has measured a ramp.
    const trajectory = trajectoryReport?.trajectory;
    const useTrajectory = !!trajectory && trajectory !== 'unknown';
    const label = useTrajectory
      ? t(`dataPipeline.dataTrajectory.${trajectory}`, { defaultValue: trajectory! })
      : t('dataPipeline.chip', {
          status: t(`dataPipeline.status.${pipelineReport.status}`, {
            defaultValue: pipelineReport.status,
          }),
        });
    chips.push({
      key: 'pipeline-quality',
      label,
      hint: [
        pipelineReport.summary,
        topGaps.length ? `${t('dataPipeline.gaps')}: ${topGaps.join(', ')}` : '',
        topActions.length ? `${t('dataPipeline.actions')}: ${topActions.join(', ')}` : '',
      ]
        .filter(Boolean)
        .join(' | '),
      kind: useTrajectory ? trajectoryKind(trajectory!) : pipelineStatusKind(pipelineReport.status),
    });
  }

  const hasRecs = recommendations.length > 0;
  const noMeta = hasRecs && recommendations.every((r) => Math.abs(r.meta_score - 0.3) < 0.001);
  const noMastery = hasRecs && recommendations.every((r) => r.comfort_score < 0.01);

  if (noMeta) {
    chips.push({
      key: 'meta',
      label: t('champSelect.dataNoMeta'),
      hint: t('champSelect.dataNoMetaHint'),
      kind: 'warning',
    });
  }
  if (noMastery) {
    chips.push({
      key: 'mastery',
      label: t('champSelect.dataNoMastery'),
      hint: t('champSelect.dataNoMasteryHint'),
      kind: 'warning',
    });
  }

  // Honest Match-V5 / Riot-key status (S1). Only chips when it actually informs:
  // the key is absent (live-match ingestion off) or the last successful ingest is
  // stale. Ranked below meta/mastery so the more actionable flags keep priority.
  if (trajectoryReport) {
    const STALE_SECS = 24 * 60 * 60;
    if (!trajectoryReport.riot_key_present) {
      chips.push({
        key: 'match-v5-key',
        label: t('champSelect.noRiotKey'),
        hint: t('champSelect.noRiotKeyHint'),
        kind: 'warning',
      });
    } else if (
      trajectoryReport.match_v5_enabled &&
      trajectoryReport.match_v5_age_secs != null &&
      trajectoryReport.match_v5_age_secs > STALE_SECS
    ) {
      const age = formatAgeShort(trajectoryReport.match_v5_age_secs);
      chips.push({
        key: 'match-v5-stale',
        label: t('champSelect.liveDataAge', { age }),
        hint: t('champSelect.liveDataAgeHint', { age }),
        kind: 'warning',
      });
    }
  }

  if (feedbackReport) {
    const { counters, status } = feedbackReport;
    chips.push({
      key: 'feedback-personalization',
      label: t(`champSelect.personalization.${status}`),
      hint: t(`champSelect.personalizationHint.${status}`, {
        active: counters.active_champion_signals,
        pending: counters.pending_sync,
        polar: counters.polar,
        total: counters.total,
      }),
      kind: feedbackStatusKind(status),
    });
  }

  if (laneMatchup?.inferred) {
    chips.push({
      key: 'inferred',
      label: t('laneMatchup.inferred'),
      hint: t('laneMatchup.inferredHint'),
      kind: 'neutral',
    });
  }

  const trustSources = recommendations[0]?.data_sources ?? [];
  for (const source of trustSources) {
    if (chips.length >= 3) break;
    const notable =
      source.risk_level !== 'low' ||
      source.confidence === 'low' ||
      source.confidence === 'heuristic';
    if (!notable) continue;
    const sample = source.sample_size != null ? ` - n=${source.sample_size}` : '';
    chips.push({
      key: `source-${source.source}`,
      label: `${source.source.replace(/_/g, ' ')} - ${source.confidence}`,
      hint: `${source.source}${sample}${source.patch ? ` - ${source.patch}` : ''}`,
      kind: source.confidence === 'low' || source.risk_level === 'high' ? 'warning' : 'neutral',
    });
  }

  if (chips.length === 0) return null;

  return (
    <div className="data-status" role="status">
      {chips.slice(0, 3).map((chip) => (
        <span
          key={chip.key}
          className={`data-status__chip data-status__chip--${chip.kind ?? 'neutral'}`}
          title={chip.hint}
        >
          {chip.label}
        </span>
      ))}
    </div>
  );
};
