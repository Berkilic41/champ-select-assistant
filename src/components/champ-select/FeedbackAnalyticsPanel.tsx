import React from 'react';
import { useTranslation } from 'react-i18next';
import type { FeedbackAnalytics } from '../../types/generated/FeedbackAnalytics';
import type { ChampionFeedbackTrend } from '../../types/generated/ChampionFeedbackTrend';
import { ChampionIcon } from '../shared/ChampionIcon';
import './FeedbackAnalyticsPanel.css';

interface Props {
  analytics?: FeedbackAnalytics | null;
}

function signedPct(value: number): string {
  const rounded = Math.round(value * 100);
  return `${rounded > 0 ? '+' : ''}${rounded}%`;
}

function displayName(trend: ChampionFeedbackTrend): string {
  return trend.champion_key || `#${trend.champion_id}`;
}

export const FeedbackAnalyticsPanel: React.FC<Props> = ({ analytics }) => {
  const { t } = useTranslation();
  if (!analytics || analytics.total_events === 0) return null;

  const disliked = analytics.disliked.slice(0, 3);

  return (
    <section className="feedback-analytics" aria-label={t('feedbackAnalytics.eyebrow')}>
      <div className="feedback-analytics__head">
        <div>
          <p className="feedback-analytics__eyebrow">{t('feedbackAnalytics.eyebrow')}</p>
          <h3 className="feedback-analytics__title">{t('feedbackAnalytics.title')}</h3>
        </div>
        <span className="feedback-analytics__badge">
          {t('feedbackAnalytics.window', { n: analytics.window_days })}
        </span>
      </div>

      <div className="feedback-analytics__metrics">
        <span>{t('feedbackAnalytics.recentSignals', { n: analytics.recent_signal_count })}</span>
        <span>{t('feedbackAnalytics.totalEvents', { n: analytics.total_events })}</span>
        <span>{t('feedbackAnalytics.trackedChampions', { n: analytics.trends.length })}</span>
      </div>

      {disliked.length > 0 ? (
        <div className="feedback-analytics__section">
          <p className="feedback-analytics__section-title">{t('feedbackAnalytics.dislikedTitle')}</p>
          <div className="feedback-analytics__list">
            {disliked.map((trend) => (
              <div key={trend.champion_id} className="feedback-analytics__row">
                <ChampionIcon championKey={trend.champion_key} size="sm" />
                <div className="feedback-analytics__copy">
                  <strong>{displayName(trend)}</strong>
                  <span>
                    {t('feedbackAnalytics.dislikedLine', {
                      sample: trend.sample,
                      sentiment: signedPct(trend.net_sentiment),
                    })}
                  </span>
                </div>
              </div>
            ))}
          </div>
        </div>
      ) : (
        <p className="feedback-analytics__empty">{t('feedbackAnalytics.noDisliked')}</p>
      )}
    </section>
  );
};
