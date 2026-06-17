import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { PowerCurveBar, gameClock } from './IngameView';

describe('PowerCurveBar', () => {
  it('renders three phase columns with labels', () => {
    const { container } = render(<PowerCurveBar early={0.3} mid={0.9} late={0.6} />);
    expect(container.querySelectorAll('.overlay-power-col')).toHaveLength(3);
    expect(container.textContent).toContain('Erken');
    expect(container.textContent).toContain('Orta');
    expect(container.textContent).toContain('Geç');
  });

  it('highlights only the peak phase', () => {
    const { container } = render(<PowerCurveBar early={0.3} mid={0.9} late={0.6} />);
    const peaks = container.querySelectorAll('.overlay-power-fill--peak');
    expect(peaks).toHaveLength(1);
    expect((peaks[0] as HTMLElement).style.height).toBe('90%');
  });

  it('sets fill heights from the 0..1 values, clamped and rounded', () => {
    const { container } = render(<PowerCurveBar early={0.25} mid={1.4} late={-0.2} />);
    const fills = container.querySelectorAll('.overlay-power-fill');
    expect((fills[0] as HTMLElement).style.height).toBe('25%');
    expect((fills[1] as HTMLElement).style.height).toBe('100%'); // 1.4 → clamp 1.0
    expect((fills[2] as HTMLElement).style.height).toBe('0%'); // -0.2 → clamp 0
  });

  it('exposes one role=img summary with percentages; columns are decorative', () => {
    const { container } = render(<PowerCurveBar early={0.3} mid={0.9} late={0.6} />);
    const bar = container.querySelector('.overlay-power');
    expect(bar).toHaveAttribute('role', 'img');
    const aria = bar?.getAttribute('aria-label') ?? '';
    expect(aria).toContain('30');
    expect(aria).toContain('90');
    expect(aria).toContain('60');
    // SR reads the single summary, not each bar.
    expect(container.querySelectorAll('.overlay-power-col[aria-hidden="true"]')).toHaveLength(3);
  });

  it('marks the live phase column with a "you are here" indicator', () => {
    const { container } = render(
      <PowerCurveBar early={0.3} mid={0.9} late={0.6} currentPhase="mid" />,
    );
    const current = container.querySelectorAll('.overlay-power-col--current');
    expect(current).toHaveLength(1);
    expect(current[0].textContent).toContain('Orta');
    expect(container.querySelectorAll('.overlay-power-now')).toHaveLength(1);
    const aria = container.querySelector('.overlay-power')?.getAttribute('aria-label') ?? '';
    expect(aria).toContain('şu an');
  });

  it('renders no live-phase marker without currentPhase', () => {
    const { container } = render(<PowerCurveBar early={0.3} mid={0.9} late={0.6} />);
    expect(container.querySelectorAll('.overlay-power-col--current')).toHaveLength(0);
    expect(container.querySelectorAll('.overlay-power-now')).toHaveLength(0);
    const aria = container.querySelector('.overlay-power')?.getAttribute('aria-label') ?? '';
    expect(aria).not.toContain('şu an');
  });

  it('ignores an unknown phase value (defensive)', () => {
    const { container } = render(
      <PowerCurveBar early={0.3} mid={0.9} late={0.6} currentPhase="garbage" />,
    );
    expect(container.querySelectorAll('.overlay-power-col--current')).toHaveLength(0);
    expect(container.querySelectorAll('.overlay-power-now')).toHaveLength(0);
    const aria = container.querySelector('.overlay-power')?.getAttribute('aria-label') ?? '';
    expect(aria).not.toContain('şu an');
  });
});

describe('gameClock (objective absolute spawn time — Epic #5)', () => {
  it('formats seconds as mm:ss game-clock with zero-padded seconds', () => {
    expect(gameClock(1440)).toBe('24:00'); // Baron @ 24:00
    expect(gameClock(65)).toBe('1:05');
    expect(gameClock(0)).toBe('0:00');
  });

  it('clamps negative values to 0:00 (defensive)', () => {
    expect(gameClock(-5)).toBe('0:00');
  });
});
