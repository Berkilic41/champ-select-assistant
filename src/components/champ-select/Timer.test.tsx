import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import { Timer } from './Timer';

describe('Timer', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('shows the initial seconds (ceil) for the given ms', () => {
    render(<Timer timeLeftMs={30000} phase="BAN_PICK" isActing />);
    expect(screen.getByText('30')).toBeInTheDocument();
  });

  it('counts down once per second', () => {
    render(<Timer timeLeftMs={30000} phase="BAN_PICK" isActing />);
    act(() => { vi.advanceTimersByTime(1000); });
    expect(screen.getByText('29')).toBeInTheDocument();
    act(() => { vi.advanceTimersByTime(2000); });
    expect(screen.getByText('27')).toBeInTheDocument();
  });

  it('never goes below zero', () => {
    render(<Timer timeLeftMs={2000} phase="BAN_PICK" isActing />);
    act(() => { vi.advanceTimersByTime(10000); });
    expect(screen.getByText('0')).toBeInTheDocument();
  });

  it('labels BAN_PICK as SIRANIZ when acting and BEKLIYORSUNUZ otherwise', () => {
    const { rerender } = render(<Timer timeLeftMs={30000} phase="BAN_PICK" isActing />);
    expect(screen.getByText('SIRANIZ')).toBeInTheDocument();
    rerender(<Timer timeLeftMs={30000} phase="BAN_PICK" isActing={false} />);
    expect(screen.getByText('BEKLIYORSUNUZ')).toBeInTheDocument();
  });

  it('maps PLANNING/FINALIZATION phases to their labels', () => {
    const { rerender } = render(<Timer timeLeftMs={60000} phase="PLANNING" />);
    expect(screen.getByText('LOBI')).toBeInTheDocument();
    rerender(<Timer timeLeftMs={30000} phase="FINALIZATION" />);
    expect(screen.getByText('KİLİTLENİYOR')).toBeInTheDocument();
  });

  it('adds the urgency animation class only when critical and acting', () => {
    const { container, rerender } = render(
      <Timer timeLeftMs={8000} phase="BAN_PICK" isActing />,
    );
    expect(container.querySelector('.cs-timer')?.className).toContain('animate-urgency');
    // critical seconds but not acting → no animation
    rerender(<Timer timeLeftMs={8000} phase="BAN_PICK" isActing={false} />);
    expect(container.querySelector('.cs-timer')?.className).not.toContain('animate-urgency');
  });

  it('exposes the countdown to screen readers via role=img + aria-label', () => {
    const { container } = render(<Timer timeLeftMs={25000} phase="BAN_PICK" isActing />);
    const svg = container.querySelector('svg');
    expect(svg).toHaveAttribute('role', 'img');
    expect(svg?.getAttribute('aria-label')).toMatch(/25/);
  });
});
