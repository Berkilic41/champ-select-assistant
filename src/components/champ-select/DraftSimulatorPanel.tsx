import React from 'react';
import { useTranslation } from 'react-i18next';
import type { DraftSimResult } from '../../types/generated/DraftSimResult';
import { ChampionIcon } from '../shared/ChampionIcon';
import './DraftSimulatorPanel.css';

interface Props {
  results?: DraftSimResult[] | null;
}

function signedScore(value: number): string {
  const rounded = Math.round(value * 100) / 100;
  return `${rounded > 0 ? '+' : ''}${rounded.toFixed(2)}`;
}

function factorKey(factor: string): string {
  return `draftSimulator.factor.${factor}`;
}

export const DraftSimulatorPanel: React.FC<Props> = ({ results }) => {
  const { t } = useTranslation();
  const visible = (results ?? []).slice(0, 3);
  if (visible.length === 0) return null;

  return (
    <section className="draft-sim" aria-label={t('draftSimulator.eyebrow')}>
      <div className="draft-sim__head">
        <div>
          <p className="draft-sim__eyebrow">{t('draftSimulator.eyebrow')}</p>
          <h3 className="draft-sim__title">{t('draftSimulator.title')}</h3>
        </div>
        <span className="draft-sim__badge">{t('draftSimulator.localOnly')}</span>
      </div>

      <div className="draft-sim__list">
        {visible.map((result, index) => {
          // Faktör adı → işaretli sayısal delta (core'un `deltas`'ından). Faktör
          // listede yoksa salt-isim gösterilir (geriye uyumlu).
          const deltaMap = new Map(result.deltas.map((d) => [d.factor, d.delta]));
          const fmtDelta = (factor: string): string => {
            const d = deltaMap.get(factor);
            return d != null ? ` ${signedScore(d)}` : '';
          };
          return (
            <article key={result.champion_id} className="draft-sim__row">
              <div className="draft-sim__rank">{index + 1}</div>
              <ChampionIcon championKey={result.champion_key} size="sm" />
              <div className="draft-sim__body">
                <div className="draft-sim__row-head">
                  <strong>{result.champion_key || `#${result.champion_id}`}</strong>
                  <span className="draft-sim__score">
                    {t('draftSimulator.scoreDelta', { score: signedScore(result.score_delta) })}
                  </span>
                </div>

                <p className="draft-sim__coach">{result.coach_sentence}</p>
                {result.why_this_move.trim() && (
                  <p className="draft-sim__why-this">
                    <span>{t('draftSimulator.whyThis')}</span> {result.why_this_move}
                  </p>
                )}
                <div className="draft-sim__meta">
                  <span className={`draft-sim__risk draft-sim__risk--${result.risk.level}`}>
                    {t('draftSimulator.risk', {
                      level: t(`draftSimulator.riskLevel.${result.risk.level}`, {
                        defaultValue: result.risk.level,
                      }),
                    })}
                  </span>
                  <span>{result.plan_shift.note}</span>
                </div>

                <div className="draft-sim__factors">
                  {result.improved_factors.slice(0, 3).map((factor) => (
                    <span key={`up-${factor}`} className="draft-sim__factor draft-sim__factor--up">
                      {t(factorKey(factor), { defaultValue: factor })}
                      {fmtDelta(factor)}
                    </span>
                  ))}
                  {result.worsened_factors.slice(0, 2).map((factor) => (
                    <span key={`down-${factor}`} className="draft-sim__factor draft-sim__factor--down">
                      {t(factorKey(factor), { defaultValue: factor })}
                      {fmtDelta(factor)}
                    </span>
                  ))}
                </div>

                {index === 0 && result.why_not_alternative && (
                  <p className="draft-sim__why-not">
                    <span>{t('draftSimulator.whyNot')}</span> {result.why_not_alternative}
                  </p>
                )}
              </div>
            </article>
          );
        })}
      </div>
    </section>
  );
};
