import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { invoke } from '../../lib/host';
import { MatchHistoryView } from './MatchHistoryView';
import type { MatchHistoryEntry } from '../../types/match-history';

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

/** GameReviewCard'ın detay panelinde beklediği StoredReview (get_game_review). */
const REVIEW = {
  match_id: 'M1',
  queue_group: 'soloq',
  created_at: 0,
  review: {
    champion_id: 86,
    champion_key: 'Garen',
    position: 'top',
    win: true,
    lines: [],
    went_right: 'CS iyiydi',
    to_fix: 'Vision artır',
    focus_check: null,
    next_focus: null,
    partial: false,
  },
};

const SAMPLE: MatchHistoryEntry = {
  match_id: 'M1',
  champion_id: 86,
  champion_key: 'Garen',
  position: 'top',
  queue_id: 420,
  win: 1,
  kills: 5,
  deaths: 2,
  assists: 7,
  duration_secs: 1800, // 30 dk → CS/dk = 180/30 = 6.0
  played_at: 1_700_000_000,
  cs: 180,
  cs_at_10: 60,
  deaths_pre_14: 1,
  vision_score: 22,
  has_review: 1,
};

describe('MatchHistoryView', () => {
  beforeEach(() => mockInvoke.mockReset());

  it('renders a match row with champion, role, KDA, CS/min, vision and reviewed badge', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'get_match_history') return Promise.resolve([SAMPLE]);
      return Promise.resolve(null);
    });

    render(<MatchHistoryView />);

    await waitFor(() => expect(screen.getByText('Garen')).toBeInTheDocument());
    expect(screen.getByText('5/2/7')).toBeInTheDocument(); // KDA
    expect(screen.getByText('6.0')).toBeInTheDocument(); // CS/dk
    expect(screen.getByText('22')).toBeInTheDocument(); // vision
    expect(screen.getByText(/Üst/)).toBeInTheDocument(); // rol (meta satırında: poolBuilder.role_top)
    expect(screen.getByText('Galibiyet')).toBeInTheDocument(); // sonuç
    expect(screen.getByText('İncelendi')).toBeInTheDocument(); // has_review → çip
  });

  it('hides the reviewed badge when the match has no game review', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'get_match_history') return Promise.resolve([{ ...SAMPLE, has_review: 0 }]);
      return Promise.resolve(null);
    });
    render(<MatchHistoryView />);
    await waitFor(() => expect(screen.getByText('Garen')).toBeInTheDocument());
    expect(screen.queryByText('İncelendi')).not.toBeInTheDocument();
  });

  it('shows the empty message once the summoner resolves with no matches', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'get_match_history') return Promise.resolve([]);
      return Promise.resolve(null);
    });
    render(<MatchHistoryView />);
    expect(await screen.findByText('Henüz maç kaydı yok')).toBeInTheDocument();
  });

  it('shows an honest data-error (not "no matches") when the fetch fails', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'get_match_history') return Promise.reject(new Error('backend down'));
      return Promise.resolve(null);
    });
    render(<MatchHistoryView />);
    expect(await screen.findByText('Veri alınamadı')).toBeInTheDocument();
    expect(screen.queryByText('Henüz maç kaydı yok')).not.toBeInTheDocument();
  });

  it('opens the review detail panel when a reviewed row is clicked, and back returns to the list', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'get_match_history') return Promise.resolve([SAMPLE]); // has_review: 1
      if (cmd === 'get_game_review') return Promise.resolve(REVIEW);
      if (cmd === 'get_match_note') return Promise.resolve(null);
      return Promise.resolve(null);
    });
    render(<MatchHistoryView />);

    // İncelenen satır tıklanabilir bir button (aria-label: openReview).
    const row = await screen.findByRole('button', { name: 'Garen karnesini aç' });
    fireEvent.click(row);

    // Detay paneli: GameReviewCard'ın went_right metni + geri butonu.
    expect(await screen.findByText('CS iyiydi')).toBeInTheDocument();
    const back = screen.getByRole('button', { name: /Maçlara dön/ });
    expect(back).toBeInTheDocument();

    // Geri → listeye döner.
    fireEvent.click(back);
    expect(await screen.findByRole('button', { name: 'Garen karnesini aç' })).toBeInTheDocument();
  });

  it('does not make a row clickable when the match has no review', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'get_match_history') return Promise.resolve([{ ...SAMPLE, has_review: 0 }]);
      return Promise.resolve(null);
    });
    render(<MatchHistoryView />);
    await waitFor(() => expect(screen.getByText('Garen')).toBeInTheDocument());
    expect(screen.queryByRole('button', { name: /karnesini aç/ })).not.toBeInTheDocument();
  });
});
