import React from 'react';
import { useTranslation } from 'react-i18next';
import { Recommendation } from '../../types/recommendation';
import { ChampionIcon } from '../shared/ChampionIcon';
import { TIER_COLORS } from '../../lib/tier';
import './QuickPickList.css';

interface Props {
  recommendations: Recommendation[];
  activeIndex: number;
  onSelect: (index: number) => void;
}

export const QuickPickList: React.FC<Props> = ({ recommendations, activeIndex, onSelect }) => {
  const { t } = useTranslation();
  return (
  <div className="quick-pick-list">
    {recommendations.slice(0, 5).map((rec, i) => (
      <div
        key={rec.champion_id}
        className={`quick-pick-row ${i === activeIndex ? 'quick-pick-row--active' : ''}`}
        onClick={() => onSelect(i)}
      >
        <span className="quick-pick-num">{i + 1}</span>
        <ChampionIcon championKey={rec.champion_key} size="sm" />
        <span className="quick-pick-name">{rec.champion_name || rec.champion_key}</span>
        <span className="quick-pick-tier" style={{ color: TIER_COLORS[rec.tier] }}>{rec.tier.toUpperCase()}</span>
        <span className="quick-pick-score">{Math.round(rec.total_score * 100)}%</span>
        <span
          className="quick-pick-conf"
          title={rec.confidence === 'low' ? t('quickPick.confLow') : rec.confidence === 'medium' ? t('quickPick.confMedium') : t('quickPick.confHigh')}
        >
          {rec.confidence === 'high' ? '●●●' : rec.confidence === 'medium' ? '●●○' : '●○○'}
        </span>
      </div>
    ))}
  </div>
  );
};
