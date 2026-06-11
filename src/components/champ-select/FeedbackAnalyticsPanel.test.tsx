import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { FeedbackAnalyticsPanel } from './FeedbackAnalyticsPanel';
import type { FeedbackAnalytics } from '../../types/generated/FeedbackAnalytics';

const analytics: FeedbackAnalytics = {
  window_days: 7,
  total_events: 8,
  recent_signal_count: 5,
  trends: [
    {
      champion_id: 238,
      champion_key: 'Zed',
      helpful: 0,
      picked: 0,
      not_helpful: 3,
      sample: 3,
      net_sentiment: -1,
      recent_count: 3,
    },
  ],
  disliked: [
    {
      champion_id: 238,
      champion_key: 'Zed',
      helpful: 0,
      picked: 0,
      not_helpful: 3,
      sample: 3,
      net_sentiment: -1,
      recent_count: 3,
    },
  ],
};

describe('FeedbackAnalyticsPanel', () => {
  it('renders nothing without feedback events', () => {
    const { container } = render(
      <FeedbackAnalyticsPanel analytics={{ ...analytics, total_events: 0 }} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it('shows recent signal count and disliked recommendation trends', () => {
    render(<FeedbackAnalyticsPanel analytics={analytics} />);
    expect(screen.getByText('Öneri kalitesi')).toBeInTheDocument();
    expect(screen.getByText('Son 7 gün')).toBeInTheDocument();
    expect(screen.getByText('5 yeni sinyal')).toBeInTheDocument();
    expect(screen.getByText('Zed')).toBeInTheDocument();
    expect(screen.getByText('3 sinyal · duygu -100%')).toBeInTheDocument();
  });

  it('shows an empty state when no recommendation has a negative trend', () => {
    render(<FeedbackAnalyticsPanel analytics={{ ...analytics, disliked: [] }} />);
    expect(screen.getByText('Negatif trend biriken öneri yok')).toBeInTheDocument();
  });
});
