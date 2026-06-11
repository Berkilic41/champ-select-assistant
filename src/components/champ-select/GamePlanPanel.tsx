import React from 'react';
import { useTranslation } from 'react-i18next';
import type { GamePlan } from '../../types/recommendation';
import './GamePlanPanel.css';

interface Props {
  plan?: GamePlan | null;
}

function pct(v: number): number {
  return Math.round(Math.max(0, Math.min(1, v)) * 100);
}

/**
 * Team-level macro game plan (#6): identity, phase-by-phase timeline, win
 * conditions, objective priorities, enemy threat and a fallback. Distinct from
 * the per-champion DraftPlan — this is "how the team wins the game".
 */
export const GamePlanPanel: React.FC<Props> = ({ plan }) => {
  const { t } = useTranslation();
  if (!plan) return null;

  const stanceLabel = (s: string) =>
    s === 'advantage'
      ? t('gamePlan.stanceAdvantage')
      : s === 'disadvantage'
        ? t('gamePlan.stanceDisadvantage')
        : t('gamePlan.stanceEven');

  return (
    <div className="game-plan">
      <div className="game-plan__head">
        <span className="game-plan__title">{t('gamePlan.title')}</span>
        <span className="game-plan__identity">{plan.team_identity}</span>
      </div>
      {plan.identity_note && <p className="game-plan__note">{plan.identity_note}</p>}
      {plan.partial && <p className="game-plan__partial">{t('gamePlan.partial')}</p>}

      <div className="game-plan__section">
        <span className="game-plan__label">{t('gamePlan.timeline')}</span>
        <div className="game-plan__timeline">
          {plan.timeline.map((band, i) => (
            <div key={i} className={`game-plan__band game-plan__band--${band.stance}`}>
              <div className="game-plan__band-head">
                <span className="game-plan__band-label">{band.label}</span>
                <span className="game-plan__band-stance">{stanceLabel(band.stance)}</span>
              </div>
              <div className="game-plan__band-track">
                <div className="game-plan__band-fill" style={{ width: `${pct(band.strength)}%` }} />
              </div>
              <p className="game-plan__band-advice">{band.advice}</p>
            </div>
          ))}
        </div>
      </div>

      {plan.win_conditions.length > 0 && (
        <div className="game-plan__section">
          <span className="game-plan__label">{t('gamePlan.winConditions')}</span>
          <ul className="game-plan__list">
            {plan.win_conditions.map((w, i) => (
              <li key={i} className="game-plan__item">{w}</li>
            ))}
          </ul>
        </div>
      )}

      {plan.objectives.length > 0 && (
        <div className="game-plan__section">
          <span className="game-plan__label">{t('gamePlan.objectives')}</span>
          <ul className="game-plan__list">
            {plan.objectives.map((o, i) => (
              <li key={i} className="game-plan__item">{o}</li>
            ))}
          </ul>
        </div>
      )}

      {plan.enemy_threat && (
        <div className="game-plan__section">
          <span className="game-plan__label">{t('gamePlan.enemyThreat')}</span>
          <p className="game-plan__text game-plan__text--threat">{plan.enemy_threat}</p>
        </div>
      )}

      {plan.alt_plan && (
        <div className="game-plan__section">
          <span className="game-plan__label">{t('gamePlan.altPlan')}</span>
          <p className="game-plan__text">{plan.alt_plan}</p>
        </div>
      )}
    </div>
  );
};
