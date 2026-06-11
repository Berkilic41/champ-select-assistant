import React from 'react';
import { useTranslation } from 'react-i18next';
import type { CounterPickHint } from '../../types/recommendation';
import { ChampionIcon } from '../shared/ChampionIcon';
import './CounterPickBoard.css';

interface Props {
  picks: CounterPickHint[];
}

const PHASE_KEYS = ['early', 'mid', 'late'] as const;

/** The phase where this counter is clearly strongest, or null when it's flat. */
function strongestPhase(pa?: [number, number, number]): 'early' | 'mid' | 'late' | null {
  if (!pa) return null;
  const max = Math.max(...pa);
  const min = Math.min(...pa);
  if (max < 0.55 || max - min < 0.08) return null;
  return PHASE_KEYS[pa.indexOf(max)];
}

/**
 * Counter-pick board: champions from the player's own pool that counter the
 * visible lane opponent (≤3). Hidden when there are none.
 */
export const CounterPickBoard: React.FC<Props> = ({ picks }) => {
  const { t } = useTranslation();
  if (!picks.length) return null;

  return (
    <div className="counter-board">
      <span className="counter-board__label">{t('counterPick.title')}</span>
      <div className="counter-board__list">
        {picks.map((p) => (
          <div key={p.champion_id} className="counter-board__row">
            <ChampionIcon championKey={p.champion_key} size="sm" />
            <div className="counter-board__body">
              <div className="counter-board__head">
                <span className="counter-board__name">{p.champion_name || p.champion_key}</span>
                <span className="counter-board__adv">
                  {Math.round(Math.max(0, Math.min(1, p.advantage)) * 100)}%
                </span>
              </div>
              <span className="counter-board__reason">
                {p.reason}
                {(() => {
                  const phase = strongestPhase(p.phase_advantage);
                  return phase ? (
                    <span className="counter-board__phase"> · {t('counterPick.strongPhase', { phase: t(`counterPick.phase.${phase}`) })}</span>
                  ) : null;
                })()}
                {p.games_on_champ > 0 && (
                  <span className="counter-board__games"> · {t('counterPick.games', { n: p.games_on_champ })}</span>
                )}
              </span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};
