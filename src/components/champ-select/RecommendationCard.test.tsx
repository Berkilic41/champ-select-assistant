import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { RecommendationCard } from './RecommendationCard';
import type { Recommendation } from '../../types/recommendation';

const rec: Recommendation = {
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
  reason: 'Komfor yüksek',
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

const champMap = new Map<number, string>([[238, 'Zed']]);

describe('RecommendationCard', () => {
  it('renders rank, name, reason and total score', () => {
    render(<RecommendationCard rec={rec} rank={1} champMap={champMap} />);
    expect(screen.getByText('1')).toBeInTheDocument();
    expect(screen.getByText('Zed')).toBeInTheDocument();
    expect(screen.getByText('Komfor yüksek')).toBeInTheDocument();
    expect(screen.getByText('85')).toBeInTheDocument();
  });

  it('breakdown is hidden until the card is clicked', () => {
    render(<RecommendationCard rec={rec} rank={1} champMap={champMap} />);
    expect(screen.queryByText('Comfort')).not.toBeInTheDocument();
    fireEvent.click(screen.getByText('Zed'));
    expect(screen.getByText('Comfort')).toBeInTheDocument();
    expect(screen.getByText('Lane Matchup')).toBeInTheDocument();
    expect(screen.getByText('Sinerji')).toBeInTheDocument();
  });

  it('clicking again collapses the breakdown', () => {
    render(<RecommendationCard rec={rec} rank={1} champMap={champMap} />);
    const card = screen.getByText('Zed');
    fireEvent.click(card);
    expect(screen.getByText('Meta')).toBeInTheDocument();
    fireEvent.click(card);
    expect(screen.queryByText('Meta')).not.toBeInTheDocument();
  });

  it('falls back to champion_key when champMap has no entry', () => {
    render(<RecommendationCard rec={rec} rank={2} champMap={new Map()} />);
    const icon = screen.getByAltText('Zed') as HTMLImageElement;
    expect(icon.src).toContain('/champion/Zed.png');
  });
});
