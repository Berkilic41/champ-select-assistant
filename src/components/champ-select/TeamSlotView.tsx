import React from 'react';
import { TeamSlot } from '../../types/recommendation';
import { ChampionIcon } from '../shared/ChampionIcon';
import './TeamSlotView.css';

interface Props {
  slot: TeamSlot;
  champMap: Map<number, string>;
  isLocalPlayer?: boolean;
}

const POSITION_LABELS: Record<string, string> = {
  top: 'TOP',
  jungle: 'JG',
  middle: 'MID',
  bottom: 'BOT',
  utility: 'SUP',
  '': '?',
};

export const TeamSlotView: React.FC<Props> = ({ slot, champMap, isLocalPlayer }) => {
  const champId = slot.champion_id || slot.intent_champion_id;
  const champKey = champId ? champMap.get(champId) : undefined;
  return (
    <div
      className={[
        'team-slot',
        isLocalPlayer ? 'team-slot--me' : '',
        slot.is_locked ? 'team-slot--locked' : '',
      ]
        .filter(Boolean)
        .join(' ')}
    >
      <span className="team-slot__pos">
        {POSITION_LABELS[slot.assigned_position] ?? '?'}
      </span>
      <ChampionIcon championKey={champKey} size="sm" />
    </div>
  );
};
