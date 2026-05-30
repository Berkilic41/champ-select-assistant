import React from 'react';

export type Tier = 's' | 'a' | 'b' | 'c';

export const TIER_COLORS: Record<Tier, string> = {
  s: '#FFD700',   // Altın
  a: '#C89B3C',   // LoL gold
  b: '#7A7060',   // Gri-bronz
  c: '#4A4A4A',   // Soluk gri
};

export const TIER_LABELS: Record<Tier, string> = {
  s: 'S', a: 'A', b: 'B', c: 'C',
};

export function tierFromScore(score: number): Tier {
  if (score >= 0.75) return 's';
  if (score >= 0.65) return 'a';
  if (score >= 0.55) return 'b';
  return 'c';
}

interface TierBadgeProps {
  tier: Tier;
  large?: boolean;
}

export const TierBadge: React.FC<TierBadgeProps> = ({ tier, large }) => {
  return React.createElement(
    'span',
    {
      style: {
        color: TIER_COLORS[tier],
        fontWeight: 800,
        fontSize: large ? '22px' : '13px',
        letterSpacing: '1px',
        border: `2px solid ${TIER_COLORS[tier]}`,
        borderRadius: '4px',
        padding: large ? '2px 8px' : '1px 5px',
        display: 'inline-block',
        lineHeight: 1,
      },
    },
    TIER_LABELS[tier],
  );
};
