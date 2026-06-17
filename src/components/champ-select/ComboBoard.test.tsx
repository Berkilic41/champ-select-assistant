import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ComboBoard } from './ComboBoard';
import type { ComboBoardEntry } from '../../types/recommendation';

const combo = (over: Partial<ComboBoardEntry> = {}): ComboBoardEntry => ({
  ally_champion_id: 61,
  ally_champion_key: 'Orianna',
  name: 'Shockwave Trap',
  combo_text: 'Nocturne R karanlık + Orianna R wombo',
  combo_type: 'wombo',
  strength: 0.92,
  ...over,
});

describe('ComboBoard', () => {
  it('renders nothing when there are no combos', () => {
    const { container } = render(<ComboBoard combos={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders combo name and description', () => {
    render(<ComboBoard combos={[combo()]} />);
    expect(screen.getByText('Shockwave Trap')).toBeInTheDocument();
    expect(screen.getByText(/Nocturne R karanlık/)).toBeInTheDocument();
  });

  it('shows the co-pick track record for an ally with history (>=2 games)', () => {
    render(
      <ComboBoard combos={[combo()]} trackRecord={{ Orianna: { games: 5, wins: 3 } }} />,
    );
    // 3/5 → %60; keyed by ally_champion_key.
    expect(screen.getByText(/5 maç.*60/)).toBeInTheDocument();
  });

  it('hides the track record when there is no entry for the ally', () => {
    render(<ComboBoard combos={[combo()]} trackRecord={{ Garen: { games: 9, wins: 5 } }} />);
    expect(screen.queryByText(/Geçmişin/)).not.toBeInTheDocument();
  });

  it('hides the track record below the 2-game floor', () => {
    render(
      <ComboBoard combos={[combo()]} trackRecord={{ Orianna: { games: 1, wins: 1 } }} />,
    );
    expect(screen.queryByText(/Geçmişin/)).not.toBeInTheDocument();
  });
});
