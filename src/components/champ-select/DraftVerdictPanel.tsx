import React from 'react';
import { useTranslation } from 'react-i18next';
import type { DraftVerdict } from '../../types/recommendation';
import './DraftVerdictPanel.css';

interface Props {
  verdict?: DraftVerdict | null;
}

/**
 * Draft verdict: one decisive read — favorability + dodge consideration + top
 * action + team needs. The dodge note defers the final call to the player.
 */
export const DraftVerdictPanel: React.FC<Props> = ({ verdict }) => {
  const { t } = useTranslation();
  if (!verdict) return null;

  return (
    <div className={`verdict verdict--${verdict.favorability}`}>
      <div className="verdict__head">
        <span className="verdict__badge">{t(`verdict.${verdict.favorability}`)}</span>
        <span className="verdict__headline">{verdict.headline}</span>
      </div>

      {verdict.dodge_consider && verdict.dodge_note && (
        <div className="verdict__dodge" role="alert">⚠ {verdict.dodge_note}</div>
      )}

      {verdict.reasons.length > 0 && (
        <ul className="verdict__reasons">
          {verdict.reasons.map((r, i) => (
            <li key={i} className="verdict__reason">{r}</li>
          ))}
        </ul>
      )}

      <p className="verdict__action">
        <span className="verdict__label">{t('verdict.topAction')}</span> {verdict.top_action}
      </p>

      {verdict.team_needs.length > 0 && (
        <div className="verdict__needs">
          <span className="verdict__label">{t('verdict.teamNeeds')}</span>
          <div className="verdict__chips">
            {verdict.team_needs.map((n, i) => (
              <span key={i} className="verdict__chip">{n}</span>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};
