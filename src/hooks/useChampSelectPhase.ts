import { ChampSelectSession } from '../types/recommendation';

export type PhaseView =
  | 'planning'
  | 'ban_acting'
  | 'ban_watching'
  | 'pick_acting'
  | 'pick_watching'
  | 'finalization';

export function detectPhaseView(session: ChampSelectSession | null): PhaseView {
  if (!session) return 'planning';
  const { phase, local_player, action_type } = session;

  if (phase === 'PLANNING') return 'planning';
  if (phase === 'FINALIZATION') return 'finalization';

  // BAN_PICK: use the action_type field to determine exact state.
  if (action_type === 'ban') return 'ban_acting';
  if (action_type === 'pick') return 'pick_acting';

  // No active action for local player — determine watching state via heuristic.
  if (local_player.is_locked) return 'pick_watching';
  const totalBans = (session.my_bans?.length ?? 0) + (session.their_bans?.length ?? 0);
  return totalBans < 10 ? 'ban_watching' : 'pick_watching';
}
