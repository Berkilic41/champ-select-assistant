import React from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { Recommendation } from '../../../types/recommendation';
import type { DraftSimResult } from '../../../types/generated/DraftSimResult';
import type { DraftFork } from '../../../types/generated/DraftFork';
import { StatBar } from '../../shared/ui';
import { DraftSimulatorPanel } from '../DraftSimulatorPanel';
import { DraftForkPanel } from '../DraftForkPanel';
import '../HeroCard.css';

interface Props {
  rec: Recommendation;
  draftSimulation?: DraftSimResult[] | null;
  draftFork?: DraftFork | null;
}

interface CoachPillar {
  key: string;
  label: string;
  text: string;
  tone?: 'neutral' | 'risk' | 'data';
}

function firstText(...values: Array<string | null | undefined>): string | null {
  for (const value of values) {
    const clean = (value ?? '').trim();
    if (clean) return clean;
  }
  return null;
}

function formatSourceTime(value?: number | null): string {
  if (value == null) return '';
  return new Date(value * 1000).toLocaleDateString();
}

function buildCoachPillars(rec: Recommendation, t: TFunction): CoachPillar[] {
  const plan = rec.draft_plan;
  const decisionText = (rec.decision_sentence ?? '').trim() || rec.reason;
  const lossRiskText = firstText(rec.risk_summary, plan?.risk_note, plan?.threats?.[0]);
  const primaryDataSource = rec.data_sources?.[0];
  const dataSummary = primaryDataSource
    ? t('heroCard.dataSourceSummary', {
        source: primaryDataSource.source.replace(/_/g, ' '),
        conf: primaryDataSource.confidence,
        risk: primaryDataSource.risk_level,
      })
    : firstText(
        rec.matchup_confidence ? t('heroCard.matchupConfidence', { conf: rec.matchup_confidence }) : null,
        rec.build_confidence ? t('heroCard.buildConfidence', { conf: rec.build_confidence }) : null,
      );
  const raw: CoachPillar[] = [
    { key: 'why', label: t('heroCard.pillarWhy'), text: decisionText },
    { key: 'lane', label: t('heroCard.pillarLane'), text: firstText(rec.lane_plan, plan?.lane_phase_advice) ?? '' },
    { key: 'mid', label: t('heroCard.pillarMidGame'), text: firstText(rec.mid_game_plan, plan?.spike_note) ?? '' },
    { key: 'fight', label: t('heroCard.pillarTeamfight'), text: firstText(rec.teamfight_job, rec.teamfight_plan, plan?.win_condition) ?? '' },
    { key: 'fallback', label: t('heroCard.pillarFallback'), text: firstText(rec.fallback_plan, plan?.comp_clash_note) ?? '' },
    { key: 'risk', label: t('heroCard.pillarRisk'), text: lossRiskText ?? '', tone: 'risk' },
    { key: 'data', label: t('heroCard.pillarData'), text: dataSummary ?? '', tone: 'data' },
  ];
  return raw.filter((p) => p.text.trim().length > 0);
}

/** The deep-dive surface — the coach read + score breakdown + why-not + data sources
 *  + phase numbers + team-need/threats/combos + the move simulations. Lifted out of the
 *  HeroCard's in-card overlay into one shared depth tab. */
export const DeepDiveTab = React.memo(function DeepDiveTab({ rec, draftSimulation, draftFork }: Props) {
  const { t } = useTranslation();
  const plan = rec.draft_plan;
  const coachPillars = buildCoachPillars(rec, t);

  return (
    <div className="hero-detail-panel hero-detail-panel--tab">
      {/* FAZ 4 / Sprint 1: grounded koçluk notu (deterministik ya da audit'i
          geçmiş LLM adayı; tüm sinyalleri tek paragrafta sentezler). */}
      {rec.coach_narrative?.text && (
        <div className="hero-detail-section">
          <span className="hero-card__plan-label">{t('heroCard.coachNote')}</span>
          <div className="hero-detail-coach-card hero-detail-coach-card--neutral">
            <p>{rec.coach_narrative.text}</p>
          </div>
        </div>
      )}

      {coachPillars.length > 0 && (
        <div className="hero-detail-section">
          <span className="hero-card__plan-label">{t('heroCard.detailCoachTitle')}</span>
          <div className="hero-detail-coach-grid">
            {coachPillars.map((pillar) => (
              <div
                key={pillar.key}
                className={`hero-detail-coach-card hero-detail-coach-card--${pillar.tone ?? 'neutral'}`}
              >
                <span className="hero-detail-coach-label">{pillar.label}</span>
                <p>{pillar.text}</p>
              </div>
            ))}
          </div>
        </div>
      )}

      <div className="hero-detail-scores">
        <StatBar label={t('heroCard.scoreTeamCounter')} value={rec.team_counter_score} />
      </div>

      {rec.score_breakdown && rec.score_breakdown.length > 0 && (
        <div className="hero-detail-section">
          <span className="hero-card__plan-label">{t('heroCard.scoreBreakdown', { defaultValue: 'Skor kırılımı' })}</span>
          <ul className="hero-detail-threats">
            {/* B3: TÜM sinyaller (kırpma yok) — şeffaflık tam olsun. */}
            {rec.score_breakdown.map((item) => (
              <li key={item.key} className="hero-detail-threat">
                <strong>{item.label}:</strong> {Math.round(item.value * 100)}%
                {' · '}
                {t('heroCard.confidenceShort', { defaultValue: 'güven' })}: {item.confidence}
              </li>
            ))}
          </ul>
          {rec.confidence_basis && (
            <p className="hero-detail-basis">
              {t(`heroCard.confidenceBasis.${rec.confidence_basis}`, {
                defaultValue: rec.confidence_basis,
              })}
            </p>
          )}
          {rec.missing_signals && rec.missing_signals.length > 0 && (
            <p className="hero-detail-basis">
              {t('heroCard.missingSignals', {
                signals: rec.missing_signals
                  .map((s) => t(`champSelect.missingSignal.${s}`, { defaultValue: s }))
                  .join(' · '),
              })}
            </p>
          )}
        </div>
      )}

      {rec.why_not && rec.why_not.length > 0 && (
        <div className="hero-detail-section">
          <span className="hero-card__plan-label">{t('heroCard.whyNot', { defaultValue: 'Neden dikkat?' })}</span>
          <ul className="hero-detail-threats">
            {rec.why_not.map((item, i) => (
              <li key={i} className="hero-detail-threat">{item}</li>
            ))}
          </ul>
        </div>
      )}

      {rec.data_sources && rec.data_sources.length > 0 && (
        <div className="hero-detail-section">
          <span className="hero-card__plan-label">{t('heroCard.dataSources', { defaultValue: 'Veri kaynaklari' })}</span>
          <div className="hero-detail-sources">
            {rec.data_sources.slice(0, 4).map((source) => (
              <div key={`${source.source}-${source.patch ?? ''}`} className="hero-detail-source">
                <strong>{source.source.replace(/_/g, ' ')}</strong>
                <span>{source.confidence} - risk {source.risk_level}</span>
                {(source.patch || source.sample_size != null || source.updated_at != null) && (
                  <small>
                    {[
                      source.patch,
                      source.sample_size != null ? `n=${source.sample_size}` : '',
                      formatSourceTime(source.updated_at),
                    ].filter(Boolean).join(' - ')}
                  </small>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {rec.phase_matchup && (
        <p className="hero-detail-phase">
          {t('heroCard.phaseLine', {
            early: Math.round(rec.phase_matchup[0] * 100),
            mid: Math.round(rec.phase_matchup[1] * 100),
            late: Math.round(rec.phase_matchup[2] * 100),
          })}
        </p>
      )}

      {plan && plan.fills_team_need.length > 0 && (
        <div className="hero-detail-section">
          <span className="hero-card__plan-label">{t('draftPlan.teamNeed')}</span>
          <div className="hero-card__quick-tags">
            {plan.fills_team_need.map((fill, i) => (
              <span key={i} className="hero-card__quick-tag hero-card__quick-tag--safe">{fill}</span>
            ))}
          </div>
        </div>
      )}

      {plan && plan.threats.length > 0 && (
        <div className="hero-detail-section">
          <span className="hero-card__plan-label">{t('draftPlan.risks')}</span>
          <ul className="hero-detail-threats">
            {plan.threats.map((threat, i) => (
              <li key={i} className="hero-detail-threat">{threat}</li>
            ))}
          </ul>
        </div>
      )}

      {plan && plan.combo_with.length > 0 && (
        <div className="hero-detail-section">
          <span className="hero-card__plan-label">{t('draftPlan.comboPlans')}</span>
          <ul className="hero-detail-threats">
            {plan.combo_with.map((h, i) => (
              <li key={i} className="hero-detail-threat">
                <strong>{h.ally_champion_key}:</strong> {h.combo_text}
                {h.ability_ref && (
                  <details className="draft-plan__combo-detail">
                    <summary>{t('draftPlan.comboMechanics')}</summary>
                    <span>{h.ability_ref}</span>
                  </details>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}

      {plan?.pick_window_note && (
        <div className="hero-detail-section">
          <span className="hero-card__plan-label">{t('draftPlan.pickPosition')}</span>
          <p className="hero-detail-threat">{plan.pick_window_note}</p>
        </div>
      )}

      {(rec.enemy_team_summary ?? '') !== '' && (
        <p className="hero-detail-enemy">{t('heroCard.enemyLabel', { summary: rec.enemy_team_summary })}</p>
      )}

      <DraftSimulatorPanel results={draftSimulation} />
      <DraftForkPanel fork={draftFork} />
    </div>
  );
});
