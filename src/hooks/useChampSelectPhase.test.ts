import { describe, it, expect } from 'vitest';
import { detectPhaseView } from './useChampSelectPhase';
import type { ChampSelectSession } from '../types/recommendation';

function makeSession(overrides: Partial<ChampSelectSession> = {}): ChampSelectSession {
  return {
    my_cell_id: 2,
    local_player: {
      cell_id: 2,
      champion_id: 0,
      intent_champion_id: 0,
      assigned_position: 'middle',
      is_locked: false,
    },
    my_team: [],
    their_team: [],
    my_bans: [],
    their_bans: [],
    phase: 'BAN_PICK',
    time_left_ms: 30000,
    action_type: '',
    queue_id: 0,
    pick_order: 0,
    ...overrides,
  };
}

describe('detectPhaseView', () => {
  it('null session → planning', () => {
    expect(detectPhaseView(null)).toBe('planning');
  });

  it('PLANNING phase → planning', () => {
    expect(detectPhaseView(makeSession({ phase: 'PLANNING' }))).toBe('planning');
  });

  it('FINALIZATION → finalization', () => {
    expect(detectPhaseView(makeSession({ phase: 'FINALIZATION' }))).toBe('finalization');
  });

  it('action_type ban → ban_acting', () => {
    expect(detectPhaseView(makeSession({ action_type: 'ban' }))).toBe('ban_acting');
  });

  it('action_type pick → pick_acting', () => {
    expect(detectPhaseView(makeSession({ action_type: 'pick' }))).toBe('pick_acting');
  });

  it('locked local player → pick_watching', () => {
    const s = makeSession({
      local_player: {
        cell_id: 2,
        champion_id: 238,
        intent_champion_id: 238,
        assigned_position: 'middle',
        is_locked: true,
      },
    });
    expect(detectPhaseView(s)).toBe('pick_watching');
  });

  it('no action, <10 bans → ban_watching', () => {
    expect(detectPhaseView(makeSession({ my_bans: [1], their_bans: [2] }))).toBe('ban_watching');
  });

  it('no action, ≥10 bans → pick_watching', () => {
    expect(
      detectPhaseView(
        makeSession({
          my_bans: [1, 2, 3, 4, 5],
          their_bans: [6, 7, 8, 9, 10],
        }),
      ),
    ).toBe('pick_watching');
  });
});
