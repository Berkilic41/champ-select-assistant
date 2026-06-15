import { describe, it, expect, vi } from 'vitest';
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

  it('the Detay button requests the deep-dive (fires onExpand)', () => {
    const onExpand = vi.fn();
    render(<HeroCard rec={mockRec} onExpand={onExpand} />);
    // The in-card detail overlay is gone — depth now lives in the Deep-Dive tab.
    expect(screen.queryByText(/AP ağırlıklı/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /Detay/i }));
    expect(onExpand).toHaveBeenCalledOnce();
    expect(screen.queryByText(/AP ağırlıklı/)).not.toBeInTheDocument();
  });

  it('shows game plan essentials inline (win condition + combo + risk)', () => {
    const fullPlan: DraftPlan = {
      combo_with: [{
        ally_champion_id: 61,
        ally_champion_key: 'Orianna',
        combo_text: 'Strong wombo',
        combo_type: 'wombo',
      }],
      win_condition: 'Teamfight odaklı',
      team_role: 'carry',
      damage_profile: 'AP burst',
      blind_pick_safety: 0.9,
      execution_difficulty: 3,
      threats: [],
      fills_team_need: ['Engage eksikliği'],
      risk_note: 'Stretch pick — sınırlı mastery',
    };
    render(<HeroCard rec={{ ...mockRec, draft_plan: fullPlan }} />);
    // Plan essentials are visible WITHOUT expanding (decision-card design).
    expect(screen.getByText('Teamfight odaklı')).toBeInTheDocument();
    expect(screen.getByText(/Orianna: Strong wombo/)).toBeInTheDocument();
    expect(screen.getByText('Stretch pick — sınırlı mastery')).toBeInTheDocument();
  });

  it('shows the calibrated win-prob badge when an estimate is attached', () => {
    render(
      <HeroCard
        rec={{
          ...mockRec,
          win_prob: { score: 0.85, probability: 0.64, confidence: 'medium', sample_size: 30 },
        }}
      />,
    );
    expect(screen.getByText(/Tahmini kazanma ~%64/)).toBeInTheDocument();
  });

  it('hides the win-prob badge when no estimate (gated / cold start)', () => {
    render(<HeroCard rec={mockRec} />);
    expect(screen.queryByText(/Tahmini kazanma/)).not.toBeInTheDocument();
  });

  it('shows score breakdown inline without expanding', () => {
    render(<HeroCard rec={mockRec} />);
    // Scores are part of the card face now, not hidden behind "Detay".
    expect(screen.getByText('Lane Matchup')).toBeInTheDocument();
    expect(screen.getByText('Sinerji')).toBeInTheDocument();
    expect(screen.getByText('Meta')).toBeInTheDocument();
  });

  it('labels inline plan pillars and surfaces the lose condition', () => {
    render(
      <HeroCard
        rec={{
          ...mockRec,
          lane_plan: 'İlk üç wave güvenli oyna.',
          mid_game_plan: 'Yan koridor baskısı kur.',
          teamfight_job: 'Backline erişimini bekle.',
          fallback_plan: 'Gerideysen pick arama.',
          risk_summary: 'Erken ölürsen tempo kaybolur.',
        }}
      />,
    );

    expect(screen.getByText('Lane')).toBeInTheDocument();
    expect(screen.getByText('Mid game')).toBeInTheDocument();
    expect(screen.getByText('Teamfight')).toBeInTheDocument();
    expect(screen.getByText('Kaybetme riski')).toBeInTheDocument();
    expect(screen.getByText('Erken ölürsen tempo kaybolur.')).toBeInTheDocument();
  });

  it('appends co-pick history to the combo line when present (3C)', () => {
    const plan: DraftPlan = {
      combo_with: [{
        ally_champion_id: 61,
        ally_champion_key: 'Orianna',
        combo_text: 'Strong wombo',
        combo_type: 'wombo',
      }],
      win_condition: 'x',
      team_role: 'x',
      damage_profile: 'x',
      blind_pick_safety: 0.5,
      execution_difficulty: 3,
      threats: [],
      fills_team_need: [],
    };
    render(<HeroCard rec={{ ...mockRec, draft_plan: plan, combo_history: { games: 3, wins: 2 } }} />);
    // Orianna: Strong wombo · geçmişin 3M %67  (round(2/3*100)=67)
    expect(screen.getByText(/geçmişin 3M %67/)).toBeInTheDocument();
  });

  it('omits co-pick history when absent or below the 2-game gate (3C)', () => {
    const plan: DraftPlan = {
      combo_with: [{
        ally_champion_id: 61,
        ally_champion_key: 'Orianna',
        combo_text: 'Strong wombo',
        combo_type: 'wombo',
      }],
      win_condition: 'x',
      team_role: 'x',
      damage_profile: 'x',
      blind_pick_safety: 0.5,
      execution_difficulty: 3,
      threats: [],
      fills_team_need: [],
    };
    render(<HeroCard rec={{ ...mockRec, draft_plan: plan, combo_history: { games: 1, wins: 1 } }} />);
    expect(screen.queryByText(/geçmişin/)).not.toBeInTheDocument();
  });

});
