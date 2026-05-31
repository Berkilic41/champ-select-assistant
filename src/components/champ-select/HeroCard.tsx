import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { DraftPlan, Recommendation } from '../../types/recommendation';
import { ChampionIcon } from '../shared/ChampionIcon';
import { TierBadge } from '../../lib/tier';
import { splashUrl } from '../../lib/ddragon';
import { DraftPlanPanel } from './DraftPlanPanel';
import './HeroCard.css';

interface QuickTagsProps {
  plan?: DraftPlan;
  phaseMatchup?: [number, number, number];
}

const QuickTags: React.FC<QuickTagsProps> = ({ plan, phaseMatchup }) => {
  const { t } = useTranslation();
  if (!plan) return null;

  type Variant = 'risk' | 'safe' | 'info';
  const chips: Array<{ text: string; variant: Variant }> = [];

  // 1. Stretch pick risk — always show first
  if (plan.risk_note) {
    chips.push({ text: plan.risk_note, variant: 'risk' });
  }
  // 2. Early-phase lane dominance
  if (chips.length < 3 && phaseMatchup && phaseMatchup[0] >= 0.65) {
    chips.push({ text: t('heroCard.earlyAdvantage'), variant: 'safe' });
  }
  // 3. Counter-pick window (late pick + counters visible enemy)
  if (chips.length < 3 && plan.pick_window_note) {
    chips.push({ text: plan.pick_window_note, variant: 'safe' });
  }
  // 4. Combo hint
  if (chips.length < 3 && plan.combo_with.length > 0) {
    chips.push({ text: t('heroCard.comboStrong', { ally: plan.combo_with[0].ally_champion_key }), variant: 'info' });
  }
  // 5. Team gap fill OR blind safety
  if (chips.length < 3) {
    if (plan.fills_team_need.length > 0) {
      chips.push({ text: plan.fills_team_need[0], variant: 'info' });
    } else if (plan.blind_pick_safety >= 0.75) {
      chips.push({ text: t('heroCard.blindSafe'), variant: 'safe' });
    }
  }

  if (chips.length === 0) return null;

  return (
    <div className="hero-card__quick-tags">
      {chips.map((chip, i) => (
        <span key={i} className={`hero-card__quick-tag hero-card__quick-tag--${chip.variant}`}>
          {chip.text}
        </span>
      ))}
    </div>
  );
};

interface Props {
  rec: Recommendation;
  onHover?: () => void;
  onExpand?: () => void;
}

interface ScoreBarProps {
  label: string;
  value: number;
}

const ScoreBar: React.FC<ScoreBarProps> = ({ label, value }) => (
  <div className="score-bar-row">
    <span className="score-bar-label">{label}</span>
    <div className="score-bar-track">
      <div
        className="score-bar-fill"
        style={{ width: `${Math.round(Math.max(0, Math.min(1, value)) * 100)}%` }}
      />
    </div>
    <span className="score-bar-pct">{Math.round(Math.max(0, Math.min(1, value)) * 100)}%</span>
  </div>
);

function confidenceDots(conf: string, games: number): string {
  if (conf === 'high' || games >= 20) return '●●●●●';
  if (conf === 'medium' || games >= 5) return '●●●○○';
  return '●○○○○';
}

function personalStatsLine(rec: Recommendation, t: TFunction): string {
  if (rec.games_on_champ === 0) return t('heroCard.firstTime');
  const wr = rec.wins_on_champ / rec.games_on_champ;
  return t('heroCard.statsLine', {
    wins: rec.wins_on_champ,
    losses: rec.games_on_champ - rec.wins_on_champ,
    wr: Math.round(wr * 100),
    games: rec.games_on_champ,
  });
}

export const HeroCard: React.FC<Props> = ({ rec, onHover, onExpand }) => {
  const { t } = useTranslation();
  const [imgError, setImgError] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const splashSrc = !imgError ? splashUrl(rec.champion_key) : undefined;

  function handleExpandClick() {
    setExpanded(prev => !prev);
    onExpand?.();
  }

  return (
    <div className="hero-card animate-hero-in">
      {/* Splash art arka plan */}
      {splashSrc && (
        <div
          className="hero-card__splash"
          style={{ backgroundImage: `url(${splashSrc})` }}
        >
          <img
            src={splashSrc}
            alt=""
            style={{ display: 'none' }}
            onError={() => setImgError(true)}
          />
        </div>
      )}
      <div className="hero-card__overlay" />

      {/* İçerik */}
      <div className="hero-card__content">
        <div className="hero-card__top">
          <TierBadge tier={rec.tier} large />
          <span className="hero-card__confidence">{confidenceDots(rec.confidence, rec.games_on_champ)}</span>
        </div>
        {rec.confidence === 'low' && (
          <div className="hero-card__low-confidence">{t('heroCard.lowConfidence')}</div>
        )}

        <div className="hero-card__body">
          <ChampionIcon championKey={rec.champion_key} size="md" />
          <div className="hero-card__info">
            <h2 className="hero-card__name">{rec.champion_name || rec.champion_key}</h2>
            <p className="hero-card__reason">{rec.reason}</p>
          </div>
        </div>

        <div className="hero-card__stats">{personalStatsLine(rec, t)}</div>

        <QuickTags plan={rec.draft_plan} phaseMatchup={rec.phase_matchup} />

        <div className="hero-card__footer">
          <span className="hero-card__key-hint">{t('heroCard.selectHint')}</span>
          <div className="hero-card__footer-actions">
            {onHover && (
              <button
                className="hero-card__hover-btn"
                onClick={onHover}
                type="button"
              >
                {t('heroCard.hoverApply')}
              </button>
            )}
            <button
              className={`hero-card__expand${expanded ? ' hero-card__expand--open' : ''}`}
              onClick={handleExpandClick}
              type="button"
              aria-expanded={expanded}
            >
              {expanded ? '↑' : t('heroCard.detailBtn')}
            </button>
          </div>
        </div>

        {/* Score bar */}
        <div className="hero-card__score-bar">
          <div
            className="hero-card__score-fill"
            style={{ '--target-width': `${Math.round(rec.total_score * 100)}%` } as React.CSSProperties}
          />
        </div>
      </div>

      {/* Detail overlay */}
      {expanded && (
        <div
          className="hero-detail-overlay"
          onClick={() => setExpanded(false)}
        >
          <div
            className="hero-detail-panel"
            onClick={e => e.stopPropagation()}
          >
            <div className="hero-detail-scores">
              <ScoreBar label={t('heroCard.scoreMatchup')}     value={rec.matchup_score} />
              <ScoreBar label={t('heroCard.scoreTeamCounter')} value={rec.team_counter_score} />
              <ScoreBar label={t('heroCard.scoreSynergy')}     value={rec.synergy_score} />
              <ScoreBar label={t('heroCard.scoreMeta')}        value={rec.meta_score} />
            </div>
            {rec.phase_matchup && (
              <p className="hero-detail-phase">
                {t('heroCard.phaseLine', {
                  early: Math.round(rec.phase_matchup[0] * 100),
                  mid: Math.round(rec.phase_matchup[1] * 100),
                  late: Math.round(rec.phase_matchup[2] * 100),
                })}
              </p>
            )}
            <DraftPlanPanel plan={rec.draft_plan} />
            {(rec.enemy_team_summary ?? '') !== '' && (
              <p className="hero-detail-enemy">{t('heroCard.enemyLabel', { summary: rec.enemy_team_summary })}</p>
            )}
          </div>
        </div>
      )}
    </div>
  );
};
