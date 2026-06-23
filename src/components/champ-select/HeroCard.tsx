import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { Recommendation } from '../../types/recommendation';
import { ChampionIcon } from '../shared/ChampionIcon';
import { TierBadge } from '../../lib/tier';
import { ConfidenceRing, StatBar, Button } from '../shared/ui';
import { splashUrl } from '../../lib/ddragon';
import './HeroCard.css';

interface Props {
  rec: Recommendation;
  onHover?: () => void;
  onExpand?: () => void;
  /** Render as the locked/finalized pick: hides the "[1-5] select" hint (there's
   *  nothing to choose anymore). Used by the post-lock view for one unified card. */
  pinned?: boolean;
}

interface PlanLine {
  key: string;
  label: string;
  text: string;
  tone?: 'neutral' | 'risk' | 'safe';
}

function personalStatsLine(rec: Recommendation, t: TFunction): string {
  if (rec.games_on_champ === 0) {
    // High mastery but no recent ranked games: honest split — the player can pilot
    // the kit, there's just no draft win-rate signal. Beats a misleading "first
    // time" for a high-mastery OTP who hasn't queued it this season.
    if ((rec.mechanical_comfort ?? 0) >= 0.5) return t('heroCard.mechanicalNoDraft');
    return t('heroCard.firstTime');
  }
  const wr = rec.wins_on_champ / rec.games_on_champ;
  return t('heroCard.statsLine', {
    wins: rec.wins_on_champ,
    losses: rec.games_on_champ - rec.wins_on_champ,
    wr: Math.round(wr * 100),
    games: rec.games_on_champ,
  });
}

function firstText(...values: Array<string | null | undefined>): string | null {
  for (const value of values) {
    const clean = (value ?? '').trim();
    if (clean) return clean;
  }
  return null;
}

/**
 * Decision card: everything you need in the 30-second pick window is visible
 * WITHOUT clicking — why this champion, the score breakdown, the game plan, and
 * (below, via BuildSummary) the build. "Detay" opens the Deep-Dive tab (one depth
 * surface) for the extras (coach read, data sources, threats, phase numbers).
 */
export const HeroCard: React.FC<Props> = ({ rec, onHover, onExpand, pinned }) => {
  const { t } = useTranslation();
  const [imgError, setImgError] = useState(false);
  const splashSrc = !imgError ? splashUrl(rec.champion_key) : undefined;
  const plan = rec.draft_plan;

  // Inline game-plan essentials — the handful of lines that drive the decision.
  const planLines: PlanLine[] = [];
  const pushPlan = (
    key: string,
    label: string,
    line?: string | null,
    tone: PlanLine['tone'] = 'neutral',
  ) => {
    const clean = (line ?? '').trim();
    if (clean && !planLines.some((item) => item.text === clean)) {
      planLines.push({ key, label, text: clean, tone });
    }
  };
  pushPlan('lane', t('heroCard.planLane'), rec.lane_plan);
  pushPlan('mid', t('heroCard.planMid'), rec.mid_game_plan);
  pushPlan('teamfight', t('heroCard.planFight'), rec.teamfight_job ?? rec.teamfight_plan);
  pushPlan('fallback', t('heroCard.planFallback'), rec.fallback_plan, 'safe');
  if (plan) {
    pushPlan('win', t('heroCard.planWin'), plan.win_condition);
    pushPlan('spike', t('heroCard.planSpike'), plan.spike_note);
    if (plan.combo_with.length > 0) {
      const c = plan.combo_with[0];
      // 3C: birincil combo müttefikiyle co-pick geçmişin (≥2 maç; host iliştirir).
      const hist = rec.combo_history;
      const histSuffix =
        hist && hist.games >= 2
          ? ` · ${t('heroCard.comboHistory', {
              n: hist.games,
              wr: Math.round((hist.wins / hist.games) * 100),
            })}`
          : '';
      pushPlan('combo', t('heroCard.planCombo'), `${c.ally_champion_key}: ${c.combo_text}${histSuffix}`);
    }
    pushPlan('lane-advice', t('heroCard.planLane'), plan.lane_phase_advice);
    pushPlan('clash', t('heroCard.planClash'), plan.comp_clash_note, 'risk');
  }

  const decisionText = (rec.decision_sentence ?? '').trim() || rec.reason;
  // Crisp hero line: the punchy headline if present, else the full decision sentence.
  const headlineText = (rec.headline ?? '').trim() || decisionText;
  const lossRiskText = firstText(rec.risk_summary, plan?.risk_note, plan?.threats?.[0]);

  return (
    <div className="hero-card animate-hero-in">
      {/* Splash art background */}
      {splashSrc && (
        <div className="hero-card__splash" style={{ backgroundImage: `url(${splashSrc})` }}>
          <img src={splashSrc} alt="" style={{ display: 'none' }} onError={() => setImgError(true)} />
        </div>
      )}
      <div className="hero-card__overlay" />

      <div className="hero-card__content">
        {/* THE answer: champion + grounded why headline + the confidence ring */}
        <div className="hero-card__headline">
          <ChampionIcon championKey={rec.champion_key} size="md" />
          <div className="hero-card__info">
            <div className="hero-card__name-row">
              <h2 className="hero-card__name">{rec.champion_name || rec.champion_key}</h2>
              <TierBadge tier={rec.tier} large />
              {rec.pro_presence != null && rec.pro_presence > 0 && (
                <span
                  className="hero-card__pro"
                  title={t('heroCard.proHeatHint', {
                    defaultValue: 'Pro sahnesinde pick + ban oranı (Leaguepedia)',
                  })}
                >
                  {t('heroCard.proHeat', {
                    pct: Math.round(rec.pro_presence * 100),
                    defaultValue: 'Pro %{{pct}}',
                  })}
                </span>
              )}
            </div>
            <p className="hero-card__reason">{headlineText}</p>
            <span className="hero-card__stats">{personalStatsLine(rec, t)}</span>
            {rec.win_prob && (
              <span
                className={`hero-card__winprob hero-card__winprob--${rec.win_prob.confidence}`}
                title={t('heroCard.winProbHint', { conf: rec.win_prob.confidence })}
              >
                {t('heroCard.winProb', {
                  pct: Math.round(rec.win_prob.probability * 100),
                  n: rec.win_prob.sample_size,
                })}
              </span>
            )}
          </div>
          <ConfidenceRing
            score={rec.total_score}
            confidence={rec.confidence}
            size={68}
            ariaLabel={t('heroCard.confidenceRingLabel', {
              defaultValue: 'Skor %{{pct}}',
              pct: Math.round(rec.total_score * 100),
            })}
          />
        </div>
        {rec.confidence === 'low' && (
          <div className="hero-card__low-confidence">{t('heroCard.lowConfidence')}</div>
        )}

        {/* Honest "missing data" flags — this pick lacks real X data (no fake score) */}
        {rec.missing_signals && rec.missing_signals.length > 0 && (
          <div className="hero-card__missing">
            {rec.missing_signals.map((sig) => (
              <span key={sig} className="hero-card__missing-chip">
                {t(`champSelect.missingSignal.${sig}`, { defaultValue: sig })}
              </span>
            ))}
          </div>
        )}

        {/* Score breakdown — inline, no click needed */}
        <div className="hero-card__scores">
          <StatBar label={t('heroCard.scoreMatchup')} value={rec.matchup_score} />
          <StatBar label={t('heroCard.scoreSynergy')} value={rec.synergy_score} />
          <StatBar label={t('heroCard.scoreMeta')} value={rec.meta_score} />
        </div>

        {/* Team role identity — core-computed "what job for the team" (already
            shown in-game); names the role the synergy_score quantifies, so the
            player knows their team job BEFORE locking. */}
        {plan?.team_role?.trim() && (
          <div className="hero-card__team-role">
            <span className="hero-card__team-role-label">{t('heroCard.teamRole')}</span>
            <span>{plan.team_role.trim()}</span>
          </div>
        )}

        {/* Game plan — inline */}
        {planLines.length > 0 && (
          <div className="hero-card__plan">
            <span className="hero-card__plan-label">{t('heroCard.planLabel')}</span>
            <ul className="hero-card__plan-list">
              {planLines.slice(0, 5).map((line) => (
                <li
                  key={`${line.key}-${line.text}`}
                  className={`hero-card__plan-item hero-card__plan-item--${line.tone ?? 'neutral'}`}
                >
                  <span className="hero-card__plan-item-label">{line.label}</span>
                  <span>{line.text}</span>
                </li>
              ))}
            </ul>
          </div>
        )}
        {lossRiskText && (
          <div className="hero-card__risk" role="alert">
            <span className="hero-card__risk-label">{t('heroCard.lossRiskLabel')}</span>
            <span>{lossRiskText}</span>
          </div>
        )}

        <div className="hero-card__footer">
          {!pinned && <span className="hero-card__key-hint">{t('heroCard.selectHint')}</span>}
          <div className="hero-card__footer-actions">
            {onHover && (
              <Button variant="primary" onClick={onHover}>
                {t('heroCard.hoverApply')}
              </Button>
            )}
            <Button variant="ghost" onClick={() => onExpand?.()}>
              {t('heroCard.detailBtn')}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
};
