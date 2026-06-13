import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import './Timer.css';

interface Props {
  timeLeftMs: number;
  phase: string;
  isActing?: boolean;
}

const TOTAL_SECONDS: Record<string, number> = { BAN_PICK: 30, PLANNING: 60, FINALIZATION: 30 };
const RADIUS = 22;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

export const Timer: React.FC<Props> = ({ timeLeftMs, phase, isActing }) => {
  const { t } = useTranslation();
  const [timeLeft, setTimeLeft] = useState(timeLeftMs);

  useEffect(() => { setTimeLeft(timeLeftMs); }, [timeLeftMs]);

  // One ticking interval for the component's lifetime — the effect above re-syncs
  // timeLeft whenever the backend pushes a new timeLeftMs. Keying this on
  // timeLeftMs would tear down + recreate the interval on every session update.
  useEffect(() => {
    const id = setInterval(() => setTimeLeft(t => Math.max(0, t - 1000)), 1000);
    return () => clearInterval(id);
  }, []);

  const seconds = Math.ceil(timeLeft / 1000);
  const total = TOTAL_SECONDS[phase] ?? 30;
  const ratio = Math.max(0, Math.min(1, seconds / total));
  const dashOffset = CIRCUMFERENCE * (1 - ratio);

  const urgency = seconds <= 10 ? 'critical' : seconds <= 20 ? 'warning' : 'ok';
  const colors = { ok: '#4CAF50', warning: '#F5A623', critical: '#F44336' };

  const phaseLabel: Record<string, string> = {
    BAN_PICK: isActing ? t('timer.yourTurn') : t('timer.waiting'),
    PLANNING: t('timer.lobby'),
    FINALIZATION: t('timer.locking'),
  };
  const label = phaseLabel[phase] ?? phase;

  return (
    <div className={`cs-timer ${urgency === 'critical' && isActing ? 'animate-urgency' : ''}`}>
      <svg width="60" height="60" viewBox="0 0 60 60">
        {/* Track */}
        <circle cx="30" cy="30" r={RADIUS} fill="none" stroke="var(--color-border)" strokeWidth="4" />
        {/* Progress */}
        <circle
          cx="30" cy="30" r={RADIUS}
          fill="none"
          stroke={colors[urgency]}
          strokeWidth="4"
          strokeDasharray={CIRCUMFERENCE}
          strokeDashoffset={dashOffset}
          strokeLinecap="round"
          transform="rotate(-90 30 30)"
          style={{ transition: 'stroke-dashoffset 1s linear, stroke 0.5s' }}
        />
        {/* Seconds */}
        <text x="30" y="35" textAnchor="middle" fontSize="14" fontWeight="700" fill={colors[urgency]}>
          {seconds}
        </text>
      </svg>
      <span className="cs-timer__label" style={{ color: colors[urgency] }}>{label}</span>
    </div>
  );
};
