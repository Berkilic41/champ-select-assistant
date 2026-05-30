import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { QuickPickList } from './QuickPickList';
import type { Recommendation } from '../../types/recommendation';

function makeRec(
  id: number,
  name: string,
  confidence: 'high' | 'medium' | 'low' = 'high',
): Recommendation {
  return {
    champion_id: id,
    champion_key: name,
    champion_name: name,
    total_score: 0.7,
    comfort_score: 0.7,
    matchup_score: 0.6,
    team_counter_score: 0.6,
    synergy_score: 0.5,
    meta_score: 0.6,
    role_fit_score: 0.7,
    risk_score: 0.05,
    reason: 'Test reason',
    core_items: [],
    situational_items: [],
    primary_rune_tree: 0,
    keystone: 0,
    tier: 'a',
    confidence,
    games_on_champ: 10,
    wins_on_champ: 6,
    enemy_team_summary: '',
    summoner_spells: [],
    secondary_runes: [],
    stat_shards: [],
  };
}

describe('QuickPickList', () => {
  const recs = [makeRec(1, 'Zed', 'high'), makeRec(2, 'Lux', 'low'), makeRec(3, 'Vi')];

  it('renders all recommendations', () => {
    render(<QuickPickList recommendations={recs} activeIndex={0} onSelect={() => {}} />);
    expect(screen.getByText('Zed')).toBeInTheDocument();
    expect(screen.getByText('Lux')).toBeInTheDocument();
  });

  it('calls onSelect with correct index on click', () => {
    const onSelect = vi.fn();
    render(<QuickPickList recommendations={recs} activeIndex={0} onSelect={onSelect} />);
    fireEvent.click(screen.getByText('Lux'));
    expect(onSelect).toHaveBeenCalledWith(1);
  });

  it('low confidence shows ●○○', () => {
    render(<QuickPickList recommendations={recs} activeIndex={0} onSelect={() => {}} />);
    const rows = document.querySelectorAll('.quick-pick-row');
    // Lux is index 1 — low confidence → ●○○
    expect(rows[1].textContent).toContain('●○○');
  });

  it('limits to 5 entries', () => {
    const many = Array.from({ length: 8 }, (_, i) => makeRec(i, `Champ${i}`));
    render(<QuickPickList recommendations={many} activeIndex={0} onSelect={() => {}} />);
    expect(document.querySelectorAll('.quick-pick-row').length).toBe(5);
  });
});
