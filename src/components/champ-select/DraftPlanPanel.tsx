import React from 'react';
import { useTranslation } from 'react-i18next';
import type { DraftPlan } from '../../types/recommendation';
import './DraftPlanPanel.css';

interface Props {
  plan?: DraftPlan;
}

export const DraftPlanPanel: React.FC<Props> = ({ plan }) => {
  const { t } = useTranslation();
  if (!plan) return null;

  const hasCombo = plan.combo_with.length > 0;
  const hasFills = plan.fills_team_need.length > 0;
  const hasThreats = plan.threats.length > 0;

  return (
    <div className="draft-plan">
      {plan.win_condition && (
        <div className="draft-plan__section">
          <span className="draft-plan__label">{t('draftPlan.winCondition')}</span>
          <p className="draft-plan__text">{plan.win_condition}</p>
        </div>
      )}

      {plan.lane_phase_advice && (
        <div className="draft-plan__section">
          <span className="draft-plan__label">{t('draftPlan.lanePhase')}</span>
          <p className="draft-plan__text draft-plan__text--lane">{plan.lane_phase_advice}</p>
        </div>
      )}

      {plan.spike_note && (
        <div className="draft-plan__section">
          <span className="draft-plan__label">{t('draftPlan.powerSpike')}</span>
          <p className="draft-plan__text draft-plan__text--spike">{plan.spike_note}</p>
        </div>
      )}

      {plan.comp_clash_note && (
        <div className="draft-plan__section">
          <span className="draft-plan__label">{t('draftPlan.compClash')}</span>
          <p className="draft-plan__text draft-plan__text--clash">{plan.comp_clash_note}</p>
        </div>
      )}

      {plan.damage_profile && (
        <div className="draft-plan__section">
          <span className="draft-plan__label">{t('draftPlan.damageProfile')}</span>
          <p className="draft-plan__text">{plan.damage_profile}</p>
        </div>
      )}

      {typeof plan.blind_pick_safety === 'number' && (
        <div className="draft-plan__section">
          <span className="draft-plan__label">{t('draftPlan.pickSafety')}</span>
          <span
            className={`draft-plan__safety-badge ${
              plan.blind_pick_safety >= 0.75
                ? 'draft-plan__safety-badge--safe'
                : plan.blind_pick_safety >= 0.5
                  ? 'draft-plan__safety-badge--medium'
                  : 'draft-plan__safety-badge--risky'
            }`}
          >
            {plan.blind_pick_safety >= 0.75
              ? t('draftPlan.safetySafe')
              : plan.blind_pick_safety >= 0.5
                ? t('draftPlan.safetyMedium')
                : t('draftPlan.safetyRisky')}
          </span>
        </div>
      )}

      {typeof plan.execution_difficulty === 'number' && (
        <div className="draft-plan__section">
          <span className="draft-plan__label">{t('draftPlan.execDifficulty')}</span>
          <span
            className={`draft-plan__exec-badge ${
              plan.execution_difficulty <= 2
                ? 'draft-plan__exec-badge--easy'
                : plan.execution_difficulty === 3
                  ? 'draft-plan__exec-badge--medium'
                  : 'draft-plan__exec-badge--hard'
            }`}
          >
            {plan.execution_difficulty <= 2
              ? t('draftPlan.execEasy')
              : plan.execution_difficulty === 3
                ? t('draftPlan.execMedium')
                : t('draftPlan.execHard', { n: plan.execution_difficulty })}
          </span>
        </div>
      )}

      {hasCombo && (
        <div className="draft-plan__section">
          <span className="draft-plan__label">{t('draftPlan.comboPlans')}</span>
          <ul className="draft-plan__list">
            {plan.combo_with.map((hint, i) => (
              <li key={i} className="draft-plan__list-item">
                <strong>{hint.ally_champion_key}:</strong> {hint.combo_text}
              </li>
            ))}
          </ul>
        </div>
      )}

      {hasFills && (
        <div className="draft-plan__section">
          <span className="draft-plan__label">{t('draftPlan.teamNeed')}</span>
          <div className="draft-plan__chips">
            {plan.fills_team_need.map((fill, i) => (
              <span key={i} className="draft-plan__chip">
                {fill}
              </span>
            ))}
          </div>
        </div>
      )}

      {hasThreats && (
        <div className="draft-plan__section">
          <span className="draft-plan__label">{t('draftPlan.risks')}</span>
          <ul className="draft-plan__list">
            {plan.threats.map((threat, i) => (
              <li key={i} className="draft-plan__list-item draft-plan__list-item--risk">
                {threat}
              </li>
            ))}
          </ul>
        </div>
      )}

      {plan.pick_window_note && (
        <div className="draft-plan__section">
          <span className="draft-plan__label">{t('draftPlan.pickPosition')}</span>
          <p className="draft-plan__text draft-plan__text--counter">{plan.pick_window_note}</p>
        </div>
      )}

      {plan.risk_note && (
        <div className="draft-plan__risk-note" role="alert">
          {plan.risk_note}
        </div>
      )}
    </div>
  );
};
