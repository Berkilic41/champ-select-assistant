import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useToast } from './useToast';

describe('useToast', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('adds a toast with a unique id and the given type', () => {
    const { result } = renderHook(() => useToast());
    act(() => result.current.addToast('hello', 'success'));
    expect(result.current.toasts).toHaveLength(1);
    expect(result.current.toasts[0].message).toBe('hello');
    expect(result.current.toasts[0].type).toBe('success');

    act(() => result.current.addToast('world', 'error'));
    expect(result.current.toasts).toHaveLength(2);
    // ids must be distinct
    expect(result.current.toasts[0].id).not.toBe(result.current.toasts[1].id);
  });

  it('auto-dismisses after the duration', () => {
    const { result } = renderHook(() => useToast());
    act(() => result.current.addToast('bye', 'info', 1000));
    expect(result.current.toasts).toHaveLength(1);
    act(() => vi.advanceTimersByTime(1000));
    expect(result.current.toasts).toHaveLength(0);
  });

  it('removeToast removes the matching toast immediately', () => {
    const { result } = renderHook(() => useToast());
    act(() => result.current.addToast('x', 'info', 99999));
    const id = result.current.toasts[0].id;
    act(() => result.current.removeToast(id));
    expect(result.current.toasts).toHaveLength(0);
  });

  it('clears pending auto-dismiss timers on unmount (no leak / late state update)', () => {
    const clearSpy = vi.spyOn(globalThis, 'clearTimeout');
    const { result, unmount } = renderHook(() => useToast());
    act(() => result.current.addToast('leak', 'info', 5000));
    const before = clearSpy.mock.calls.length;
    unmount();
    expect(clearSpy.mock.calls.length).toBeGreaterThan(before);
    clearSpy.mockRestore();
  });
});
