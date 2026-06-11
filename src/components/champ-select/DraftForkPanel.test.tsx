import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { DraftForkPanel } from './DraftForkPanel';
import type { DraftFork } from '../../types/generated/DraftFork';
import type { DraftSimResult } from '../../types/generated/DraftSimResult';

function simResult(
  champion_id: number,
  champion_key: string,
  score_delta: number,
  risk: string,
): DraftSimResult {
  return {
    champion_id,
    champion_key,
    score_delta,
    improved_factors: ['engage', 'frontline'],
    worsened_factors: [],
    deltas: [],
    risk: {
      level: risk,
      summary: `Risk ${risk}`,
      factors: [],
    },
    plan_shift: {
      before: 'dengeli',
      after: 'frontline/teamfight',
      note: 'Plan frontline/teamfight eksenine kayıyor.',
    },
    coach_sentence: `${champion_key}: engage yönünde katkı sağlıyor.`,
    why_this_move: 'Engage tarafına somut katkı veriyor.',
    why_not_alternative: 'Alternatif için belirgin bir dezavantaj sinyali yok.',
  };
}

const fork: DraftFork = {
  option_a: simResult(89, 'Leona', 0.31, 'low'),
  option_b: simResult(111, 'Nautilus', 0.22, 'medium'),
  plan_divergence: 'İki pick de planı frontline/teamfight eksenine çekiyor.',
  risk_divergence: 'Leona riski low, Nautilus riski medium.',
  shared_factors: ['engage'],
  diverging_factors: ['frontline'],
  recommendation: 'Leona kompozisyona biraz daha çok katkı veriyor; ama Nautilus risklerini tartmadan kilitleme.',
};

describe('DraftForkPanel', () => {
  it('renders nothing without a fork', () => {
    const { container } = render(<DraftForkPanel fork={null} />);
    expect(container.firstChild).toBeNull();
  });

  it('shows two-option fork read with shared and diverging factors', () => {
    render(<DraftForkPanel fork={fork} />);

    expect(screen.getByText('İki pick arasında karar')).toBeInTheDocument();
    expect(screen.getByText('Leona')).toBeInTheDocument();
    expect(screen.getByText('Nautilus')).toBeInTheDocument();
    expect(screen.getByText('+0.31 · risk düşük')).toBeInTheDocument();
    expect(screen.getByText('+0.22 · risk orta')).toBeInTheDocument();
    expect(screen.getByText('İkisi de güçlendirir')).toBeInTheDocument();
    expect(screen.getByText('Ayrışan faktörler')).toBeInTheDocument();
    expect(screen.getByText(/kompozisyona biraz daha çok katkı/)).toBeInTheDocument();
  });
});
