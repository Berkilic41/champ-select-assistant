import { describe, it, expect, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { invoke } from '../lib/host';
import { useSummonerData } from './useSummonerData';

const mockInvoke = invoke as ReturnType<typeof import('vitest').vi.fn>;

/** LCU-first: first invoke call is sync_lcu_player_data — make it fail. */
function lcuFails() {
  mockInvoke.mockRejectedValueOnce('LCU bağlantısı yok');
}

describe('useSummonerData', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it('shows "League Client açık değil" when LCU fails and Riot key is absent', async () => {
    // LCU fails; Riot returns NotConfigured
    lcuFails();
    mockInvoke.mockRejectedValueOnce('Yapılandırma eksik: Riot API key yapılandırılmamış');
    const { result } = renderHook(() => useSummonerData('TestUser', 'TR1', 'europe'));
    await act(async () => {
      await result.current.sync();
    });
    expect(result.current.error).toBe('League Client açık değil');
    expect(result.current.loading).toBe(false);
  });

  it('shows raw error for genuine network failures', async () => {
    lcuFails();
    mockInvoke.mockRejectedValueOnce('Network connection failed');
    const { result } = renderHook(() => useSummonerData('TestUser', 'TR1', 'europe'));
    await act(async () => {
      await result.current.sync();
    });
    expect(result.current.error).toBe('Network connection failed');
  });

  it('succeeds via LCU path when sync_lcu_player_data resolves', async () => {
    mockInvoke.mockResolvedValueOnce({
      summoner: { puuid: 'test-puuid', game_name: 'TestUser', tag_line: 'TR1', region: 'tr1' },
      matches_synced: 5,
      matches_skipped: 0,
      masteries_synced: 3,
      errors: 0,
    });
    // Promise.allSettled calls: get_masteries, get_player_stats, get_champions, get_ddragon_version
    mockInvoke.mockResolvedValueOnce([]); // get_masteries
    mockInvoke.mockResolvedValueOnce([]); // get_player_stats
    mockInvoke.mockResolvedValueOnce([]); // get_champions
    mockInvoke.mockResolvedValueOnce('14.10'); // get_ddragon_version

    const { result } = renderHook(() => useSummonerData('TestUser', 'TR1', 'tr1'));
    await act(async () => {
      await result.current.sync();
    });
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeUndefined();
    expect(result.current.summonerInfo?.puuid).toBe('test-puuid');
  });
});
