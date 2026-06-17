import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { invoke } from '../../lib/host';
import { PoolBuilder } from './PoolBuilder';

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

describe('PoolBuilder', () => {
  beforeEach(() => mockInvoke.mockReset());

  it('renders suggestions and the pool coach plan for the selected role', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'get_pool_suggestions') {
        return Promise.resolve([
          { champion_id: 1, champion_key: 'Ahri', archetype: 'burst_mage', reason: 'role fit', ease: 4 },
        ]);
      }
      if (cmd === 'get_champion_pool_plan') {
        return Promise.resolve({
          role: 'middle',
          data_state: 'rich',
          pool_size: 3,
          coverage: {
            role_covered: true,
            has_blind_safe: false,
            has_counter_flex: true,
            has_comfort: true,
            identity_variety: false,
            execution_risk: 'medium',
          },
          gaps: [
            {
              dimension: 'blind_safe',
              severity: 'high',
              note: 'Blind first-pick icin guvenli secenegin yok.',
            },
          ],
          training: [
            {
              champion_id: 2,
              champion_key: 'Orianna',
              role_in_plan: 'blind_safe',
              reason: 'Blind first-pick icin guvenli.',
              confidence: 'medium',
              drills: ['Skillshot isabeti pratigi yap.'],
            },
          ],
          summary: 'middle havuzu 3 sampiyon; oncelikli aciklar: blind_safe.',
        });
      }
      return Promise.resolve(null);
    });

    render(<PoolBuilder />);
    await waitFor(() => expect(screen.getByText('Ahri')).toBeInTheDocument());
    expect(screen.getByText('Burst Mage')).toBeInTheDocument();
    expect(screen.getByText('Havuz koçu')).toBeInTheDocument();
    expect(screen.getByText('Orianna')).toBeInTheDocument();
    expect(screen.getByText('Blind güvenli pick')).toBeInTheDocument();
    // Role tabs present (default mid).
    expect(screen.getByText('Orta')).toBeInTheDocument();
  });

  it('shows a loading message (not "no suggestions") while the summoner is still resolving', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve(null);
      if (cmd === 'get_champion_pool_plan') return Promise.resolve(null);
      return Promise.resolve([]);
    });
    render(<PoolBuilder />);
    expect(await screen.findByText('Öneriler yükleniyor…')).toBeInTheDocument();
    expect(screen.queryByText('Bu rol için öneri yok')).not.toBeInTheDocument();
  });

  it('shows the empty message once the summoner resolves with no suggestions', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'get_champion_pool_plan') return Promise.resolve(null);
      return Promise.resolve([]);
    });
    render(<PoolBuilder />);
    expect(await screen.findByText('Bu rol için öneri yok')).toBeInTheDocument();
  });

  it('shows an honest data-error (not "no suggestions") when the pool fetch fails', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'get_pool_suggestions') return Promise.reject(new Error('backend down'));
      if (cmd === 'get_champion_pool_plan') return Promise.resolve(null);
      return Promise.resolve(null);
    });
    render(<PoolBuilder />);
    // Backend/DB hatası "bu rol için öneri yok" gibi değil dürüst hata gösterir.
    expect(await screen.findByText('Veri alınamadı')).toBeInTheDocument();
    expect(screen.queryByText('Bu rol için öneri yok')).not.toBeInTheDocument();
  });

  it('renders learning-targets with gain, no-movement and match win-rate states (Epic #4)', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_active_summoner_puuid') return Promise.resolve('p');
      if (cmd === 'get_learning_progress')
        return Promise.resolve([
          { champion_id: 86, champion_key: 'Garen', points_gained: 500, current_level: 6, games_played: 4, wins: 3 },
          { champion_id: 99, champion_key: 'Lux', points_gained: 200, current_level: 4, games_played: 2, wins: 1 },
          { champion_id: 64, champion_key: 'LeeSin', points_gained: 0, current_level: 7, games_played: 0, wins: 0 },
        ]);
      if (cmd === 'get_champion_pool_plan') return Promise.resolve(null);
      return Promise.resolve([]);
    });
    render(<PoolBuilder />);
    expect(await screen.findByText('Öğrenme hedeflerin')).toBeInTheDocument();
    expect(screen.getByText('Garen')).toBeInTheDocument();
    expect(screen.getByText('LeeSin')).toBeInTheDocument();
    // gain>0 → puan satırı; gain=0 → "henüz hareket yok".
    expect(screen.getByText('+500 puan · Sv 6')).toBeInTheDocument();
    expect(screen.getByText('İşaretli — henüz hareket yok')).toBeInTheDocument();
    // games>=3 → maç sayısı + WR; 1-2 maç → ince-örneklem dürüstlüğü (sadece sayı, WR yok).
    expect(screen.getByText('4 maç · %75')).toBeInTheDocument();
    expect(screen.getByText('2 maç')).toBeInTheDocument();
    // games=0 → maç alt-satırı hiç gösterilmez (dürüst gizleme, "%0" uydurmaz).
    expect(screen.queryByText('0 maç')).not.toBeInTheDocument();
  });

  it('exposes the role buttons as a labelled group with aria-pressed (a11y)', () => {
    mockInvoke.mockResolvedValue(null);
    const { container } = render(<PoolBuilder />);
    expect(container.querySelector('.pool-builder__roles')).toHaveAttribute('role', 'group');
    // Varsayılan rol (orta) aktif → aria-pressed=true.
    const active = container.querySelector('.pool-builder__role--active');
    expect(active).toHaveAttribute('aria-pressed', 'true');
  });
});
