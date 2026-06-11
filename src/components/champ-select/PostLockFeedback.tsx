import React, { useState } from 'react';
import { invoke } from '../../lib/host';
import { useTranslation } from 'react-i18next';
import { Recommendation } from '../../types/recommendation';
import type { FeedbackVerdict, RecommendationFeedbackPayload } from '../../types/feedback';
import { Button } from '../shared/ui';
import './PostLockFeedback.css';

interface Props {
  rec: Recommendation;
  onFeedbackSubmitted?: () => void;
}

/** Post-decision feedback — shown only AFTER lock (finalization / locked view), never
 *  on the active decision card where it competed for attention during deliberation. */
export const PostLockFeedback: React.FC<Props> = ({ rec, onFeedbackSubmitted }) => {
  const { t } = useTranslation();
  const [sent, setSent] = useState(false);
  const [failed, setFailed] = useState(false);

  async function send(feedback: FeedbackVerdict, reason: string = feedback) {
    setSent(true);
    setFailed(false);
    const payload: RecommendationFeedbackPayload = {
      championId: rec.champion_id,
      championKey: rec.champion_key,
      feedback,
      modelVersion: rec.model_version ?? 'unknown',
      score: rec.total_score,
      payload: {
        decisionSentence: rec.decision_sentence ?? null,
        reason: rec.reason,
        feedbackReason: reason,
        whyNot: rec.why_not ?? [],
      },
    };
    await invoke('submit_recommendation_feedback', { feedback: payload })
      .then(() => onFeedbackSubmitted?.())
      .catch(() => {
        // Don't fail silently — revert to the buttons and tell the user it failed.
        setSent(false);
        setFailed(true);
      });
  }

  return (
    <div
      className="post-lock-feedback"
      aria-label={t('heroCard.feedbackLabel', { defaultValue: 'Öneri feedback' })}
    >
      {sent ? (
        <span className="post-lock-feedback__thanks">
          {t('heroCard.feedbackThanks', { defaultValue: 'Feedback kaydedildi' })}
        </span>
      ) : (
        <>
          <span className="post-lock-feedback__q">
            {t('heroCard.feedbackLabel', { defaultValue: 'Öneri feedback' })}
          </span>
          <Button variant="subtle" onClick={() => send('helpful')}>
            {t('heroCard.feedbackGood', { defaultValue: 'Mantıklı' })}
          </Button>
          <Button variant="subtle" onClick={() => send('not_helpful')}>
            {t('heroCard.feedbackBad', { defaultValue: 'Kötü' })}
          </Button>
          <Button variant="subtle" onClick={() => send('not_helpful', 'outside_pool')}>
            {t('heroCard.feedbackPool', { defaultValue: 'Pool dışı' })}
          </Button>
          <Button variant="subtle" onClick={() => send('not_helpful', 'risky')}>
            {t('heroCard.feedbackRisk', { defaultValue: 'Riskli' })}
          </Button>
          {failed && (
            <span className="post-lock-feedback__error">{t('app.feedbackFailed')}</span>
          )}
        </>
      )}
    </div>
  );
};
