import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { invoke } from '../lib/host';
import { useLcuStatus } from './useLcuStatus';

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

describe('useLcuStatus', () => {
  beforeEach(() => mockInvoke.mockReset());

  it('starts in connecting state before the command resolves', () => {
    mockInvoke.mockResolvedValue({ connected: true });
    // Read synchronously, before the awaited invoke microtask flushes.
    const { result } = renderHook(() => useLcuStatus());
    expect(result.current.status.kind).toBe('connecting');
  });

  it('transitions to lobby when connect_lcu reports connected', async () => {
    mockInvoke.mockResolvedValueOnce({
      connected: true,
      summoner_name: 'Faker',
      port: 51234,
    });
    const { result } = renderHook(() => useLcuStatus());
    await waitFor(() => expect(result.current.status.kind).toBe('lobby'));
    expect(result.current.status).toMatchObject({
      kind: 'lobby',
      summonerName: 'Faker',
      port: 51234,
    });
  });

  it('falls back to defaults when name/port are missing', async () => {
    mockInvoke.mockResolvedValueOnce({ connected: true });
    const { result } = renderHook(() => useLcuStatus());
    await waitFor(() => expect(result.current.status.kind).toBe('lobby'));
    expect(result.current.status).toMatchObject({ summonerName: 'Summoner', port: 0 });
  });

  it('transitions to disconnected with the reported error', async () => {
    mockInvoke.mockResolvedValueOnce({ connected: false, error: 'lockfile yok' });
    const { result } = renderHook(() => useLcuStatus());
    await waitFor(() => expect(result.current.status.kind).toBe('disconnected'));
    expect(result.current.status).toMatchObject({ error: 'lockfile yok' });
  });

  it('transitions to disconnected when the command throws', async () => {
    mockInvoke.mockRejectedValueOnce('boom');
    const { result } = renderHook(() => useLcuStatus());
    await waitFor(() => expect(result.current.status.kind).toBe('disconnected'));
    expect(result.current.status).toMatchObject({ error: 'boom' });
  });

  it('retry re-invokes connect_lcu and recovers', async () => {
    mockInvoke.mockResolvedValueOnce({ connected: false, error: 'kapalı' });
    const { result } = renderHook(() => useLcuStatus());
    await waitFor(() => expect(result.current.status.kind).toBe('disconnected'));

    mockInvoke.mockResolvedValueOnce({ connected: true, summoner_name: 'Ann', port: 1 });
    await act(async () => {
      result.current.retry();
    });
    await waitFor(() => expect(result.current.status.kind).toBe('lobby'));
    expect(mockInvoke).toHaveBeenCalledTimes(2);
  });
});
