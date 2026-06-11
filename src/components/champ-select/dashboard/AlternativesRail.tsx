import React from 'react';
import clsx from 'clsx';
import { motion } from 'framer-motion';
import { useTranslation } from 'react-i18next';
import { Recommendation } from '../../../types/recommendation';
import { ChampionIcon } from '../../shared/ChampionIcon';
import { ConfidenceRing } from '../../shared/ui';
import { TierBadge } from '../../../lib/tier';
import './AlternativesRail.css';

interface Props {
  recommendations: Recommendation[];
  activeIndex: number;
  onSelect: (index: number) => void;
  /** Optional: apply a hover in the LoL client (double-click / future action). */
  onHover?: (championId: number) => void;
}

/** Persistent right-rail of alternatives. #1 is the visually dominant "pick"; 2-5 are
 *  compact. One-tap (or [1-5]) switches the active decision card. */
export const AlternativesRail = React.memo(function AlternativesRail({
  recommendations,
  activeIndex,
  onSelect,
  onHover,
}: Props) {
  const { t } = useTranslation();
  return (
    <div
      className="alt-rail"
      role="listbox"
      aria-label={t('quickPick.title', { defaultValue: 'Alternatifler' })}
    >
      {recommendations.slice(0, 5).map((rec, i) => {
        const active = i === activeIndex;
        const lead = i === 0;
        return (
          <button
            key={rec.champion_id}
            type="button"
            role="option"
            aria-selected={active}
            className={clsx('alt-row', active && 'alt-row--active', lead && 'alt-row--lead')}
            onClick={() => onSelect(i)}
            onDoubleClick={onHover ? () => onHover(rec.champion_id) : undefined}
            title={`[${i + 1}] ${rec.champion_name || rec.champion_key}`}
          >
            {active && <motion.span layoutId="alt-active" className="alt-row__bg" />}
            <span className="alt-row__num">{i + 1}</span>
            <ChampionIcon championKey={rec.champion_key} size={lead ? 'md' : 'sm'} />
            <div className="alt-row__body">
              <span className="alt-row__name">{rec.champion_name || rec.champion_key}</span>
              <span className="alt-row__meta">
                <TierBadge tier={rec.tier} />
                <span className="alt-row__score">{Math.round(rec.total_score * 100)}%</span>
              </span>
            </div>
            <ConfidenceRing score={rec.total_score} confidence={rec.confidence} size={lead ? 40 : 32} />
          </button>
        );
      })}
    </div>
  );
});
