import React from 'react';
import { useTranslation } from 'react-i18next';
import { Timer } from './Timer';
import { PhaseView } from '../../hooks/useChampSelectPhase';
import './PhaseHeader.css';

interface Props {
  timeLeftMs: number;
  lolPhase: string;
  view: PhaseView;
  isActing: boolean;
}

export const PhaseHeader: React.FC<Props> = ({ timeLeftMs, lolPhase, view, isActing }) => {
  const { t } = useTranslation();
  return (
    <div className={`phase-header phase-header--${isActing ? 'acting' : 'watching'}`}>
      <span className="phase-header__msg">{t(`champSelect.phase.${view}`)}</span>
      <Timer timeLeftMs={timeLeftMs} phase={lolPhase} isActing={isActing} />
    </div>
  );
};
