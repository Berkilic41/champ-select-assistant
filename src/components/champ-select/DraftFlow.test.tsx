import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { DraftFlow } from './DraftFlow';
import type { ChampSelectSession, TeamSlot } from '../../types/recommendation';

const slot = (cell: number, champ: number, intent: number, locked: boolean): TeamSlot => ({
  cell_id: cell,
  champion_id: champ,
  intent_champion_id: intent,
  assigned_position: 'middle',
  is_locked: locked,
});

const session: ChampSelectSession = {
  my_cell_id: 0,
  local_player: slot(0, 238, 0, true),
  my_team: [slot(0, 238, 0, true), slot(1, 0, 99, false), slot(2, 0, 0, false)],
  their_team: [slot(5, 103, 0, true), slot(6, 0, 0, false)],
  my_bans: [],
  their_bans: [],
  phase: 'BAN_PICK',
  time_left_ms: 30000,
  action_type: 'pick',
  queue_id: 420,
  pick_order: 7,
};

describe('DraftFlow', () => {
  it('shows the title and the local pick order', () => {
    render(<DraftFlow session={session} />);
    expect(screen.getByText('Draft Akışı')).toBeInTheDocument();
    expect(screen.getByText(/7\/10/)).toBeInTheDocument();
  });

  it('renders a dot per slot with correct states', () => {
    const { container } = render(<DraftFlow session={session} />);
    expect(container.querySelectorAll('.draft-flow__dot').length).toBe(5);
    expect(container.querySelectorAll('.draft-flow__dot--locked').length).toBe(2);
    expect(container.querySelectorAll('.draft-flow__dot--hover').length).toBe(1);
  });
});
