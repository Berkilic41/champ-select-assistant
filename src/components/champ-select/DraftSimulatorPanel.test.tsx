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

  it('shows the why_this_move rationale for the pick', () => {
    render(<DraftSimulatorPanel results={[result]} />);
    expect(screen.getByText('Neden bu?')).toBeInTheDocument();
    expect(screen.getByText(/Engage tarafına somut katkı/)).toBeInTheDocument();
  });

  it('appends the signed numeric delta to factor chips when present', () => {
    const withDeltas: DraftSimResult = {
      ...result,
      deltas: [
        { factor: 'engage', before: 0.4, after: 0.57, delta: 0.17 },
        { factor: 'damage_balance', before: 0.6, after: 0.55, delta: -0.05 },
      ],
    };
    render(<DraftSimulatorPanel results={[withDeltas]} />);
    expect(screen.getByText('Engage +0.17')).toBeInTheDocument();
    expect(screen.getByText('Hasar dengesi -0.05')).toBeInTheDocument();
    // 'frontline' has no delta entry → bare label, no suffix (backward compatible).
    expect(screen.getByText('Ön saf')).toBeInTheDocument();
  });

  it('hides the why_this_move line when it is empty', () => {
    render(<DraftSimulatorPanel results={[{ ...result, why_this_move: '' }]} />);
    expect(screen.queryByText('Neden bu?')).not.toBeInTheDocument();
  });
});
