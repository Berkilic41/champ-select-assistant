import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { DataStatusBadges } from './DataStatusBadges';
import type { Recommendation, LaneMatchup } from '../../types/recommendation';
import type { DataSourceRegistryReport } from '../../types/generated/DataSourceRegistryReport';
import type { DraftBrainQualityReport } from '../../types/generated/DraftBrainQualityReport';
import type { FeedbackObservabilityReport } from '../../types/generated/FeedbackObservabilityReport';
import type { PipelineQualityReport } from '../../types/generated/PipelineQualityReport';

// The component only reads meta_score / comfort_score / length, so minimal casts suffice.
function rec(meta: number, comfort: number): Recommendation {
  return { meta_score: meta, comfort_score: comfort } as Recommendation;
}

const inferredMatchup = {
  opponent_key: 'Zed',
  opponent_name: 'Zed',
  phase_advantage: [0.5, 0.5, 0.5],
  tips: [],
  inferred: true,
} as LaneMatchup;

const highQualityReport: DraftBrainQualityReport = {
  feedback_total: 0,
  feedback_unsynced: 0,
  model_pack_version: 'model-v1',
  data_pack_version: 'data-v1',
  data_pack_confidence: 'high',
  data_pack_generated_at: 1_800_000_000,
  data_pack_fresh: true,
  local_rules_version: 'rules-v1',
  cloud_configured: true,
  notes: [],
};

const fallbackRegistry: DataSourceRegistryReport = {
  champion_rates_count: 0,
  matchup_count: 0,
  build_count: 0,
  primary_role_build_coverage: 0,
  meta_role_coverage: 0,
  exact_matchup_coverage: 0,
  stale_sources: [],
  high_risk_sources: [],
  fallback_active: true,
  confidence: 'low',
  sources: [],
  generated_at: 0,
};

const activeFeedbackReport: FeedbackObservabilityReport = {
  counters: {
    total: 9,
    polar: 9,
    neutral: 0,
    active_champion_signals: 2,
    pending_sync: 0,
  },
  status: 'active',
};

const degradedPipelineReport: PipelineQualityReport = {
  status: 'degraded',
  confidence: 'medium',
  coverage: {
    champion_rate_coverage: 0.9,
    build_coverage: 0.4,
    matchup_coverage: 0.2,
    role_coverage: 0.8,
    patch_fresh: true,
    sources_fresh: true,
    fallback_available: true,
    last_good_cache_available: true,
    has_high_risk_source: false,
    sources: [],
  },
  gaps: [
    { dimension: 'matchup_coverage_low', severity: 'medium', note: 'n' },
    { dimension: 'build_coverage_low', severity: 'medium', note: 'n' },
  ],
  actions: [
    { action: 'refresh_matchups', reason: 'r' },
    { action: 'refresh_builds', reason: 'r' },
  ],
  summary: 'Veri kullanilabilir ama eksik.',
};

describe('DataStatusBadges', () => {
  it('renders nothing without recommendations', () => {
    const { container } = render(<DataStatusBadges recommendations={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it('shows the no-meta chip when every rec is meta-neutral (0.3)', () => {
    render(<DataStatusBadges recommendations={[rec(0.3, 0.5), rec(0.3, 0.7)]} />);
    expect(screen.getByText(/genel sıralama/)).toBeInTheDocument();
  });

  it('shows the no-mastery chip when comfort is 0 across the board', () => {
    render(<DataStatusBadges recommendations={[rec(0.55, 0), rec(0.6, 0)]} />);
    expect(screen.getByText('Maç geçmişi yüklenmedi')).toBeInTheDocument();
  });

  it('shows the inferred-opponent chip from laneMatchup', () => {
    render(
      <DataStatusBadges recommendations={[rec(0.55, 0.5)]} laneMatchup={inferredMatchup} />,
    );
    expect(screen.getByText('Tahmini rakip')).toBeInTheDocument();
  });

  it('caps at 3 chips', () => {
    const { container } = render(
      <DataStatusBadges recommendations={[rec(0.3, 0)]} laneMatchup={inferredMatchup} />,
    );
    expect(container.querySelectorAll('.data-status__chip')).toHaveLength(3);
  });

  it('shows pack confidence from DraftBrain quality report', () => {
    render(
      <DataStatusBadges
        recommendations={[rec(0.55, 0.5)]}
        qualityReport={highQualityReport}
      />,
    );
    expect(screen.getByText('Paket high')).toBeInTheDocument();
  });

  it('prioritizes stale data pack warning', () => {
    render(
      <DataStatusBadges
        recommendations={[rec(0.55, 0.5)]}
        qualityReport={{ ...highQualityReport, data_pack_fresh: false }}
      />,
    );
    expect(screen.getByText('Veri paketi eski')).toBeInTheDocument();
  });

  it('shows local fallback from registry report', () => {
    render(
      <DataStatusBadges
        recommendations={[rec(0.55, 0.5)]}
        registryReport={fallbackRegistry}
      />,
    );
    expect(screen.getByText('Yerel yedek')).toBeInTheDocument();
  });

  it('shows feedback personalization status from the Rust status token', () => {
    render(
      <DataStatusBadges
        recommendations={[rec(0.55, 0.5)]}
        feedbackReport={activeFeedbackReport}
      />,
    );
    expect(screen.getByText('Kişisel öneriler aktif')).toBeInTheDocument();
  });

  it('shows pipeline quality status from the production quality report', () => {
    render(
      <DataStatusBadges
        recommendations={[rec(0.55, 0.5)]}
        pipelineReport={degradedPipelineReport}
      />,
    );
    expect(screen.getByText('Pipeline eksik')).toBeInTheDocument();
  });

  it('prefers the fused trajectory label over the static pipeline status', () => {
    render(
      <DataStatusBadges
        recommendations={[rec(0.55, 0.5)]}
        pipelineReport={degradedPipelineReport}
        trajectoryReport={{
          trajectory: 'warming_up',
          quality_status: 'degraded',
          ramp_state: 'progressing',
          data_growing: true,
          measured_at: 1_779_000_000,
          riot_key_present: true,
          match_v5_enabled: false,
          match_v5_last_success_at: null,
          match_v5_age_secs: null,
        }}
      />,
    );
    // The honest "growing" message replaces the static "Pipeline eksik".
    expect(screen.getByText('Veri büyüyor, hedefe yaklaşıyor')).toBeInTheDocument();
    expect(screen.queryByText('Pipeline eksik')).not.toBeInTheDocument();
  });

  it('falls back to the static pipeline status when trajectory is unknown', () => {
    render(
      <DataStatusBadges
        recommendations={[rec(0.55, 0.5)]}
        pipelineReport={degradedPipelineReport}
        trajectoryReport={{
          trajectory: 'unknown',
          quality_status: 'degraded',
          ramp_state: 'unknown',
          data_growing: false,
          measured_at: null,
          riot_key_present: false,
          match_v5_enabled: false,
          match_v5_last_success_at: null,
          match_v5_age_secs: null,
        }}
      />,
    );
    expect(screen.getByText('Pipeline eksik')).toBeInTheDocument();
  });

  it('prioritizes pending feedback sync in the personalization chip', () => {
    render(
      <DataStatusBadges
        recommendations={[rec(0.55, 0.5)]}
        feedbackReport={{ ...activeFeedbackReport, counters: { ...activeFeedbackReport.counters, pending_sync: 2 }, status: 'needs_sync' }}
      />,
    );
    expect(screen.getByText('Feedback sync bekliyor')).toBeInTheDocument();
  });
});
