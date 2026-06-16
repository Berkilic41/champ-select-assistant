import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, waitFor, act } from '@testing-library/react';
import { invoke } from '../lib/host';
import { useActiveSummonerPuuid } from './useActiveSummonerPuuid';

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

describe('useActiveSummonerPuuid', () => {
  beforeEach(() => mockInvoke.mockReset());

  it('starts empty and returns the active summoner puuid once it resolves', async () => {
    mockInvoke.mockResolvedValue('puuid-1');
    const { result } = renderHook(() => useActiveSummonerPuuid());
    expect(result.current).toBe(''); // mount: henüz çözülmedi
    await waitFor(() => expect(result.current).toBe('puuid-1'));
    expect(mockInvoke).toHaveBeenCalledWith('get_active_summoner_puuid');
  });

  it('retries: an empty first response does not strand the consumer at empty', async () => {
    vi.useFakeTimers();
    mockInvoke.mockResolvedValueOnce(null).mockResolvedValue('puuid-2');
    const { result } = renderHook(() => useActiveSummonerPuuid());
    await act(async () => {
      // İlk deneme null → setTimeout(1500) ile retry planlanır; ilerlet → çözülür.
      await vi.advanceTimersByTimeAsync(1600);
    });
    expect(result.current).toBe('puuid-2');
    vi.useRealTimers();
  });
});
