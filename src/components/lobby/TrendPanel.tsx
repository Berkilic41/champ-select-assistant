import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { LineChart, Line, ResponsiveContainer } from 'recharts';
import { TrendingUp, TrendingDown, Minus } from 'lucide-react';
import { invoke } from '../../lib/host';
import type { TrendReport } from '../../types/generated/TrendReport';
import './GameReviewCard.css';

interface TrendResult {
  report: TrendReport;
  role: string;
  queue_group: string;
  games: number;
}

const SPARK_METRICS = ['cs_per_min', 'deaths_per_10', 'vision_score'] as const;

/**
 * Trend panosu (Faz C4): baskın rol + queue-grubundaki son maçların sparkline'ı
 * + yarı-medyan yön hükümleri (core'dan). < 8 maçta dürüst "ince veri" notu.
 */
export const TrendPanel: React.FC = () => {
  const { t } = useTranslation();
  const [data, setData] = useState<TrendResult | null>(null);

  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const puuid = await invoke<string | null>('get_active_summoner_puuid');
        if (!puuid) return;
        const res = await invoke<TrendResult | null>('get_trend_report', { puuid });
        if (alive && res) setData(res);
      } catch {
        /* trend üretilemedi — panel gizli kalır */
      }
    })();
    return () => {
      alive = false;
    };
  }, []);

  if (!data || data.report.points.length < 3) return null;
  const { report } = data;

  const dirIcon = (d: string) =>
    d === 'improving' ? (
      <TrendingUp size={12} className="grc-up" />
    ) : d === 'declining' ? (
      <TrendingDown size={12} className="grc-down" />
    ) : (
      <Minus size={12} className="grc-even" />
    );

  const verdictFor = (metric: string) => report.verdicts.find((v) => v.metric === metric);

  return (
    <div className="grc-card">
      <div className="grc-head">
        <span className="grc-title">{t('trend.title')}</span>
        <span className="grc-chip">
          {t(`review.queue.${data.queue_group}`, { defaultValue: data.queue_group })}
          {data.role ? ` · ${t(`overlay.position.${data.role}`, { defaultValue: data.role })}` : ''}
        </span>
        <span className="grc-chip">{t('trend.games', { n: data.games })}</span>
      </div>

      {SPARK_METRICS.map((metric) => {
        const points = report.points
          .map((p, i) => ({ i, v: p[metric] ?? null }))
          .filter((p) => p.v !== null);
        if (points.length < 3) return null;
        const verdict = verdictFor(metric);
        return (
          <div key={metric} className="grc-trend-row">
            <span className="grc-metric">
              {t(`review.metric.${metric}`, { defaultValue: metric })}
            </span>
            <div className="grc-spark">
              <ResponsiveContainer width="100%" height={28}>
                <LineChart data={points}>
                  <Line
                    type="monotone"
                    dataKey="v"
                    stroke="var(--gold, #c89b3c)"
                    strokeWidth={1.5}
                    dot={false}
                    isAnimationActive={false}
                  />
                </LineChart>
              </ResponsiveContainer>
            </div>
            {verdict && (
              <span className="grc-trend-verdict" title={`${verdict.first_half} → ${verdict.second_half}`}>
                {dirIcon(verdict.direction)}
                {t(`trend.${verdict.direction}`)}
              </span>
            )}
          </div>
        );
      })}

      {verdictFor('win_rate') && (
        <div className="grc-trend-row">
          <span className="grc-metric">{t('trend.winRate')}</span>
          <span className="grc-value">
            {verdictFor('win_rate')!.first_half}% → {verdictFor('win_rate')!.second_half}%
          </span>
          <span className="grc-trend-verdict">
            {dirIcon(verdictFor('win_rate')!.direction)}
            {t(`trend.${verdictFor('win_rate')!.direction}`)}
          </span>
        </div>
      )}

      {report.partial && <p className="grc-partial">{t('trend.partial')}</p>}
    </div>
  );
};
