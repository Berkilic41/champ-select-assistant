import React from 'react';
import { useTranslation } from 'react-i18next';
import { BanSuggestion, EnemyPoolSummary } from '../../types/recommendation';
import type { ScoutingReport } from '../../types/generated/ScoutingReport';
import { ChampionIcon } from '../shared/ChampionIcon';
import './BanSuggestionList.css';

interface Props {
  suggestions: BanSuggestion[];
  enemyPools?: EnemyPoolSummary[];
  scoutingReport?: ScoutingReport | null;
}

export const BanSuggestionList: React.FC<Props> = ({ suggestions, enemyPools = [], scoutingReport }) => {
  const { t } = useTranslation();
  const hasScouting = Boolean(
    scoutingReport && (scoutingReport.ban_targets.length > 0 || scoutingReport.enemies.length > 0),
  );
  const confidenceLabel = (confidence: string): string => {
    const key = confidence === 'high' || confidence === 'medium' || confidence === 'low'
      ? confidence
      : 'unknown';
    return t(`champSelect.confidence.${key}`);
  };
  const threatLabel = (level: string, fallback: string): string => {
    if (level === 'high' || level === 'medium' || level === 'low') {
      return t(`champSelect.threat.${level}`);
    }
    return fallback;
  };

  if (!suggestions.length && !hasScouting) return (
    <p className="ban-suggestion-empty">{t('champSelect.banComputing')}</p>
  );

  const otpMap = new Map(enemyPools.map(p => [p.top_champion_id, p]));

  return (
    <div className="ban-suggestion-list">
      {hasScouting && scoutingReport && (
        <div className="ban-scouting-card">
          <div className="ban-scouting-head">
            <p className="ban-scouting-title">{t('champSelect.scoutingTitle')}</p>
            {scoutingReport.partial && (
              <span className="ban-scouting-partial">{t('champSelect.scoutingPartial')}</span>
            )}
          </div>

          <div className="ban-scouting-section ban-scouting-teamread">
            <span className="ban-scouting-label">{t('champSelect.scoutGameplan')}</span>
            <span className="ban-scouting-wincon">
              {t(`champSelect.scoutWinCondition.${scoutingReport.team_read.win_condition}`, {
                defaultValue: scoutingReport.team_read.win_condition,
              })}
            </span>
            <span className="ban-scouting-reason">{scoutingReport.team_read.strategy_note}</span>
            {scoutingReport.team_read.primary_threat_key && (
              <span className="ban-scouting-tags">
                {t('champSelect.scoutPrimaryThreat', {
                  champ: scoutingReport.team_read.primary_threat_key,
                })}
              </span>
            )}
          </div>

          {scoutingReport.ban_targets.length > 0 && (
            <div className="ban-scouting-section">
              <span className="ban-scouting-label">{t('champSelect.scoutingBanTargets')}</span>
              {scoutingReport.ban_targets.slice(0, 3).map(target => (
                <div key={target.champion_id} className="ban-scouting-target">
                  <ChampionIcon championKey={target.champion_key} size="sm" />
                  <div className="ban-scouting-copy">
                    <span className="ban-scouting-name">{target.champion_key}</span>
                    <span className="ban-scouting-reason">{target.reason}</span>
                  </div>
                  <span className="ban-scouting-confidence">{confidenceLabel(target.confidence)}</span>
                </div>
              ))}
            </div>
          )}

          {scoutingReport.enemies.length > 0 && (
            <div className="ban-scouting-section">
              <span className="ban-scouting-label">{t('champSelect.scoutingProfiles')}</span>
              <div className="ban-scouting-profiles">
                {scoutingReport.enemies.slice(0, 3).map(enemy => (
                  <div key={`${enemy.cell_id}-${enemy.top_champion_id}`} className="ban-scouting-profile">
                    <ChampionIcon championKey={enemy.top_champion_key} size="sm" />
                    <div className="ban-scouting-copy">
                      <span className="ban-scouting-name">{enemy.top_champion_key}</span>
                      <span className="ban-scouting-reason">{enemy.note}</span>
                      {enemy.playstyle_tags.length > 0 && (
                        <span className="ban-scouting-tags">
                          {enemy.playstyle_tags.slice(0, 3).join(' · ')}
                        </span>
                      )}
                    </div>
                    <span className="ban-scouting-threat">
                      {threatLabel(enemy.threat_level, enemy.threat)}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {!suggestions.length && (
        <p className="ban-suggestion-empty">{t('champSelect.banComputing')}</p>
      )}

      {suggestions.length > 0 && (
        <>
      <p className="ban-suggestion-title">{t('champSelect.banSuggestions')}</p>
      {suggestions.slice(0, 3).map((s, i) => {
        const otp = otpMap.get(s.champion_id);
        const otpNote = otp && otp.play_rate >= 0.40
          ? t('champSelect.banOtp', { pct: Math.round(otp.play_rate * 100) })
          : null;
        return (
          <div key={s.champion_id} className="ban-suggestion-row">
            <span className="ban-suggestion-rank">{i + 1}</span>
            <ChampionIcon championKey={s.champion_key} size="sm" />
            <div className="ban-suggestion-body">
              <div className="ban-suggestion-head">
                <span className="ban-suggestion-name">{s.champion_name || s.champion_key}</span>
                <span className="ban-suggestion-threat">{Math.round(s.threat_score * 100)}%</span>
              </div>
              <span className="ban-suggestion-reason">
                {s.reason}
                {otpNote && <span className="ban-suggestion-otp"> · {otpNote}</span>}
              </span>
            </div>
          </div>
        );
      })}
        </>
      )}
    </div>
  );
};
