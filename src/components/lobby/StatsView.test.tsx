import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { invoke } from '../../lib/host';
import { StatsView } from './StatsView';
import type { ChampionStats } from '../../types/riot';

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

// Yalnız champion_id/games/wins okunuyor → minimal cast yeterli.
function stat(champion_id: number, games: number, wins: number): ChampionStats {
  return { champion_id, games, wins } as unknown as ChampionStats;
}

describe('StatsView', () => {
  // Çocuk kartlar (RankCard/TrendPanel/...) puuid çözer; null döndür → sessizce null render.
  beforeEach(() => mockInvoke.mockResolvedValue(null));

  it('shows a thin-data note instead of silently hiding the win-rate section when no champion has 3+ games', () => {
    // Tüm şampiyonlar <3 maç → wrData boş. Eskiden bölüm sessizce kaybolurdu;
    // artık dürüst bir "yeterli maç yok" notu görünür. (Mastery de boş → recharts render olmaz.)
    render(
      <StatsView
        stats={[stat(64, 2, 1), stat(51, 1, 0)]}
        masteries={[]}
        champMap={
          new Map([
            [64, 'LeeSin'],
            [51, 'Caitlyn'],
          ])
        }
      />,
    );
    expect(
      screen.getByText(
        'En az 3 maç oynanmış şampiyon yok — galibiyet oranı için daha fazla maç gerek',
      ),
    ).toBeInTheDocument();
  });

  it('renders the global empty state when there is no data at all', () => {
    render(<StatsView stats={[]} masteries={[]} champMap={new Map()} />);
    expect(screen.getByText('Veri yok — önce senkronize et.')).toBeInTheDocument();
  });
});
