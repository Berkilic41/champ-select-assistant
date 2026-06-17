import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { invoke } from '../../lib/host';
import { GameReviewCard } from './GameReviewCard';

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

/** generate_game_reviews'in döndürdüğü en-yeni StoredReview (GameReview JSON). */
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
    to_fix: 'Vizyon artır',
    focus_check: { result: 'met', label: 'CS hedefi', achieved: 6.1 },
    next_focus: null,
    partial: false,
  },
};

describe('GameReviewCard', () => {
  beforeEach(() => mockInvoke.mockReset());

  it('renders the latest review and the focus-goal streak history dots (Epic #3)', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'generate_game_reviews')
        return Promise.resolve({ created: 0, latest: REVIEW, streak: 3 });
      if (cmd === 'get_match_note') return Promise.resolve(null);
      if (cmd === 'get_focus_history')
        return Promise.resolve([
          { result: 'met', label: 'Ölüm hedefi', metric: 'deaths_per_10' },
          { result: 'missed', label: 'KDA hedefi', metric: 'kda' },
          { result: 'met', label: 'CS hedefi', metric: 'cs_per_min' },
        ]);
      return Promise.resolve(null);
    });

    const { container } = render(<GameReviewCard />);
    await waitFor(() => expect(screen.getByText('CS iyiydi')).toBeInTheDocument());

    // Hedef serisi: 3 nokta (2 met, 1 missed).
    expect(container.querySelectorAll('.grc-goal-dot')).toHaveLength(3);
    expect(container.querySelectorAll('.grc-goal-dot--met')).toHaveLength(2);
    expect(container.querySelectorAll('.grc-goal-dot--missed')).toHaveLength(1);
  });

  it('omits the streak-history row when there is no closed focus history', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'generate_game_reviews')
        return Promise.resolve({ created: 0, latest: REVIEW, streak: 0 });
      if (cmd === 'get_match_note') return Promise.resolve(null);
      if (cmd === 'get_focus_history') return Promise.resolve([]);
      return Promise.resolve(null);
    });

    const { container } = render(<GameReviewCard />);
    await waitFor(() => expect(screen.getByText('CS iyiydi')).toBeInTheDocument());
    expect(container.querySelector('.grc-streak-history')).toBeNull();
  });
});
