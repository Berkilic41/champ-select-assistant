import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { BanSuggestionList } from './BanSuggestionList';
import type { BanSuggestion } from '../../types/recommendation';

const ban = (id: number, name: string, threat: number): BanSuggestion => ({
  champion_id: id,
  champion_key: name,
  champion_name: name,
  threat_score: threat,
  reason: `${name} tehdidi`,
});

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
});
