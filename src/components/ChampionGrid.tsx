import React, { useEffect, useState } from 'react';
import { invoke } from '../lib/host';
import { useTranslation } from 'react-i18next';
import { ChampionStats, MasteryEntry } from '../types/riot';
import { ChampionIcon } from './shared/ChampionIcon';
import { archetypeLabel } from '../lib/archetype';
import { ChampionDetailCard } from './champ-select/ChampionDetailCard';
import './ChampionGrid.css';

interface ChampionGridProps {
  stats: ChampionStats[];
  masteries: MasteryEntry[];
  champMap: Map<number, string>;
  isLoading: boolean;
}

type MasteryLevel = 5 | 6 | 7;

function masteryColor(level: number): string {
  const colors: Record<MasteryLevel, string> = {
    7: 'var(--color-gold)',
    6: '#9B59B6',
    5: '#3498DB',
  };
  return (colors as Record<number, string>)[level] ?? 'var(--color-text-muted)';
}

function winRate(wins: number, games: number): string {
  if (games === 0) return '-';
  return `${Math.round((wins / games) * 100)}%`;
}

interface CardProps {
  stat: ChampionStats;
  mastery?: MasteryEntry;
  champKey?: string;
  archetype?: string;
  onClick?: () => void;
}

const ChampionCard: React.FC<CardProps> = ({ stat, mastery, champKey, archetype, onClick }) => {
  const { t } = useTranslation();
  return (
  <div className="champ-card champ-card--clickable" onClick={onClick}>
    <div className="champ-card__icon-wrap">
      <ChampionIcon size="md" championKey={champKey} />
      {mastery && mastery.mastery_level >= 5 && (
        <span
          className="champ-card__mastery-badge"
          style={{ color: masteryColor(mastery.mastery_level) }}
          title={t('lobby.masteryTitle', { level: mastery.mastery_level })}
        >
          {mastery.mastery_level}
        </span>
      )}
    </div>
    {archetype ? (
      <span className="champ-card__archetype">{archetypeLabel(archetype)}</span>
    ) : (
      <span className="champ-card__id">{champKey ?? `#${stat.champion_id}`}</span>
    )}
    <div className="champ-card__footer">
      <span className="champ-card__games">{stat.games}G</span>
      <span
        className={`champ-card__wr ${stat.wins / stat.games >= 0.52 ? 'champ-card__wr--high' : ''}`}
      >
        {winRate(stat.wins, stat.games)}
      </span>
    </div>
    <span className="champ-card__kda">KDA {stat.kda_avg.toFixed(2)}</span>
  </div>
  );
};

const SkeletonCard: React.FC = () => (
  <div className="champ-card champ-card--skeleton">
    <div className="champ-card__skeleton-icon" />
    <div className="champ-card__skeleton-line" />
    <div className="champ-card__skeleton-line champ-card__skeleton-line--short" />
  </div>
);

const SKELETON_COUNT = 10;

export const ChampionGrid: React.FC<ChampionGridProps> = ({ stats, masteries, champMap, isLoading }) => {
  const { t } = useTranslation();
  const [archetypeMap, setArchetypeMap] = useState<Map<number, string>>(new Map());
  useEffect(() => {
    invoke<Array<{ champion_id: number; archetype: string }>>('get_champion_archetypes')
      .then((rows) => setArchetypeMap(new Map(rows.map((r) => [r.champion_id, r.archetype]))))
      .catch(() => {});
  }, []);
  const [detailId, setDetailId] = useState<number | null>(null);
  const masteryMap = new Map<number, MasteryEntry>(
    masteries.map(m => [m.champion_id, m])
  );

  if (isLoading) {
    return (
      <div className="champ-grid">
        {Array.from({ length: SKELETON_COUNT }).map((_, i) => (
          <SkeletonCard key={i} />
        ))}
      </div>
    );
  }

  // Maç istatistiği yoksa mastery'den sentetik kart oluştur
  if (stats.length === 0 && masteries.length === 0) {
    return (
      <p className="champ-grid__empty">{t('lobby.gridEmpty')}</p>
    );
  }

  const displayStats: ChampionStats[] = stats.length > 0
    ? stats
    : masteries.slice(0, 10).map(m => ({
        champion_id: m.champion_id,
        games: 0,
        wins: 0,
        kda_avg: 0,
      }));

  return (
    <>
      <div className="champ-grid">
        {displayStats.map(stat => (
          <ChampionCard
            key={stat.champion_id}
            stat={stat}
            mastery={masteryMap.get(stat.champion_id)}
            champKey={champMap.get(stat.champion_id)}
            archetype={archetypeMap.get(stat.champion_id)}
            onClick={() => setDetailId(stat.champion_id)}
          />
        ))}
      </div>
      <ChampionDetailCard
        championId={detailId}
        championKey={detailId !== null ? champMap.get(detailId) : undefined}
        onClose={() => setDetailId(null)}
      />
    </>
  );
};
