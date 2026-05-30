import React from 'react';
import { Timer } from './Timer';
import { PhaseView } from '../../hooks/useChampSelectPhase';
import './PhaseHeader.css';

const PHASE_MESSAGES: Record<PhaseView, string> = {
  planning: 'Lobi — Kuyruğa hazır ol',
  ban_acting: 'BAN SIRASI — Tehlikeli şampiyonu banla',
  ban_watching: 'Takım banlıyor...',
  pick_acting: 'SENİN SIRAN — Hover önerini seç',
  pick_watching: 'Takım seçiyor...',
  finalization: 'Seçim kilitlendi — Build planına bak',
};

interface Props {
  timeLeftMs: number;
  lolPhase: string;
  view: PhaseView;
  isActing: boolean;
}

export const PhaseHeader: React.FC<Props> = ({ timeLeftMs, lolPhase, view, isActing }) => (
  <div className={`phase-header phase-header--${isActing ? 'acting' : 'watching'}`}>
    <span className="phase-header__msg">{PHASE_MESSAGES[view]}</span>
    <Timer timeLeftMs={timeLeftMs} phase={lolPhase} isActing={isActing} />
  </div>
);
