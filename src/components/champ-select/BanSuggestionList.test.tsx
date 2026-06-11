import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { BanSuggestionList } from './BanSuggestionList';
import type { BanSuggestion, EnemyPoolSummary } from '../../types/recommendation';
import type { ScoutingReport } from '../../types/generated/ScoutingReport';

const ban = (id: number, name: string, threat: number): BanSuggestion => ({
  champion_id: id,
  champion_key: name,
  champion_name: name,
  threat_score: threat,
  reason: `${name} tehdidi`,
});

const scoutingReport: ScoutingReport = {
  partial: false,
  team_read: {
    win_condition: 'pick',
    high_threat_count: 1,
    primary_threat_key: 'Zed',
    primary_threat_cell_id: 5,
    strategy_note: 'Pick komp: vision basın, izole durmayın, gruplu hareket edin.',
  },
  ban_targets: [{
    champion_id: 238,
    champion_key: 'Zed',
    reason: 'Rakibin ana kozu; banı oyun planını bozar',
    confidence: 'medium',
  }],
  enemies: [{
    cell_id: 5,
    top_champion_id: 238,
    top_champion_key: 'Zed',
    play_rate: 0.7,
    game_count: 12,
    pool_tendency: 'otp',
    playstyle_tags: ['erken agresif', 'snowball'],
    threat: 'yüksek',
    threat_level: 'medium',
    note: 'Zed · tek şampiyon (son 12 maçta %70)',
  }],
};

describe('BanSuggestionList', () => {
  it('shows a computing placeholder when there are no suggestions', () => {
    render(<BanSuggestionList suggestions={[]} />);
    expect(screen.getByText(/hesaplanıyor/)).toBeInTheDocument();
  });

  it('renders names, threat percentages and reasons', () => {
    render(<BanSuggestionList suggestions={[ban(1, 'Zed', 0.82)]} />);
    expect(screen.getByText('Zed')).toBeInTheDocument();
    expect(screen.getByText('82%')).toBeInTheDocument();
    expect(screen.getByText(/Zed tehdidi/)).toBeInTheDocument();
  });

  it('caps the list at three suggestions', () => {
    const five = [1, 2, 3, 4, 5].map((i) => ban(i, `C${i}`, 0.5));
    const { container } = render(<BanSuggestionList suggestions={five} />);
    expect(container.querySelectorAll('.ban-suggestion-row').length).toBe(3);
  });

  it('annotates a suggestion when the enemy is a high-play-rate OTP', () => {
    const pools: EnemyPoolSummary[] = [{
      cell_id: 0,
      top_champion_id: 1,
      top_champion_key: 'Zed',
      play_rate: 0.6,
      game_count: 30,
    }];
    render(<BanSuggestionList suggestions={[ban(1, 'Zed', 0.82)]} enemyPools={pools} />);
    expect(screen.getByText(/rakip OTP \(%60\)/)).toBeInTheDocument();
  });

  it('does not annotate when the enemy play rate is below the OTP threshold', () => {
    const pools: EnemyPoolSummary[] = [{
      cell_id: 0,
      top_champion_id: 1,
      top_champion_key: 'Zed',
      play_rate: 0.2,
      game_count: 30,
    }];
    render(<BanSuggestionList suggestions={[ban(1, 'Zed', 0.82)]} enemyPools={pools} />);
    expect(screen.queryByText(/rakip OTP/)).not.toBeInTheDocument();
  });

  it('renders scouting ban targets and enemy profiles', () => {
    const { container } = render(
      <BanSuggestionList
        suggestions={[ban(1, 'Ahri', 0.7)]}
        scoutingReport={scoutingReport}
      />,
    );

    expect(screen.getByText(/Scouting ban hedefleri/)).toBeInTheDocument();
    expect(screen.getByText(/Rakibin ana kozu/)).toBeInTheDocument();
    expect(screen.getByText(/erken agresif/)).toBeInTheDocument();
    expect(container.querySelector('.ban-scouting-confidence')).toHaveTextContent('orta');
    expect(container.querySelector('.ban-scouting-threat')).toHaveTextContent('orta');
  });

  it('renders the team-level gameplan read with win condition and primary threat', () => {
    render(
      <BanSuggestionList suggestions={[ban(1, 'Ahri', 0.7)]} scoutingReport={scoutingReport} />,
    );
    expect(screen.getByText('Düşman planı')).toBeInTheDocument();
    expect(screen.getByText('Pick komp')).toBeInTheDocument(); // win condition i18n
    expect(screen.getByText(/Pick komp: vision basın/)).toBeInTheDocument(); // strategy note
    expect(screen.getByText('Ana tehdit: Zed')).toBeInTheDocument(); // primary threat
  });
});
