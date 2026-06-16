import React from 'react';
import { useTranslation } from 'react-i18next';
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';
import { ChampionStats, MasteryEntry } from '../../types/riot';
import { GameReviewCard } from './GameReviewCard';
import { RankCard } from './RankCard';
import { TrendPanel } from './TrendPanel';
import { WeeklySummaryCard } from './WeeklySummaryCard';
import './StatsView.css';

interface Props {
  stats: ChampionStats[];
  masteries: MasteryEntry[];
  champMap: Map<number, string>;
}

export const StatsView: React.FC<Props> = ({ stats, masteries, champMap }) => {
  const { t } = useTranslation();
  const masteryData = masteries
    .slice()
    .sort((a, b) => b.mastery_points - a.mastery_points)
    .slice(0, 10)
    .map(m => ({
      name: champMap.get(m.champion_id) ?? `#${m.champion_id}`,
      mastery: m.mastery_points,
    }));

  const wrData = stats
    .filter(s => s.games >= 3)
    .slice()
    .sort((a, b) => b.games - a.games)
    .slice(0, 10)
    .map(s => ({
      name: champMap.get(s.champion_id) ?? `#${s.champion_id}`,
      wr: Math.round((s.wins / s.games) * 100),
      games: s.games,
    }));

  const totalGames = stats.reduce((acc, s) => acc + s.games, 0);
  const totalWins = stats.reduce((acc, s) => acc + s.wins, 0);
  const overallWinRate = totalGames > 0 ? Math.round((totalWins / totalGames) * 100) : 0;
  const top5Keys = masteries
    .slice()
    .sort((a, b) => b.mastery_points - a.mastery_points)
    .slice(0, 5)
    .map(m => champMap.get(m.champion_id) ?? `#${m.champion_id}`);

  if (!masteries.length && !stats.length) {
    return <p className="stats-empty">{t('stats.empty')}</p>;
  }

  return (
    <div className="stats-view">
      {/* D2: rank bağlamı (soloQ/flex tier + LP + rekor). */}
      <RankCard />
      {/* Koç döngüsü (C1+C2): en son maçın karnesi + sonraki maç hedefi. */}
      <GameReviewCard />
      {/* C4: baskın rol+queue trend okuması. */}
      <TrendPanel />
      {/* F3: haftalık koç özeti. */}
      <WeeklySummaryCard />
      {totalGames > 0 && (
        <p className="stats-summary">
          {t('stats.summary', { games: totalGames, wr: overallWinRate })}
          {top5Keys.length > 0 && <> · {t('stats.top5', { names: top5Keys.join(', ') })}</>}
        </p>
      )}

      {masteryData.length > 0 && (
        <div className="stats-section">
          <h3 className="stats-title">{t('stats.masteryTitle')}</h3>
          <ResponsiveContainer width="100%" height={200}>
            <BarChart data={masteryData} margin={{ top: 4, right: 8, left: 0, bottom: 4 }}>
              <XAxis dataKey="name" tick={{ fontSize: 9, fill: 'var(--color-text-muted)' }} />
              <YAxis width={40} tick={{ fontSize: 9, fill: 'var(--color-text-muted)' }} />
              <Tooltip
                contentStyle={{ background: 'var(--color-bg-panel)', border: '1px solid var(--color-border)', fontSize: 12 }}
                cursor={{ fill: 'rgba(200,170,80,0.08)' }}
              />
              <Bar dataKey="mastery" fill="var(--color-gold)" radius={[3, 3, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </div>
      )}

      {wrData.length > 0 ? (
        <div className="stats-section">
          <h3 className="stats-title">{t('stats.winRateTitle')}</h3>
          <ResponsiveContainer width="100%" height={200}>
            <BarChart data={wrData} margin={{ top: 4, right: 8, left: 0, bottom: 4 }}>
              <XAxis dataKey="name" tick={{ fontSize: 9, fill: 'var(--color-text-muted)' }} />
              <YAxis domain={[0, 100]} width={30} tick={{ fontSize: 9, fill: 'var(--color-text-muted)' }} unit="%" />
              <Tooltip
                contentStyle={{ background: 'var(--color-bg-panel)', border: '1px solid var(--color-border)', fontSize: 12 }}
                cursor={{ fill: 'rgba(200,170,80,0.08)' }}
                formatter={(value: number) => [`${value}%`, t('stats.winLabel')]}
              />
              <Bar dataKey="wr" fill="var(--color-success, #3498DB)" radius={[3, 3, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </div>
      ) : stats.length > 0 ? (
        // İnce veri: stat var ama ≥3 maçlık şampiyon yok → bölümü sessizce gizleme,
        // dürüstçe nedenini söyle.
        <div className="stats-section">
          <h3 className="stats-title">{t('stats.winRateTitle')}</h3>
          <p className="stats-summary">{t('stats.winRateThin')}</p>
        </div>
      ) : null}
    </div>
  );
};
