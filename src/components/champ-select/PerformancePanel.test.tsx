import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { PerformancePanel } from './PerformancePanel';
import type { PerformanceReport } from '../../types/generated/PerformanceReport';

const report: PerformanceReport = {
  games: 12,
  wins: 7,
  losses: 5,
  win_rate: 7 / 12,
  avg_kda: 3.2,
  avg_cs_per_min: 7.5,
  avg_cs_at_10: 68,
  avg_deaths_pre_14: 1.5,
  avg_vision_score: 21,
  loss_streak: 3,
  form_delta: 0.2,
  main_role: 'middle',
  off_role_rate: 0.1,
  top_champions: [{
    champion_id: 238,
    champion_key: 'Zed',
    games: 5,
    wins: 3,
    win_rate: 0.6,
    avg_kda: 2.8,
  }],
  main_lesson: 'Kısa ara ver; son maçlarda karar kalitesi düşüyor.',
  tilt_warning: true,
  partial: false,
};

describe('PerformancePanel', () => {
  it('renders the main post-game lesson and top champion', () => {
    render(<PerformancePanel report={report} />);
    expect(screen.getByText('Son maçlardan ana ders')).toBeInTheDocument();
    expect(screen.getByText(report.main_lesson)).toBeInTheDocument();
    expect(screen.getByText('Zed')).toBeInTheDocument();
    expect(screen.getByText('tilt riski')).toBeInTheDocument();
    expect(screen.getByText('Orta')).toBeInTheDocument();
    expect(screen.getByText('7.5 CS/dk')).toBeInTheDocument();
    // Timeline-türevi metrikler (WS4) — alan doluysa görünür.
    expect(screen.getByText('68 CS@10')).toBeInTheDocument();
    expect(screen.getByText('1.5 erken ölüm/maç')).toBeInTheDocument();
    expect(screen.getByText('21 vizyon')).toBeInTheDocument();
  });

  it('renders nothing without a report', () => {
    const { container } = render(<PerformancePanel report={null} />);
    expect(container.firstChild).toBeNull();
  });
});
