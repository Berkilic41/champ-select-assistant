import React from 'react';
import { useTranslation } from 'react-i18next';
import type { PerformanceReport } from '../../types/generated/PerformanceReport';
import { ChampionIcon } from '../shared/ChampionIcon';
import './PerformancePanel.css';

interface Props {
  report?: PerformanceReport | null;
}

function pct(value: number): string {
  return `${Math.round(value * 100)}%`;
}

function signedPct(value: number): string {
  const rounded = Math.round(value * 100);
  return `${rounded > 0 ? '+' : ''}${rounded}%`;
}

const ROLE_KEYS: Record<string, string> = {
  top: 'top',
  jungle: 'jungle',
  middle: 'middle',
  bottom: 'bottom',
  utility: 'utility',
};

export const PerformancePanel: React.FC<Props> = ({ report }) => {
  const { t } = useTranslation();
  if (!report) return null;

  const roleKey = ROLE_KEYS[report.main_role];
  const mainRole = roleKey
    ? t(`champSelect.roles.${roleKey}`)
    : t('performance.unknownRole');

  return (
    <section className="performance-panel" aria-label={t('performance.eyebrow')}>
      <div className="performance-panel__head">
        <div>
          <p className="performance-panel__eyebrow">{t('performance.eyebrow')}</p>
          <h3 className="performance-panel__title">{t('performance.title')}</h3>
        </div>
        {report.partial && <span className="performance-panel__badge">{t('performance.thinData')}</span>}
        {report.tilt_warning && (
          <span className="performance-panel__badge performance-panel__badge--risk">
            {t('performance.tiltRisk')}
          </span>
        )}
      </div>

      <p className="performance-panel__lesson">{report.main_lesson}</p>

      <div className="performance-panel__metrics">
        <span>{t('performance.games', { n: report.games })}</span>
        <span>{pct(report.win_rate)} WR</span>
        <span>{report.avg_kda.toFixed(1)} KDA</span>
        {report.avg_cs_per_min != null && (
          <span>{t('performance.csPerMin', { cs: report.avg_cs_per_min.toFixed(1) })}</span>
        )}
        {report.avg_cs_at_10 != null && (
          <span>{t('performance.csAt10', { cs: report.avg_cs_at_10.toFixed(0) })}</span>
        )}
        {report.avg_deaths_pre_14 != null && (
          <span>{t('performance.earlyDeaths', { n: report.avg_deaths_pre_14.toFixed(1) })}</span>
        )}
        {report.avg_vision_score != null && (
          <span>{t('performance.vision', { n: report.avg_vision_score.toFixed(0) })}</span>
        )}
        <span>{mainRole}</span>
        {report.games >= 10 && (
          <span>{t('performance.form', { delta: signedPct(report.form_delta) })}</span>
        )}
      </div>

      {report.top_champions.length > 0 && (
        <div className="performance-panel__champions">
          {report.top_champions.map((champion) => (
            <div key={champion.champion_id} className="performance-panel__champion">
              <ChampionIcon championKey={champion.champion_key} size="sm" />
              <div className="performance-panel__champion-copy">
                <strong>{champion.champion_key || `#${champion.champion_id}`}</strong>
                <span>
                  {t('performance.champLine', {
                    games: champion.games,
                    wr: pct(champion.win_rate),
                    kda: champion.avg_kda.toFixed(1),
                  })}
                </span>
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
};
