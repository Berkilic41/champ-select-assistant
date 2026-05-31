import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useKeyboardShortcuts } from './useKeyboardShortcuts';
import type { Recommendation } from '../types/recommendation';

const recs = [
  { champion_id: 11 },
  { champion_id: 22 },
  { champion_id: 33 },
] as Recommendation[];

function press(key: string, mods: Partial<KeyboardEventInit> = {}) {
  act(() => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key, ...mods }));
  });
}

describe('useKeyboardShortcuts', () => {
  let onHover: Mock<(championId: number) => void>;
  beforeEach(() => {
    onHover = vi.fn<(championId: number) => void>();
  });

  it('number keys 1..5 hover the matching recommendation', () => {
    renderHook(() => useKeyboardShortcuts(recs, true, onHover, 0));
    press('1');
    expect(onHover).toHaveBeenCalledWith(11);
    press('3');
    expect(onHover).toHaveBeenCalledWith(33);
  });

  it('Enter hovers the active index', () => {
    renderHook(() => useKeyboardShortcuts(recs, true, onHover, 1));
    press('Enter');
    expect(onHover).toHaveBeenCalledWith(22);
  });

  it('ignores keys combined with ctrl/meta/alt', () => {
    renderHook(() => useKeyboardShortcuts(recs, true, onHover, 0));
    press('1', { ctrlKey: true });
    press('1', { metaKey: true });
    press('1', { altKey: true });
    expect(onHover).not.toHaveBeenCalled();
  });

  it('does nothing for an out-of-range slot or when inactive', () => {
    const { rerender } = renderHook(
      ({ active }) => useKeyboardShortcuts(recs, active, onHover, 0),
      { initialProps: { active: true } },
    );
    press('5'); // only 3 recs → slot 5 empty
    expect(onHover).not.toHaveBeenCalled();

    rerender({ active: false });
    press('1');
    expect(onHover).not.toHaveBeenCalled();
  });
});
