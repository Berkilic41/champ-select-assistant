import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CalendarCheck } from 'lucide-react';
import { invoke } from '../../lib/host';
import { useActiveSummonerPuuid } from '../../hooks/useActiveSummonerPuuid';
import './GameReviewCard.css';

interface WeeklySummary {
  goals_met: number;
  goals_missed: number;
  hit_rate: number | null;
  games: number;
  wins: number;
  reviews: number;
}

/** F3: haftalık koç özeti — son 7 günün hedef isabeti + W/L + karne sayısı. */
export const WeeklySummaryCard: React.FC = () => {
  const { t } = useTranslation();
  const puuid = useActiveSummonerPuuid();
  const [summary, setSummary] = useState<WeeklySummary | null>(null);
  const [error, setError] = useState(false);

  useEffect(() => {
    if (!puuid) return;
    let alive = true;
    (async () => {
      try {
        const res = await invoke<WeeklySummary>('get_weekly_summary', { puuid });
        if (alive) setSummary(res);
      } catch {
        if (alive) setError(true);
      }
    })();
    return () => {
      alive = false;
    };
  }, [puuid]);

  if (error) {
    return (
      <div className="grc-card">
        <div className="grc-head">
          <CalendarCheck size={14} />
          <span className="grc-title">{t('weekly.title')}</span>
        </div>
        <p className="grc-baseline">{t('app.dataError')}</p>
      </div>
    );
  }
  if (!summary || summary.games === 0) return null;

  return (
    <div className="grc-card">
      <div className="grc-head">
        <CalendarCheck size={14} />
        <span className="grc-title">{t('weekly.title')}</span>
        <span className="grc-chip">
          {t('weekly.record', { w: summary.wins, l: summary.games - summary.wins })}
        </span>
      </div>
      <ul className="grc-lines">
        <li className="grc-line">
          <span className="grc-metric">{t('weekly.goals')}</span>
          <span className="grc-value">
            {summary.hit_rate !== null
              ? t('weekly.hitRate', {
                  met: summary.goals_met,
                  total: summary.goals_met + summary.goals_missed,
                  pct: Math.round(summary.hit_rate * 100),
                })
              : t('weekly.noGoals')}
          </span>
        </li>
        <li className="grc-line">
          <span className="grc-metric">{t('weekly.reviews')}</span>
          <span className="grc-value">{summary.reviews}</span>
        </li>
      </ul>
    </div>
  );
};
