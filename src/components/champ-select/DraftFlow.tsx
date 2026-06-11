import React from 'react';
import { useTranslation } from 'react-i18next';
import type { ChampSelectSession, TeamSlot } from '../../types/recommendation';
import './DraftFlow.css';

interface Props {
  session: ChampSelectSession;
}

function dotState(s: TeamSlot): 'locked' | 'hover' | 'empty' {
  if (s.is_locked || s.champion_id > 0) return 'locked';
  if (s.intent_champion_id > 0) return 'hover';
  return 'empty';
}

const Dots: React.FC<{ slots: TeamSlot[]; localCell?: number }> = ({ slots, localCell }) => (
  <div className="draft-flow__dots">
    {slots.map((s) => (
      <span
        key={s.cell_id}
        className={`draft-flow__dot draft-flow__dot--${dotState(s)}${
          localCell !== undefined && s.cell_id === localCell ? ' draft-flow__dot--local' : ''
        }`}
      />
    ))}
  </div>
);

/**
 * Compact draft-progress strip: your global pick position + per-team locked /
 * hovering / empty dots. An at-a-glance "where are we in the draft" view that
 * complements (not duplicates) the detailed team columns.
 */
export const DraftFlow: React.FC<Props> = ({ session }) => {
  const { t } = useTranslation();

  return (
    <div className="draft-flow">
      <span className="draft-flow__title">{t('draftFlow.title')}</span>
      {session.pick_order > 0 && (
        <span className="draft-flow__order">{t('draftFlow.yourPick', { n: session.pick_order })}</span>
      )}
      <div className="draft-flow__teams">
        <div className="draft-flow__team">
          <span className="draft-flow__team-label">{t('champSelect.yourTeam')}</span>
          <Dots slots={session.my_team} localCell={session.my_cell_id} />
        </div>
        <div className="draft-flow__team">
          <span className="draft-flow__team-label">{t('champSelect.enemyTeam')}</span>
          <Dots slots={session.their_team} />
        </div>
      </div>
    </div>
  );
};
