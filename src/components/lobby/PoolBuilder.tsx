import React, { useEffect, useState } from 'react';
import { invoke } from '../../lib/host';
import { useActiveSummonerPuuid } from '../../hooks/useActiveSummonerPuuid';
import { useTranslation } from 'react-i18next';
import type { PoolSuggestion } from '../../types/recommendation';
import type { ChampionPoolPlan } from '../../types/generated/ChampionPoolPlan';
import type { MasteryProgressEntry } from '../../types/generated/MasteryProgressEntry';
import { ChampionIcon } from '../shared/ChampionIcon';
import { archetypeLabel } from '../../lib/archetype';
import './PoolBuilder.css';

const ROLES = ['top', 'jungle', 'middle', 'bottom', 'utility'] as const;
type Role = (typeof ROLES)[number];

/**
 * Pool builder: champions to learn for a chosen role. Meta-led (current win-rate
 * from blended champion_rates) tempered by learnability (role fit + ease + blind
 * safety + synergy with the mastered pool). Degrades to learnability-led when no
 * meta data exists; a meta tier only shows when the sample is sufficient.
 */
export const PoolBuilder: React.FC = () => {
  const { t } = useTranslation();
  const [role, setRole] = useState<Role>('middle');
  // Ortak puuid çözümü (retry'lı) — stats kartlarıyla aynı hook. puuid çözülünce
  // [puuid]'e key'li suggestions/mastery effect'leri kendiliğinden yeniden koşar.
  const puuid = useActiveSummonerPuuid();
  const [suggestions, setSuggestions] = useState<PoolSuggestion[]>([]);
  const [loading, setLoading] = useState(true);
  const [poolPlan, setPoolPlan] = useState<ChampionPoolPlan | null>(null);
  const [progress, setProgress] = useState<MasteryProgressEntry[]>([]);

  // Mastery progress is role-independent — fetch once per player.
  useEffect(() => {
    if (!puuid) return;
    invoke<MasteryProgressEntry[]>('get_mastery_progress', { puuid, days: 30 })
      .then((p) => setProgress(p ?? []))
      .catch(() => setProgress([]));
  }, [puuid]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    Promise.allSettled([
      invoke<PoolSuggestion[]>('get_pool_suggestions', { role, puuid }),
      invoke<ChampionPoolPlan>('get_champion_pool_plan', { role, puuid }),
    ])
      .then(([suggestionResult, planResult]) => {
        if (cancelled) return;
        setSuggestions(suggestionResult.status === 'fulfilled' ? suggestionResult.value ?? [] : []);
        setPoolPlan(planResult.status === 'fulfilled' ? planResult.value ?? null : null);
        setLoading(false);
      })
      .catch(() => {
        if (!cancelled) {
          setSuggestions([]);
          setPoolPlan(null);
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [role, puuid]);

  return (
    <div className="pool-builder">
      <div className="pool-builder__roles" role="group" aria-label={t('poolBuilder.roleGroupLabel')}>
        {ROLES.map((r) => (
          <button
            key={r}
            type="button"
            className={`pool-builder__role${r === role ? ' pool-builder__role--active' : ''}`}
            aria-pressed={r === role}
            onClick={() => setRole(r)}
          >
            {t(`poolBuilder.role_${r}`)}
          </button>
        ))}
      </div>

      {progress.length > 0 && (
        <section className="pool-progress" aria-label={t('poolCoach.progressTitle')}>
          <span className="pool-progress__title">{t('poolCoach.progressTitle')}</span>
          <div className="pool-progress__items">
            {progress.map((p) => (
              <div key={p.champion_id} className="pool-progress__item">
                <ChampionIcon championKey={p.champion_key} size="sm" />
                <div className="pool-progress__copy">
                  <strong>{p.champion_key || `#${p.champion_id}`}</strong>
                  <span>{t('poolCoach.progressGain', { pts: p.points_gained.toLocaleString() })}</span>
                </div>
              </div>
            ))}
          </div>
        </section>
      )}

      {poolPlan ? (
        <section className="pool-coach" aria-label={t('poolCoach.title')}>
          <div className="pool-coach__header">
            <span className="pool-coach__eyebrow">{t('poolCoach.eyebrow')}</span>
            <span className={`pool-coach__state pool-coach__state--${poolPlan.data_state}`}>
              {t(`poolCoach.dataState.${poolPlan.data_state}`, { defaultValue: poolPlan.data_state })}
            </span>
          </div>
          <p className="pool-coach__summary">{poolPlan.summary}</p>
          {poolPlan.data_state === 'thin' ? (
            <p className="pool-coach__thin">{t('poolCoach.thinHint')}</p>
          ) : null}

          <div className="pool-coach__coverage" aria-label={t('poolCoach.coverageTitle')}>
            <CoverageChip
              label={t('poolCoach.coverage.role_covered')}
              ok={poolPlan.coverage.role_covered}
              t={t}
            />
            <CoverageChip
              label={t('poolCoach.coverage.has_blind_safe')}
              ok={poolPlan.coverage.has_blind_safe}
              t={t}
            />
            <CoverageChip
              label={t('poolCoach.coverage.has_counter_flex')}
              ok={poolPlan.coverage.has_counter_flex}
              t={t}
            />
            <CoverageChip
              label={t('poolCoach.coverage.has_comfort')}
              ok={poolPlan.coverage.has_comfort}
              t={t}
            />
            <CoverageChip
              label={t('poolCoach.coverage.identity_variety')}
              ok={poolPlan.coverage.identity_variety}
              t={t}
            />
            <span className={`pool-coach__chip pool-coach__risk pool-coach__risk--${poolPlan.coverage.execution_risk}`}>
              {t('poolCoach.coverage.execution_risk')}: {t(`poolCoach.executionRisk.${poolPlan.coverage.execution_risk}`, { defaultValue: poolPlan.coverage.execution_risk })}
            </span>
          </div>

          <div className="pool-coach__split">
            <div className="pool-coach__panel">
              <h3>{t('poolCoach.gapsTitle')}</h3>
              {poolPlan.gaps.length === 0 ? (
                <p className="pool-coach__muted">{t('poolCoach.noGaps')}</p>
              ) : (
                <ul className="pool-coach__gaps">
                  {poolPlan.gaps.map((gap) => (
                    <li key={`${gap.dimension}-${gap.severity}`}>
                      <span className={`pool-coach__severity pool-coach__severity--${gap.severity}`}>
                        {t(`poolCoach.severity.${gap.severity}`, { defaultValue: gap.severity })}
                      </span>
                      <strong>{t(`poolCoach.gap.${gap.dimension}`, { defaultValue: gap.dimension })}</strong>
                      <span>{gap.note}</span>
                    </li>
                  ))}
                </ul>
              )}
            </div>

            <div className="pool-coach__panel">
              <h3>{t('poolCoach.trainingTitle')}</h3>
              {poolPlan.training.length === 0 ? (
                <p className="pool-coach__muted">{t('poolCoach.noTraining')}</p>
              ) : (
                <div className="pool-coach__training">
                  {poolPlan.training.map((target) => (
                    <article key={`${target.role_in_plan}-${target.champion_id}`} className="pool-coach__target">
                      <ChampionIcon championKey={target.champion_key} size="sm" />
                      <div>
                        <strong>{target.champion_key}</strong>
                        <span>{t(`poolCoach.training.${target.role_in_plan}`, { defaultValue: target.role_in_plan })}</span>
                      </div>
                      <p>{target.reason}</p>
                      {target.drills.length > 0 && (
                        <div className="pool-coach__drills">
                          <span className="pool-coach__drills-title">{t('poolCoach.drillsTitle')}</span>
                          <ul>
                            {target.drills.map((drill, i) => (
                              <li key={i}>{drill}</li>
                            ))}
                          </ul>
                        </div>
                      )}
                      <small>{t(`champSelect.confidence.${target.confidence}`, { defaultValue: target.confidence })}</small>
                    </article>
                  ))}
                </div>
              )}
            </div>
          </div>
        </section>
      ) : null}

      {suggestions.length === 0 ? (
        <p className="pool-builder__empty">
          {t(loading || !puuid ? 'poolBuilder.loading' : 'poolBuilder.empty')}
        </p>
      ) : (
        <div className="pool-builder__grid">
          {suggestions.map((s) => (
            <div key={s.champion_id} className="pool-builder__card">
              <ChampionIcon championKey={s.champion_key} size="md" />
              <span className="pool-builder__name">{s.champion_key}</span>
              <span className="pool-builder__arch">{archetypeLabel(s.archetype)}</span>
              <div className="pool-builder__badges">
                <span className="pool-builder__ease">{t('poolBuilder.ease', { n: s.ease })}</span>
                {s.meta_tier ? (
                  <span className={`pool-builder__tier pool-builder__tier--${s.meta_tier.toLowerCase()}`}>
                    {t('poolBuilder.metaTier', { tier: s.meta_tier })}
                  </span>
                ) : null}
              </div>
              <span className="pool-builder__reason">{s.reason}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

interface CoverageChipProps {
  label: string;
  ok: boolean;
  t: ReturnType<typeof useTranslation>['t'];
}

const CoverageChip: React.FC<CoverageChipProps> = ({ label, ok, t }) => (
  <span className={`pool-coach__chip${ok ? ' pool-coach__chip--ok' : ' pool-coach__chip--gap'}`}>
    {label}: {ok ? t('poolCoach.coverage.ok') : t('poolCoach.coverage.missing')}
  </span>
);
