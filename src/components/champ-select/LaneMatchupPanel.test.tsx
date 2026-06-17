import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { LaneMatchupPanel } from './LaneMatchupPanel';
import type { LaneMatchup } from '../../types/recommendation';

describe('LaneMatchupPanel', () => {
  it('renders nothing when matchup is null', () => {
    const { container } = render(<LaneMatchupPanel matchup={null} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders the opponent and tips', () => {
    const m: LaneMatchup = {
      opponent_key: 'Zed',
      opponent_name: 'Zed',
      phase_advantage: [0.7, 0.5, 0.3],
      tips: ['Lvl 1-3 agresif bas'],
    };
    render(<LaneMatchupPanel matchup={m} />);
    expect(screen.getByText('Zed')).toBeInTheDocument();
    expect(screen.getByText(/agresif bas/)).toBeInTheDocument();
  });

  it('labels the advantage bars as a KB estimate (data honesty)', () => {
    const m: LaneMatchup = {
      opponent_key: 'Zed',
      opponent_name: 'Zed',
      phase_advantage: [0.7, 0.5, 0.3],
      source: 'kb_estimate',
      tips: [],
    };
    render(<LaneMatchupPanel matchup={m} />);
    expect(screen.getByText('KB tahmini')).toBeInTheDocument();
  });

  it('omits the estimate badge when source is absent', () => {
    const m: LaneMatchup = {
      opponent_key: 'Zed',
      opponent_name: 'Zed',
      phase_advantage: [0.7, 0.5, 0.3],
      tips: [],
    };
    render(<LaneMatchupPanel matchup={m} />);
    expect(screen.queryByText('KB tahmini')).not.toBeInTheDocument();
  });
});
