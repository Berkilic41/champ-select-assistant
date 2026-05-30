import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { HeroCard } from './HeroCard';
import type { Recommendation, DraftPlan } from '../../types/recommendation';

const mockRec: Recommendation = {
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
  reason: 'Komfor: yüksek · Meta güçlü',
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

describe('HeroCard', () => {
  it('renders champion name', () => {
    render(<HeroCard rec={mockRec} />);
    expect(screen.getByText('Zed')).toBeInTheDocument();
  });

  it('renders reason text', () => {
    render(<HeroCard rec={mockRec} />);
    expect(screen.getByText(/Komfor: yüksek/)).toBeInTheDocument();
  });

  it('no low-confidence warning when confidence is high', () => {
    render(<HeroCard rec={mockRec} />);
    expect(screen.queryByText(/Az veri/)).not.toBeInTheDocument();
  });

  it('shows low-confidence warning when confidence is low', () => {
    render(<HeroCard rec={{ ...mockRec, confidence: 'low', games_on_champ: 1 }} />);
    expect(screen.getByText(/Az veri/)).toBeInTheDocument();
  });

  it('expand button toggles detail panel', () => {
    render(<HeroCard rec={mockRec} />);
    const expandBtn = screen.getByRole('button', { name: /Detay/i });
    // Detail panel not visible initially
    expect(screen.queryByText(/AP ağırlıklı/)).not.toBeInTheDocument();
    fireEvent.click(expandBtn);
    expect(screen.getByText(/AP ağırlıklı/)).toBeInTheDocument();
  });

  it('expand panel shows score rows', () => {
    render(<HeroCard rec={mockRec} />);
    fireEvent.click(screen.getByRole('button', { name: /Detay/i }));
    expect(screen.getByText('Lane Matchup')).toBeInTheDocument();
    expect(screen.getByText('Meta')).toBeInTheDocument();
  });

  it('hides detail panel on overlay click', () => {
    render(<HeroCard rec={mockRec} />);
    fireEvent.click(screen.getByRole('button', { name: /Detay/i }));
    expect(screen.getByText(/AP ağırlıklı/)).toBeInTheDocument();
    const overlay = document.querySelector('.hero-detail-overlay')!;
    fireEvent.click(overlay);
    expect(screen.queryByText(/AP ağırlıklı/)).not.toBeInTheDocument();
  });

  it('QuickTags renders at most 3 chips', () => {
    const fullPlan: DraftPlan = {
      combo_with: [{
        ally_champion_id: 61,
        ally_champion_key: 'Orianna',
        combo_text: 'Strong wombo',
        combo_type: 'wombo',
      }],
      win_condition: 'teamfight',
      team_role: 'carry',
      damage_profile: 'AP burst',
      blind_pick_safety: 0.9,
      execution_difficulty: 3,
      threats: [],
      fills_team_need: ['Engage eksikliği'],
      risk_note: 'Stretch pick — sınırlı mastery',
    };
    render(<HeroCard rec={{ ...mockRec, draft_plan: fullPlan }} />);
    const tags = document.querySelectorAll('.hero-card__quick-tag');
    expect(tags.length).toBe(3);
    expect(screen.getByText('Stretch pick — sınırlı mastery')).toBeInTheDocument();
    expect(screen.getByText('Orianna ile güçlü combo')).toBeInTheDocument();
    expect(screen.getByText('Engage eksikliği')).toBeInTheDocument();
  });
});
