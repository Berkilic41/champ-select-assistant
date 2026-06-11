import React from 'react';
import './ui.css';

interface StatBarProps {
  label: string;
  /** 0..1 — clamped, rendered as a percent. */
  value: number;
}

function clamp01(v: number): number {
  return Math.max(0, Math.min(1, v));
}

/** Labelled score bar — extracted verbatim from HeroCard's `ScoreBar` (same classes
 *  so existing HeroCard styling + tests stay valid), now reusable across the app. */
export const StatBar: React.FC<StatBarProps> = ({ label, value }) => {
  const pct = Math.round(clamp01(value) * 100);
  return (
    <div className="score-bar-row">
      <span className="score-bar-label">{label}</span>
      <div className="score-bar-track">
        <div className="score-bar-fill" style={{ width: `${pct}%` }} />
      </div>
      <span className="score-bar-pct">{pct}%</span>
    </div>
  );
};
