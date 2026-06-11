import React from 'react';
import { useTranslation } from 'react-i18next';
import type { LaneMatchup } from '../../types/recommendation';
import { ChampionIcon } from '../shared/ChampionIcon';
import './LaneMatchupPanel.css';

interface Props {
  matchup?: LaneMatchup | null;
}

function pct(v: number): number {
  return Math.round(Math.max(0, Math.min(1, v)) * 100);
}

function tone(v: number): 'up' | 'down' | 'even' {
  if (v > 0.55) return 'up';
  if (v < 0.45) return 'down';
  return 'even';
}

/**
 * Lane matchup: local pick vs the visible lane opponent — per-phase advantage
 * bars + concrete tips. Hidden when there's no lane opponent.
 */
export const LaneMatchupPanel: React.FC<Props> = ({ matchup }) => {
  const { t } = useTranslation();
  if (!matchup) return null;

  const phases: Array<[string, number]> = [
    [t('champDetail.early'), matchup.phase_advantage[0]],
    [t('champDetail.mid'), matchup.phase_advantage[1]],
    [t('champDetail.late'), matchup.phase_advantage[2]],
  ];

  return (
    <div className="lane-matchup">
      <div className="lane-matchup__head">
        <span className="lane-matchup__label">{t('laneMatchup.title')}</span>
        <span className="lane-matchup__opp">
          <ChampionIcon championKey={matchup.opponent_key} size="sm" />
          {matchup.opponent_name || matchup.opponent_key}
          {matchup.inferred && (
            <span className="lane-matchup__inferred" title={t('laneMatchup.inferredHint')}>
              {t('laneMatchup.inferred')}
            </span>
          )}
        </span>
      </div>
      <div className="lane-matchup__phases">
        {phases.map(([label, v], i) => (
          <div key={i} className={`lane-matchup__phase lane-matchup__phase--${tone(v)}`}>
            <span className="lane-matchup__phase-label">{label}</span>
            <div className="lane-matchup__track">
              <div className="lane-matchup__fill" style={{ width: `${pct(v)}%` }} />
            </div>
            <span className="lane-matchup__pct">{pct(v)}%</span>
          </div>
        ))}
      </div>
      {matchup.tips.length > 0 && (
        <ul className="lane-matchup__tips">
          {matchup.tips.map((tip, i) => (
            <li key={i} className="lane-matchup__tip">{tip}</li>
          ))}
        </ul>
      )}
    </div>
  );
};
