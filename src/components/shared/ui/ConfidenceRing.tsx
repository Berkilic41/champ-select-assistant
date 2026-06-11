import React from 'react';
import './ui.css';

interface ConfidenceRingProps {
  /** 0..1 — shown as a percent in the center. */
  score: number;
  /** "high" | "medium" | "low" — drives the ring fill + color. */
  confidence: string;
  size?: number;
  /** Accessible label, e.g. "Skor %72, güven yüksek". */
  ariaLabel?: string;
}

function clamp01(v: number): number {
  return Math.max(0, Math.min(1, v));
}

function level(confidence: string): { fill: number; tone: string } {
  if (confidence === 'high') return { fill: 1, tone: 'var(--teal)' };
  if (confidence === 'medium') return { fill: 0.66, tone: 'var(--gold)' };
  return { fill: 0.33, tone: 'var(--warning)' };
}

/** The strong confidence signal — an SVG ring whose fill encodes confidence
 *  (high=full teal, medium=2/3 gold, low=1/3 amber) wrapping the score %. */
export const ConfidenceRing: React.FC<ConfidenceRingProps> = ({
  score,
  confidence,
  size = 64,
  ariaLabel,
}) => {
  const pct = Math.round(clamp01(score) * 100);
  const { fill, tone } = level(confidence);
  const stroke = Math.max(4, Math.round(size * 0.09));
  const r = (size - stroke) / 2;
  const circ = 2 * Math.PI * r;
  const offset = circ * (1 - fill);
  const c = size / 2;

  return (
    <div
      className="ui-ring"
      style={{ width: size, height: size }}
      role="img"
      aria-label={ariaLabel ?? `${pct}%`}
    >
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
        <circle className="ui-ring-track" cx={c} cy={c} r={r} strokeWidth={stroke} fill="none" />
        <circle
          className="ui-ring-arc"
          cx={c}
          cy={c}
          r={r}
          strokeWidth={stroke}
          fill="none"
          style={{ stroke: tone, strokeDasharray: circ, strokeDashoffset: offset }}
          transform={`rotate(-90 ${c} ${c})`}
        />
      </svg>
      <span className="ui-ring-pct" style={{ fontSize: Math.round(size * 0.28) }}>
        {pct}
        <span className="ui-ring-pct-sign">%</span>
      </span>
    </div>
  );
};
