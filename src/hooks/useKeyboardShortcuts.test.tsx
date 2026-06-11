import { describe, it, expect, vi } from 'vitest';
import { render } from '@testing-library/react';
import { useKeyboardShortcuts } from './useKeyboardShortcuts';
import type { Recommendation } from '../types/recommendation';

function rec(id: number): Recommendation {
  return { champion_id: id } as unknown as Recommendation;
}

const recs = [rec(11), rec(22), rec(33)];

function Harness(props: {
  isActive: boolean;
  onHover: (id: number) => void;
  activeIndex?: number;
  onSelect?: (i: number) => void;
}) {
  useKeyboardShortcuts(recs, props.isActive, props.onHover, props.activeIndex ?? 0, props.onSelect);
  return null;
}

function press(key: string) {
  window.dispatchEvent(new KeyboardEvent('keydown', { key }));
}

describe('useKeyboardShortcuts', () => {
  it('1-5 switches the active pick via onSelect (the fix)', () => {
    const onHover = vi.fn();
    const onSelect = vi.fn();
    render(<Harness isActive onHover={onHover} onSelect={onSelect} />);
    press('2');
    expect(onSelect).toHaveBeenCalledWith(1);
    expect(onHover).not.toHaveBeenCalled();
  });

  it('Enter applies a hover on the active pick', () => {
    const onHover = vi.fn();
    render(<Harness isActive onHover={onHover} activeIndex={2} onSelect={vi.fn()} />);
    press('Enter');
    expect(onHover).toHaveBeenCalledWith(33);
  });

  it('without onSelect, 1-5 fall back to applying a hover (ban phase)', () => {
    const onHover = vi.fn();
    render(<Harness isActive onHover={onHover} />);
    press('1');
    expect(onHover).toHaveBeenCalledWith(11);
  });

  it('does nothing when inactive', () => {
    const onSelect = vi.fn();
    render(<Harness isActive={false} onHover={vi.fn()} onSelect={onSelect} />);
    press('3');
    expect(onSelect).not.toHaveBeenCalled();
  });
});
