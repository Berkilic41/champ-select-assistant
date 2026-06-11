import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { DraftSimulatorPanel } from './DraftSimulatorPanel';
import type { DraftSimResult } from '../../types/generated/DraftSimResult';

const result: DraftSimResult = {
  champion_id: 89,
  champion_key: 'Leona',
  score_delta: 0.42,
  improved_factors: ['engage', 'frontline', 'synergy'],
  worsened_factors: ['damage_balance'],
  deltas: [],
  risk: {
    level: 'medium',
    summary: 'Dikkat: hasar dengesi.',
    factors: ['damage_balance'],
  },
  plan_shift: {
    before: 'dengeli',
    after: 'frontline/teamfight',
    note: 'Plan dengeli ekseninden frontline/teamfight eksenine kayıyor.',
  },
  coach_sentence: 'Leona: engage güçleniyor, hasar dengesi zayıflıyor.',
  why_this_move: 'Engage tarafına somut katkı veriyor.',
  why_not_alternative: 'Alternatif düşün: hasar dengesi zayıflıyor (medium).',
};

describe('DraftSimulatorPanel', () => {
  it('renders nothing without simulation results', () => {
    const { container } = render(<DraftSimulatorPanel results={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it('shows local simulation read with mapped factor labels', () => {
    render(<DraftSimulatorPanel results={[result]} />);

    expect(screen.getByText('Draft simülatörü')).toBeInTheDocument();
    expect(screen.getByText('Leona')).toBeInTheDocument();
    expect(screen.getByText('Delta +0.42')).toBeInTheDocument();
    expect(screen.getByText('Risk orta')).toBeInTheDocument();
    expect(screen.getByText('Engage')).toBeInTheDocument();
    expect(screen.getByText('Ön saf')).toBeInTheDocument();
    expect(screen.getByText('Hasar dengesi')).toBeInTheDocument();
    expect(screen.getByText(/Alternatif düşün/)).toBeInTheDocument();
  });
});
