import React from 'react';
import { useTranslation } from 'react-i18next';
import { BanSuggestion, EnemyPoolSummary } from '../../types/recommendation';
import { ChampionIcon } from '../shared/ChampionIcon';
import './BanSuggestionList.css';

interface Props {
  suggestions: BanSuggestion[];
  enemyPools?: EnemyPoolSummary[];
}

export const BanSuggestionList: React.FC<Props> = ({ suggestions, enemyPools = [] }) => {
  const { t } = useTranslation();
  if (!suggestions.length) return (
    <p className="ban-suggestion-empty">{t('champSelect.banComputing')}</p>
  );

  const otpMap = new Map(enemyPools.map(p => [p.top_champion_id, p]));

  return (
    <div className="ban-suggestion-list">
      <p className="ban-suggestion-title">{t('champSelect.banSuggestions')}</p>
      {suggestions.slice(0, 3).map(s => {
        const otp = otpMap.get(s.champion_id);
        const otpNote = otp && otp.play_rate >= 0.40
          ? t('champSelect.banOtp', { pct: Math.round(otp.play_rate * 100) })
          : null;
        return (
          <div key={s.champion_id} className="ban-suggestion-row">
            <ChampionIcon championKey={s.champion_key} size="sm" />
            <span className="ban-suggestion-name">{s.champion_name || s.champion_key}</span>
            <span className="ban-suggestion-threat">{Math.round(s.threat_score * 100)}%</span>
            <span className="ban-suggestion-reason">
              {s.reason}
              {otpNote && <span className="ban-suggestion-otp"> · {otpNote}</span>}
            </span>
          </div>
        );
      })}
    </div>
  );
};
