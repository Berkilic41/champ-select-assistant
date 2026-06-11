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
});
