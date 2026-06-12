import React from 'react';
import { useTranslation } from 'react-i18next';
import { BanSuggestion } from '../../types/recommendation';
import { ChampionIcon } from '../shared/ChampionIcon';
import './BanSuggestionList.css';

interface Props {
  suggestions: BanSuggestion[];
}

export const BanSuggestionList: React.FC<Props> = ({ suggestions }) => {
  const { t } = useTranslation();

  if (!suggestions.length) return (
    <p className="ban-suggestion-empty">{t('champSelect.banComputing')}</p>
  );

  return (
    <div className="ban-suggestion-list">
      <p className="ban-suggestion-title">{t('champSelect.banSuggestions')}</p>
      {suggestions.slice(0, 3).map((s, i) => (
        <div key={s.champion_id} className="ban-suggestion-row">
          <span className="ban-suggestion-rank">{i + 1}</span>
          <ChampionIcon championKey={s.champion_key} size="sm" />
          <div className="ban-suggestion-body">
            <div className="ban-suggestion-head">
              <span className="ban-suggestion-name">{s.champion_name || s.champion_key}</span>
              <span className="ban-suggestion-threat">{Math.round(s.threat_score * 100)}%</span>
            </div>
            <span className="ban-suggestion-reason">{s.reason}</span>
          </div>
        </div>
      ))}
    </div>
  );
};
