import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { DeepDiveTab } from './DeepDiveTab';
import type { Recommendation, DraftPlan } from '../../../types/recommendation';

const baseRec: Recommendation = {
  champion_id: 238,
  champion_key: 'Zed',
  champion_name: 'Zed',
  total_score: 0.85,
  comfort_score: 0.9,
  matchup_score: 0.8,
  team_counter_score: 0.7,
  synergy_score: 0.6,
  meta_score: 0.75,
  role_fit_score: 0.8,
  risk_score: 0.05,
  reason: 'Komfor: yüksek',
  core_items: [],
  situational_items: [],
  primary_rune_tree: 8100,
  keystone: 8112,
  tier: 's',
  confidence: 'high',
  games_on_champ: 25,
  wins_on_champ: 18,
  enemy_team_summary: 'AP ağırlıklı',
  summoner_spells: [],
  secondary_runes: [],
  stat_shards: [],
};

describe('DeepDiveTab', () => {
  it('renders the coach read grid + data sources (lifted from HeroCard detail)', () => {
    render(
      <DeepDiveTab
        rec={{
          ...baseRec,
          decision_sentence: 'Zed seç çünkü mid-game pick tehdidi kurar.',
          lane_plan: 'İlk üç wave güvenli oyna.',
          mid_game_plan: 'Yan koridor baskısı kur.',
          teamfight_job: 'Backline erişimini bekle.',
          fallback_plan: 'Gerideysen pick arama.',
          risk_summary: 'Erken ölürsen tempo kaybolur.',
          data_sources: [{
            source: 'cloud_postgres',
            patch: '16.10',
            sample_size: 1200,
            confidence: 'high',
            risk_level: 'low',
            updated_at: 1_800_000_000,
          }],
        }}
      />,
    );
    expect(screen.getByText('Koç okuması')).toBeInTheDocument();
    expect(screen.getByText('Nasıl kaybedersin')).toBeInTheDocument();
    expect(screen.getByText('Veri güveni')).toBeInTheDocument();
    expect(screen.getAllByText(/cloud postgres/).length).toBeGreaterThan(0);
  });

  it('shows team-need gaps and the enemy comp summary', () => {
    const plan: DraftPlan = {
      combo_with: [],
      win_condition: 'Teamfight odaklı',
      team_role: 'carry',
      damage_profile: 'AP burst',
      blind_pick_safety: 0.9,
      execution_difficulty: 3,
      threats: [],
      fills_team_need: ['Engage eksikliği'],
    };
    render(<DeepDiveTab rec={{ ...baseRec, draft_plan: plan }} />);
    expect(screen.getByText('Engage eksikliği')).toBeInTheDocument();
    expect(screen.getByText(/AP ağırlıklı/)).toBeInTheDocument();
  });

  it('renders the coach note when a narrative is attached (FAZ 4)', () => {
    render(
      <DeepDiveTab
        rec={{
          ...baseRec,
          coach_narrative: {
            text: 'Zed için matchup penceren iyi.',
            source: 'deterministic',
            external_rejected: false,
          },
        }}
      />,
    );
    expect(screen.getByText('Koç notu')).toBeInTheDocument();
    expect(screen.getByText('Zed için matchup penceren iyi.')).toBeInTheDocument();
  });

  it('hides the coach note when no narrative is attached', () => {
    render(<DeepDiveTab rec={baseRec} />);
    expect(screen.queryByText('Koç notu')).not.toBeInTheDocument();
  });

  it('surfaces the KB pick profile: low blind-safety → risky band, high difficulty → hard band', () => {
    const plan: DraftPlan = {
      combo_with: [],
      win_condition: 'x',
      team_role: 'carry',
      damage_profile: 'AP',
      blind_pick_safety: 0.3, // < 0.6 → riskli
      execution_difficulty: 5, // >= 4 → zor (5/5)
      threats: [],
      fills_team_need: [],
    };
    render(<DeepDiveTab rec={{ ...baseRec, draft_plan: plan }} />);
    expect(screen.getByText('Pick profili')).toBeInTheDocument();
    expect(screen.getByText('Güvenli değil (blind)')).toBeInTheDocument();
    expect(screen.getByText('Zor (5/5)')).toBeInTheDocument();
  });

  it('shows the blind-safe + easy bands for a high-safety low-difficulty pick', () => {
    const plan: DraftPlan = {
      combo_with: [],
      win_condition: 'x',
      team_role: 'carry',
      damage_profile: 'AP',
      blind_pick_safety: 0.9, // >= 0.6 → güvenli
      execution_difficulty: 1, // <= 2 → kolay
      threats: [],
      fills_team_need: [],
    };
    render(<DeepDiveTab rec={{ ...baseRec, draft_plan: plan }} />);
    expect(screen.getByText('Blind pick güvenli')).toBeInTheDocument();
    expect(screen.getByText('Kolay')).toBeInTheDocument();
  });
});
