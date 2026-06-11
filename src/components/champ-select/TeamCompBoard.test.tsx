import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { TeamCompBoard } from './TeamCompBoard';
import type { CompSummary, TeamCompBoard as TeamCompBoardData } from '../../types/recommendation';

const side = (over: Partial<CompSummary> = {}): CompSummary => ({
  tanks: 0,
  fighters: 0,
  mages: 0,
  marksmen: 0,
  assassins: 0,
  supports: 0,
  ap_share: 0,
  ad_share: 0,
  has_engage: false,
  has_frontline: false,
  has_hard_cc: false,
  has_peel: false,
  gaps: [],
  summary: 'dengeli takım',
  ...over,
});

describe('TeamCompBoard', () => {
  it('renders nothing when comp is null', () => {
    const { container } = render(<TeamCompBoard comp={null} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders role chips and gap badges', () => {
    const comp: TeamCompBoardData = {
      ally: side({ tanks: 1, mages: 2, summary: 'AP ağırlıklı', gaps: ['Engage yok'] }),
      enemy: side({ marksmen: 1 }),
    };
    render(<TeamCompBoard comp={comp} />);
    expect(screen.getByText(/Tank ×1/)).toBeInTheDocument();
    expect(screen.getByText(/Büyücü ×2/)).toBeInTheDocument();
    expect(screen.getByText('Engage yok')).toBeInTheDocument();
  });
});
