import { describe, it, expect } from 'vitest';
import type { MacroState } from './generated/MacroState';
import type { ObjectiveTimer } from './generated/ObjectiveTimer';

// Compile-time contract guard for the in-game macro/objective timer core (overlay,
// Faz 4). A Rust field add/remove/retype regenerates these and breaks `pnpm typecheck`
// here before the overlay runtime can drift. No bigint — times/counters are u32/i32 →
// number. Objective/state/phase are Rust-locked machine tokens.

const timer: ObjectiveTimer = {
  objective: 'dragon',
  next_spawn_secs: 300,
  seconds_until: 180,
  state: 'pending_first',
};

describe('Macro timers contract', () => {
  it('ObjectiveTimer is bigint-free with a locked vocabulary', () => {
    const keys: Record<keyof ObjectiveTimer, true> = {
      objective: true,
      next_spawn_secs: true,
      seconds_until: true,
      state: true,
    };
    expect(Object.keys(keys).sort()).toEqual([
      'next_spawn_secs',
      'objective',
      'seconds_until',
      'state',
    ]);
    expect(typeof timer.next_spawn_secs).toBe('number');
    expect(typeof timer.seconds_until).toBe('number'); // i32 → number, can be negative

    const objectives = ['grubs', 'herald', 'dragon', 'baron'];
    const states = ['pending_first', 'respawning', 'soon', 'up'];
    expect(objectives).toContain(timer.objective);
    expect(states).toContain(timer.state);
  });

  it('MacroState bundles timers + reminders + phase', () => {
    const keys: Record<keyof MacroState, true> = {
      game_time_secs: true,
      phase: true,
      objectives: true,
      reminders: true,
      phase_note: true,
    };
    expect(Object.keys(keys).sort()).toEqual([
      'game_time_secs',
      'objectives',
      'phase',
      'phase_note',
      'reminders',
    ]);

    const state: MacroState = {
      game_time_secs: 1500,
      phase: 'mid',
      objectives: [timer],
      reminders: ['Dragon yakında — nehir görüşü ve pozisyon al.'],
      phase_note: 'Orta oyun: obje etrafında grupla.',
    };
    expect(typeof state.game_time_secs).toBe('number');
    expect(['early', 'mid', 'late']).toContain(state.phase);
    expect(state.objectives[0].objective).toBe('dragon');
  });
});
