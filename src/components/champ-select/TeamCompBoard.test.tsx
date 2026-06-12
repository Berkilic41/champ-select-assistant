import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { TeamCompBoard, chatSuggestions } from './TeamCompBoard';
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

  // B4: takım sohbeti yardımcısı — yalnız panoya kopyalanır, LCU chat'e yazım YOK.
  it('chat suggestions: gated below 2 picks, capped at 2 lines, deficit-driven', () => {
    const t = ((key: string) => key) as never;
    // <2 bilinen pick → boş draft "her şey eksik" diye spam'lemez.
    expect(chatSuggestions(side({ tanks: 1 }), t)).toEqual([]);
    // engage+frontline yok + full AD → ilk 2 eksik döner (cap 2).
    const lines = chatSuggestions(
      side({ fighters: 1, marksmen: 2, ad_share: 0.9, ap_share: 0.05 }),
      t,
    );
    expect(lines).toEqual(['teamComp.chat.needEngage', 'teamComp.chat.needFrontline']);
    // Eksik yoksa satır yok.
    expect(
      chatSuggestions(
        side({
          tanks: 1,
          mages: 1,
          has_engage: true,
          has_frontline: true,
          has_peel: true,
          ap_share: 0.5,
          ad_share: 0.5,
        }),
        t,
      ),
    ).toEqual([]);
  });

  it('renders copyable chat rows when the ally comp has deficits', () => {
    const comp: TeamCompBoardData = {
      ally: side({ fighters: 1, marksmen: 1, ad_share: 0.9, ap_share: 0.05 }),
      enemy: side({ marksmen: 1 }),
    };
    render(<TeamCompBoard comp={comp} />);
    expect(screen.getByText('Takım sohbeti önerisi')).toBeInTheDocument();
    expect(screen.getAllByLabelText('Kopyala').length).toBeGreaterThan(0);
  });
});
