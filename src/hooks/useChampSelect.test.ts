import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useChampSelect } from './useChampSelect';

const mockInvoke = invoke as ReturnType<typeof vi.fn>;
const mockListen = listen as ReturnType<typeof vi.fn>;

function makeSession(phase: string) {
  return {
    my_cell_id: 0,
    local_player: { champion_id: 0, assigned_position: 'middle' },
    my_team: [],
    their_team: [],
    my_bans: [],
    their_bans: [],
    phase,
    time_left_ms: 30000,
    action_type: 'pick',
    queue_id: 420,
    pick_order: 1,
  };
}

describe('useChampSelect', () => {
  let handler: (e: { payload: unknown }) => void;

  beforeEach(() => {
    mockInvoke.mockReset();
    mockListen.mockReset();
    // Capture the 'champ-select-session' event handler so tests can drive it.
    mockListen.mockImplementation(
      (_event: string, cb: (e: { payload: unknown }) => void) => {
        handler = cb;
        return Promise.resolve(() => {});
      },
    );
  });

  it('fetches recommendations when a session arrives', async () => {
    mockInvoke.mockResolvedValueOnce([{ champion_id: 1, champion_key: 'Annie' }]);
    const { result } = renderHook(() => useChampSelect('puuid-1'));

    act(() => handler({ payload: makeSession('pick') }));

    await waitFor(() => expect(result.current.recommendations).toHaveLength(1));
    expect(mockInvoke).toHaveBeenCalledWith(
      'get_recommendations',
      expect.objectContaining({ puuid: 'puuid-1' }),
    );
    expect(result.current.session?.phase).toBe('pick');
    expect(result.current.isActive).toBe(true);
    expect(result.current.error).toBeNull();
  });

  it('surfaces a friendly error when get_recommendations rejects', async () => {
    mockInvoke.mockRejectedValueOnce('Geçersiz session JSON');
    const { result } = renderHook(() => useChampSelect('puuid-1'));

    act(() => handler({ payload: makeSession('pick') }));

    await waitFor(() =>
      expect(result.current.error).toContain('Öneri alınamadı'),
    );
    expect(result.current.error).toContain('Geçersiz session JSON');
  });

  it('clears the session when a null payload arrives', async () => {
    mockInvoke.mockResolvedValue([]);
    const { result } = renderHook(() => useChampSelect('puuid-1'));

    act(() => handler({ payload: makeSession('pick') }));
    await waitFor(() => expect(result.current.session).not.toBeNull());

    act(() => handler({ payload: null }));
    expect(result.current.session).toBeNull();
    expect(result.current.isActive).toBe(false);
  });
});
