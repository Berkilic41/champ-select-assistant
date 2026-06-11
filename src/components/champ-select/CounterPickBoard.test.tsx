import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { CounterPickBoard } from './CounterPickBoard';
import type { CounterPickHint } from '../../types/recommendation';

const hint = (id: number, name: string, adv: number): CounterPickHint => ({
  champion_id: id,
  champion_key: name,
  champion_name: name,
  advantage: adv,
  reason: `${name} karşısında iyi eşleşme`,
  games_on_champ: 10,
});

describe('CounterPickBoard', () => {
  it('renders nothing when there are no counters', () => {
    const { container } = render(<CounterPickBoard picks={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders name, advantage % and reason for each pick', () => {
    render(<CounterPickBoard picks={[hint(238, 'Zed', 0.8)]} />);
    expect(screen.getByText('Zed')).toBeInTheDocument();
    expect(screen.getByText('80%')).toBeInTheDocument();
    expect(screen.getByText(/iyi eşleşme/)).toBeInTheDocument();
  });
});
