import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { invoke } from '../../lib/host';
import { IngameView, PowerCurveBar, gameClock } from './IngameView';
import { DEFAULT_SETTINGS } from '../../hooks/useSettings';

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

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

const PLAN = {
  champion_key: 'Garen',
  champion_name: 'Garen',
  position: 'top',
  opponent_name: 'Darius',
  opponent_key: 'Darius',
  level: 6,
  kills: 3,
  deaths: 1,
  assists: 2,
  cs: 80,
  cs_per_min: 6.5,
  power_early: 0.7,
  power_mid: 0.6,
  power_late: 0.4,
  win_condition: 'Erken tempo kur ve kuleyi al.',
  team_role: 'Frontline',
  damage_profile: 'AD',
  spike_note: '',
  spike_window: '',
  lane_note: '',
  wave_note: '',
  matchup_tips: [],
  mid_plan: '',
  late_plan: '',
};
const MACRO = {
  live: true,
  state: {
    phase: 'mid',
    phase_note: 'Objektiflere bas.',
    objectives: [{ objective: 'dragon', state: 'pending', seconds_until: 120, next_spawn_secs: 300 }],
    reminders: ['Ward al'],
  },
};

describe('IngameView compact HUD toggle (Overlay HUD Epic)', () => {
  beforeEach(() => mockInvoke.mockReset());

  it('toggles between the detailed plan text and a compact glanceable HUD', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve(DEFAULT_SETTINGS);
      if (cmd === 'get_ingame_plan') return Promise.resolve(PLAN);
      if (cmd === 'get_macro_state') return Promise.resolve(MACRO);
      return Promise.resolve(undefined);
    });
    const { container } = render(<IngameView />);

    // Detaylı (varsayılan): yoğun plan metni görünür; görseller (güç eğrisi) de var.
    expect(await screen.findByText('Erken tempo kur ve kuleyi al.')).toBeInTheDocument();
    expect(container.querySelector('.overlay-power')).toBeInTheDocument();

    // "Kompakt"a geç → yoğun plan metni gizlenir, glanceable görseller KALIR.
    fireEvent.click(screen.getByRole('button', { name: 'Kompakt' }));
    expect(screen.queryByText('Erken tempo kur ve kuleyi al.')).not.toBeInTheDocument();
    expect(container.querySelector('.overlay-power')).toBeInTheDocument();

    // "Detaylı"ya geri dön → metin yeniden görünür.
    fireEvent.click(screen.getByRole('button', { name: 'Detaylı' }));
    expect(screen.getByText('Erken tempo kur ve kuleyi al.')).toBeInTheDocument();
  });
});
