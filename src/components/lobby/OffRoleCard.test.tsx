import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { invoke } from '../../lib/host';
import { OffRoleCard } from './OffRoleCard';

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

describe('OffRoleCard', () => {
  beforeEach(() => mockInvoke.mockReset());

  it('renders the main role and weaker off-roles (Epic #3 Slice 2)', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'get_off_role_performance')
        return Promise.resolve({
          main_role: 'middle',
          main_games: 10,
          main_wins: 7,
          off_roles: [{ position: 'top', games: 5, wins: 1 }],
        });
      return Promise.resolve(null);
    });
    render(<OffRoleCard />);
    expect(await screen.findByText('Off-rol zayıflığı')).toBeInTheDocument();
    // Ana rol chip: WR %70 (7/10), rol etiketi overlay.position.middle → "Orta".
    expect(screen.getByText('Ana rol: Orta · %70')).toBeInTheDocument();
    // Zayıf off-rol satırı: "Üst" + WR %20 (1/5) · 5 maç.
    expect(screen.getByText('Üst')).toBeInTheDocument();
    expect(screen.getByText('%20 · 5 maç')).toBeInTheDocument();
  });

  it('renders nothing when there is no off-role weakness (honest hide)', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'get_off_role_performance') return Promise.resolve(null);
      return Promise.resolve(null);
    });
    const { container } = render(<OffRoleCard />);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('get_off_role_performance', { puuid: 'p' }),
    );
    // null rapor → kart hiç çizilmez (sessiz/dürüst gizleme).
    expect(screen.queryByText('Off-rol zayıflığı')).not.toBeInTheDocument();
    expect(container.querySelector('.grc-card')).toBeNull();
  });
});
