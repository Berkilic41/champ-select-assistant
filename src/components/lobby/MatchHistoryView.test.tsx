import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent, within } from '@testing-library/react';
import { invoke } from '../../lib/host';
import { MatchHistoryView } from './MatchHistoryView';
import type { MatchHistoryEntry } from '../../types/match-history';

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

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

// Filtre <select> option'ları satır metniyle (champ/rol/sonuç) çakıştığı için
// satır içeriği assertion'larını listeye (within) kapsıyoruz.
const rowList = () => within(screen.getByRole('list'));

describe('MatchHistoryView', () => {
  beforeEach(() => mockInvoke.mockReset());

  it('renders a match row with champion, role, KDA, CS/min, vision and reviewed badge', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'get_match_history') return Promise.resolve([SAMPLE]);
      return Promise.resolve(null);
    });

    render(<MatchHistoryView />);

    await waitFor(() => expect(screen.getByRole('list')).toBeInTheDocument());
    expect(rowList().getByText('Garen')).toBeInTheDocument();
    expect(rowList().getByText('5/2/7')).toBeInTheDocument(); // KDA
    expect(rowList().getByText('6.0')).toBeInTheDocument(); // CS/dk
    expect(rowList().getByText('22')).toBeInTheDocument(); // vision
    expect(rowList().getByText(/Üst/)).toBeInTheDocument(); // rol (meta satırı)
    expect(rowList().getByText('Galibiyet')).toBeInTheDocument(); // sonuç
    expect(rowList().getByText('İncelendi')).toBeInTheDocument(); // has_review → çip
  });

  it('hides the reviewed badge when the match has no game review', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'get_match_history') return Promise.resolve([{ ...SAMPLE, has_review: 0 }]);
      return Promise.resolve(null);
    });
    render(<MatchHistoryView />);
    await waitFor(() => expect(screen.getByRole('list')).toBeInTheDocument());
    expect(rowList().getByText('Garen')).toBeInTheDocument();
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
    await waitFor(() => expect(screen.getByRole('list')).toBeInTheDocument());
    expect(rowList().getByText('Garen')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /karnesini aç/ })).not.toBeInTheDocument();
  });

  it('filters the list by result and by champion (Slice 3)', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'get_match_history')
        return Promise.resolve([
          SAMPLE, // Garen / top / win / 5-2-7
          {
            ...SAMPLE,
            match_id: 'M2',
            champion_key: 'Ahri',
            position: 'middle',
            win: 0,
            has_review: 0,
            kills: 1,
            deaths: 9,
            assists: 3,
          },
        ]);
      return Promise.resolve(null);
    });
    render(<MatchHistoryView />);
    await waitFor(() => expect(screen.getByRole('list')).toBeInTheDocument());

    // Başta iki maç da listede (KDA ile ayırt — option çakışmasından bağımsız).
    expect(rowList().getByText('5/2/7')).toBeInTheDocument();
    expect(rowList().getByText('1/9/3')).toBeInTheDocument();

    // Sonuç = Galibiyet → yalnız Garen (win).
    fireEvent.change(screen.getByLabelText('Sonuç'), { target: { value: 'win' } });
    expect(rowList().getByText('5/2/7')).toBeInTheDocument();
    expect(rowList().queryByText('1/9/3')).not.toBeInTheDocument();

    // Sonuç sıfırla + Şampiyon = Ahri → yalnız Ahri.
    fireEvent.change(screen.getByLabelText('Sonuç'), { target: { value: 'all' } });
    fireEvent.change(screen.getByLabelText('Şampiyon'), { target: { value: 'Ahri' } });
    expect(rowList().getByText('1/9/3')).toBeInTheDocument();
    expect(rowList().queryByText('5/2/7')).not.toBeInTheDocument();
  });

  it('shows a no-filter-match message when filters exclude every match (Slice 3)', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'get_match_history') return Promise.resolve([SAMPLE]); // tek galibiyet
      return Promise.resolve(null);
    });
    render(<MatchHistoryView />);
    await waitFor(() => expect(screen.getByRole('list')).toBeInTheDocument());
    // Mağlubiyet filtrele → hiç maç yok.
    fireEvent.change(screen.getByLabelText('Sonuç'), { target: { value: 'loss' } });
    expect(screen.getByText('Bu filtreye uygun maç yok')).toBeInTheDocument();
    expect(screen.queryByRole('list')).not.toBeInTheDocument();
  });
});
