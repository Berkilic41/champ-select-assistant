import { describe, it, expect } from 'vitest';
import type { OverlayMacroState } from './generated/OverlayMacroState';
import type { MacroState } from './generated/MacroState';

// Compile-time contract guard for the in-game overlay command response (Faz 4). A Rust
// field add/remove/retype regenerates this and breaks `pnpm typecheck` here before the
// overlay UI can drift. `live=false` ⇒ no game ⇒ state is null (silent-poll contract).

describe('Overlay macro state contract', () => {
  it('OverlayMacroState wraps live flag + optional MacroState', () => {
    const keys: Record<keyof OverlayMacroState, true> = {
      live: true,
      state: true,
    };
    expect(Object.keys(keys).sort()).toEqual(['live', 'state']);

    const offline: OverlayMacroState = { live: false, state: null };
    expect(offline.state).toBeNull();
    expect(typeof offline.live).toBe('boolean');

    const macro: MacroState = {
      game_time_secs: 1503,
      phase: 'mid',
      objectives: [
        { objective: 'baron', next_spawn_secs: 1500, seconds_until: -3, state: 'up' },
      ],
      reminders: ['Baron hazır — vision kontrolü olmadan başlatma.'],
      phase_note: 'Orta oyun: obje etrafında grupla.',
    };
    const live: OverlayMacroState = { live: true, state: macro };
    expect(live.state?.objectives[0].objective).toBe('baron');
    // No bigint anywhere — times/counters are number.
    expect(typeof live.state?.game_time_secs).toBe('number');
    expect(typeof live.state?.objectives[0].seconds_until).toBe('number');
  });
});
