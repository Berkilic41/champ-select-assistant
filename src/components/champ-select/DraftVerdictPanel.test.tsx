import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { DraftVerdictPanel } from './DraftVerdictPanel';
import type { DraftVerdict } from '../../types/recommendation';

const verdict = (over: Partial<DraftVerdict> = {}): DraftVerdict => ({
  favorability: 'favorable',
  score: 0.9,
  headline: 'Draft lehte görünüyor',
  reasons: ['Lane eşleşmen lehte'],
  dodge_consider: false,
  dodge_note: null,
  top_action: 'Tempo bas',
  team_needs: [],
  ...over,
});

describe('DraftVerdictPanel', () => {
  it('renders nothing when verdict is null', () => {
    const { container } = render(<DraftVerdictPanel verdict={null} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders headline, favorability badge and top action', () => {
    render(<DraftVerdictPanel verdict={verdict()} />);
    expect(screen.getByText('Draft lehte görünüyor')).toBeInTheDocument();
    expect(screen.getByText('Lehte')).toBeInTheDocument();
    expect(screen.getByText(/Tempo bas/)).toBeInTheDocument();
  });

  it('shows the dodge note only when dodge_consider is set', () => {
    render(
      <DraftVerdictPanel
        verdict={verdict({ favorability: 'risky', dodge_consider: true, dodge_note: 'Ciddi dezavantaj — dodge düşün' })}
      />,
    );
    expect(screen.getByText(/dodge düşün/)).toBeInTheDocument();
  });
});
