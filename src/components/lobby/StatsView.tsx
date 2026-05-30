import React from 'react';
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';
import { ChampionStats, MasteryEntry } from '../../types/riot';
import './StatsView.css';

interface Props {
  stats: ChampionStats[];
  masteries: MasteryEntry[];
  champMap: Map<number, string>;
}

export const StatsView: React.FC<Props> = ({ stats, masteries, champMap }) => {
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
    return <p className="stats-empty">Veri yok — önce senkronize et.</p>;
  }

  return (
    <div className="stats-view">
      {totalGames > 0 && (
        <p className="stats-summary">
          {totalGames} maç · %{overallWinRate} kazanma oranı
          {top5Keys.length > 0 && <> · Top 5: {top5Keys.join(', ')}</>}
        </p>
      )}

      {masteryData.length > 0 && (
        <div className="stats-section">
          <h3 className="stats-title">Mastery (Top 10)</h3>
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

      {wrData.length > 0 && (
        <div className="stats-section">
          <h3 className="stats-title">Kazanma Oranı % (min 3 maç)</h3>
          <ResponsiveContainer width="100%" height={200}>
            <BarChart data={wrData} margin={{ top: 4, right: 8, left: 0, bottom: 4 }}>
              <XAxis dataKey="name" tick={{ fontSize: 9, fill: 'var(--color-text-muted)' }} />
              <YAxis domain={[0, 100]} width={30} tick={{ fontSize: 9, fill: 'var(--color-text-muted)' }} unit="%" />
              <Tooltip
                contentStyle={{ background: 'var(--color-bg-panel)', border: '1px solid var(--color-border)', fontSize: 12 }}
                cursor={{ fill: 'rgba(200,170,80,0.08)' }}
                formatter={(value: number) => [`${value}%`, 'Kazanma']}
              />
              <Bar dataKey="wr" fill="var(--color-success, #3498DB)" radius={[3, 3, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </div>
      )}
    </div>
  );
};
