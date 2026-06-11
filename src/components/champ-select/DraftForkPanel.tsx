import React from 'react';
import { useTranslation } from 'react-i18next';
import type { DraftFork } from '../../types/generated/DraftFork';
import { ChampionIcon } from '../shared/ChampionIcon';
import './DraftForkPanel.css';

interface Props {
  fork?: DraftFork | null;
}

function factorKey(factor: string): string {
  return `draftSimulator.factor.${factor}`;
}

function signedScore(value: number): string {
  const rounded = Math.round(value * 100) / 100;
  return `${rounded > 0 ? '+' : ''}${rounded.toFixed(2)}`;
}

export const DraftForkPanel: React.FC<Props> = ({ fork }) => {
  const { t } = useTranslation();
  if (!fork) return null;

  const options = [fork.option_a, fork.option_b];
  const shared = fork.shared_factors.slice(0, 3);
  const diverging = fork.diverging_factors.slice(0, 3);

  return (
    <section className="draft-fork" aria-label={t('draftFork.eyebrow')}>
      <div className="draft-fork__head">
        <div>
          <p className="draft-fork__eyebrow">{t('draftFork.eyebrow')}</p>
          <h3 className="draft-fork__title">{t('draftFork.title')}</h3>
        </div>
      </div>

      <div className="draft-fork__options">
        {options.map((option) => (
          <div key={option.champion_id} className={`draft-fork__option draft-fork__option--${option.risk.level}`}>
            <ChampionIcon championKey={option.champion_key} size="sm" />
            <div className="draft-fork__option-copy">
              <strong>{option.champion_key || `#${option.champion_id}`}</strong>
              <span>
                {t('draftFork.optionLine', {
                  score: signedScore(option.score_delta),
                  risk: t(`draftSimulator.riskLevel.${option.risk.level}`, {
                    defaultValue: option.risk.level,
                  }),
                })}
              </span>
            </div>
          </div>
        ))}
      </div>

      <p className="draft-fork__recommendation">{fork.recommendation}</p>

      <div className="draft-fork__reads">
        <p>{fork.plan_divergence}</p>
        <p>{fork.risk_divergence}</p>
      </div>

      {(shared.length > 0 || diverging.length > 0) && (
        <div className="draft-fork__factors">
          {shared.length > 0 && (
            <div>
              <span className="draft-fork__label">{t('draftFork.shared')}</span>
              <div className="draft-fork__chips">
                {shared.map((factor) => (
                  <span key={`shared-${factor}`} className="draft-fork__chip">
                    {t(factorKey(factor), { defaultValue: factor })}
                  </span>
                ))}
              </div>
            </div>
          )}
          {diverging.length > 0 && (
            <div>
              <span className="draft-fork__label">{t('draftFork.diverging')}</span>
              <div className="draft-fork__chips">
                {diverging.map((factor) => (
                  <span key={`diverging-${factor}`} className="draft-fork__chip draft-fork__chip--split">
                    {t(factorKey(factor), { defaultValue: factor })}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </section>
  );
};
