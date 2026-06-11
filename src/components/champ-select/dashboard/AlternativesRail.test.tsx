import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { AlternativesRail } from './AlternativesRail';
import type { Recommendation } from '../../../types/recommendation';

function rec(id: number, key: string, score: number): Recommendation {
  return {
    champion_id: id,
    champion_key: key,
    champion_name: key,
    total_score: score,
    comfort_score: 0.5,
    matchup_score: 0.5,
    team_counter_score: 0.5,
    synergy_score: 0.5,
    meta_score: 0.5,
    role_fit_score: 0.5,
    risk_score: 0.05,
    reason: '',
    core_items: [],
    situational_items: [],
    primary_rune_tree: 0,
    keystone: 0,
    tier: 'a',
    confidence: 'high',
    games_on_champ: 10,
    wins_on_champ: 6,
    enemy_team_summary: '',
    summoner_spells: [],
    secondary_runes: [],
    stat_shards: [],
  };
}

describe('AlternativesRail', () => {
  const recs = [
    rec(1, 'Zed', 0.82),
    rec(2, 'Talon', 0.71),
    rec(3, 'Ahri', 0.64),
  ];

  it('renders one option row per recommendation with names', () => {
    render(<AlternativesRail recommendations={recs} activeIndex={0} onSelect={() => {}} />);
    expect(screen.getAllByRole('option')).toHaveLength(3);
    expect(screen.getByText('Zed')).toBeInTheDocument();
    expect(screen.getByText('Talon')).toBeInTheDocument();
  });

  it('marks the active row aria-selected', () => {
    render(<AlternativesRail recommendations={recs} activeIndex={1} onSelect={() => {}} />);
    const options = screen.getAllByRole('option');
    expect(options[0]).toHaveAttribute('aria-selected', 'false');
    expect(options[1]).toHaveAttribute('aria-selected', 'true');
  });

  it('calls onSelect with the row index on click', () => {
    const onSelect = vi.fn();
    render(<AlternativesRail recommendations={recs} activeIndex={0} onSelect={onSelect} />);
    fireEvent.click(screen.getByText('Ahri'));
    expect(onSelect).toHaveBeenCalledWith(2);
  });

  it('caps at five rows', () => {
    const many = Array.from({ length: 8 }, (_, i) => rec(i + 1, `C${i}`, 0.6 - i * 0.02));
    render(<AlternativesRail recommendations={many} activeIndex={0} onSelect={() => {}} />);
    expect(screen.getAllByRole('option')).toHaveLength(5);
  });
});
