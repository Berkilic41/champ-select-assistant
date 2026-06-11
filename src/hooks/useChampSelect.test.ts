import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { invoke } from '../lib/host';
import { listen } from '../lib/host';
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
    localStorage.clear(); // role persistence (preferredRole) must not leak across tests
    mockInvoke.mockReset();
    mockListen.mockReset();
    // Default: every command resolves (the hook also fires get_game_plan /
    // get_champion_analysis besides get_draft_brain_recommendations). `mockResolvedValueOnce`
    // in individual tests still takes precedence for the first call.
    mockInvoke.mockResolvedValue(null);
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
      'get_draft_brain_recommendations',
      expect.objectContaining({ puuid: 'puuid-1' }),
    );
    expect(result.current.session?.phase).toBe('pick');
    expect(result.current.isActive).toBe(true);
    expect(result.current.error).toBeNull();
  });

  it('surfaces a friendly error when get_draft_brain_recommendations rejects', async () => {
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

  // ── Role resolution (Faz 8 regression lock) ────────────────────────────────

  it('uses the LCU assigned position as the role', async () => {
    const { result } = renderHook(() => useChampSelect('p'));
    act(() => handler({ payload: makeSession('pick') })); // assignedPosition: 'middle'
    await waitFor(() => expect(result.current.session).not.toBeNull());
    expect(result.current.role).toBe('middle');
    expect(result.current.roleSource).toBe('lcu');
  });

  it('prompts for a role when LCU gives no position (Blind/Normal)', async () => {
    const { result } = renderHook(() => useChampSelect('p'));
    const blind = { ...makeSession('pick'), local_player: { champion_id: 0, assigned_position: '' } };
    act(() => handler({ payload: blind }));
    await waitFor(() => expect(result.current.session).not.toBeNull());
    expect(result.current.role).toBe('');
    expect(result.current.roleSource).toBe('none');
  });

  it('setRole overrides the position, patches the session, and persists it', async () => {
    const { result } = renderHook(() => useChampSelect('p'));
    act(() => handler({ payload: makeSession('pick') }));
    await waitFor(() => expect(result.current.session).not.toBeNull());

    act(() => result.current.setRole('top'));

    expect(result.current.role).toBe('top');
    expect(result.current.roleSource).toBe('manual');
    expect(result.current.session?.local_player.assigned_position).toBe('top');
    expect(localStorage.getItem('preferredRole')).toBe('top');
  });

  it('seeds the role from the persisted preference when LCU is empty', async () => {
    localStorage.setItem('preferredRole', 'bottom');
    const { result } = renderHook(() => useChampSelect('p'));
    const blind = { ...makeSession('pick'), local_player: { champion_id: 0, assigned_position: '' } };
    act(() => handler({ payload: blind }));
    await waitFor(() => expect(result.current.session).not.toBeNull());
    // Persisted default fills in and is applied to the session.
    expect(result.current.role).toBe('bottom');
    expect(result.current.session?.local_player.assigned_position).toBe('bottom');
  });
});
