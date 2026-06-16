import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { DataStatusBadges } from './DataStatusBadges';
import type { Recommendation, LaneMatchup } from '../../types/recommendation';
import type { DataSourceRegistryReport } from '../../types/generated/DataSourceRegistryReport';
import type { DraftBrainQualityReport } from '../../types/generated/DraftBrainQualityReport';
import type { FeedbackObservabilityReport } from '../../types/generated/FeedbackObservabilityReport';
import type { PipelineQualityReport } from '../../types/generated/PipelineQualityReport';
import type { DataTrajectoryView } from '../../types/generated/DataTrajectoryView';

// The component reads missing_signals (noMeta) / comfort_score (noMastery) / length.
// `missing` carries the structural backend signal (e.g. ['meta'] = no meta-rate row).
function rec(meta: number, comfort: number, missing: string[] = []): Recommendation {
  return { meta_score: meta, comfort_score: comfort, missing_signals: missing } as Recommendation;
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

// Sağlıklı taban: prod-key var, Match-V5 açık, taze. Testler ilgili alanı ezer.
const baseTrajectory: DataTrajectoryView = {
  trajectory: 'unknown',
  quality_status: 'degraded',
  ramp_state: 'unknown',
  data_growing: false,
  measured_at: null,
  riot_key_present: true,
  match_v5_enabled: true,
  match_v5_last_success_at: null,
  match_v5_age_secs: null,
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

  it('shows the no-meta chip when every rec structurally lacks meta', () => {
    render(
      <DataStatusBadges recommendations={[rec(0.3, 0.5, ['meta']), rec(0.3, 0.7, ['meta'])]} />,
    );
    expect(screen.getByText(/genel sıralama/)).toBeInTheDocument();
  });

  it('does NOT show no-meta when meta_score happens to be 0.3 but meta IS present', () => {
    // Eski sihirli-sabit yanlış-pozitifi: ~%50.1 WR → meta_score 0.3 ama gerçek meta var.
    // Yapısal missing_signals boş → chip çıkmamalı.
    render(<DataStatusBadges recommendations={[rec(0.3, 0.5, []), rec(0.3, 0.7, [])]} />);
    expect(screen.queryByText(/genel sıralama/)).not.toBeInTheDocument();
  });

  it('shows the no-mastery chip when comfort is 0 across the board', () => {
    render(<DataStatusBadges recommendations={[rec(0.55, 0), rec(0.6, 0)]} />);
    expect(screen.getByText('Maç geçmişi yüklenmedi')).toBeInTheDocument();
  });

  it('shows the no-Riot-key chip when the trajectory reports no production key', () => {
    render(
      <DataStatusBadges
        recommendations={[rec(0.55, 0.5)]}
        trajectoryReport={{ ...baseTrajectory, riot_key_present: false }}
      />,
    );
    expect(
      screen.getByText('Riot anahtarı yok · canlı maç verisi kapalı'),
    ).toBeInTheDocument();
  });

  it('shows the stale live-data chip when Match-V5 ingest is older than 24h', () => {
    render(
      <DataStatusBadges
        recommendations={[rec(0.55, 0.5)]}
        trajectoryReport={{ ...baseTrajectory, match_v5_age_secs: 259200 }}
      />,
    );
    expect(screen.getByText('Canlı veri: 3 gün')).toBeInTheDocument();
  });

  it('does NOT show the stale chip when Match-V5 ingest is fresh', () => {
    render(
      <DataStatusBadges
        recommendations={[rec(0.55, 0.5)]}
        trajectoryReport={{ ...baseTrajectory, match_v5_age_secs: 3600 }}
      />,
    );
    expect(screen.queryByText(/Canlı veri:/)).not.toBeInTheDocument();
  });

  it('shows the inferred-opponent chip from laneMatchup', () => {
    render(
      <DataStatusBadges recommendations={[rec(0.55, 0.5)]} laneMatchup={inferredMatchup} />,
    );
    expect(screen.getByText('Tahmini rakip')).toBeInTheDocument();
  });

  it('caps at 3 chips', () => {
    const { container } = render(
      <DataStatusBadges recommendations={[rec(0.3, 0, ['meta'])]} laneMatchup={inferredMatchup} />,
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

  it('keeps actionable cold-start chips visible over diagnostic chips when capped', () => {
    // 5 aday chip: pack-stale + registry-fallback + pipeline (diagnostik) ve
    // noMeta + noMastery (aksiyon-alınabilir). Cap 3; aksiyon-alınabilir olanlar
    // diagnostiklerce ilk-3'ten atılmamalı.
    const { container } = render(
      <DataStatusBadges
        recommendations={[rec(0.3, 0, ['meta'])]}
        qualityReport={{ ...highQualityReport, data_pack_fresh: false }}
        registryReport={fallbackRegistry}
        pipelineReport={degradedPipelineReport}
      />,
    );
    expect(container.querySelectorAll('.data-status__chip')).toHaveLength(3);
    expect(screen.getByText(/genel sıralama/)).toBeInTheDocument(); // noMeta
    expect(screen.getByText('Maç geçmişi yüklenmedi')).toBeInTheDocument(); // noMastery
  });
});
