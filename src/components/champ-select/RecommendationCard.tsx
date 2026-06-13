import React, { useState } from 'react';
import { Recommendation } from '../../types/recommendation';
import { ChampionIcon } from '../shared/ChampionIcon';
import { BuildSummary } from './BuildSummary';
import './RecommendationCard.css';

interface Props {
  rec: Recommendation;
  rank: number;
  champMap: Map<number, string>;
}

interface ScoreRowProps {
  label: string;
  value: number;
}

const ScoreRow: React.FC<ScoreRowProps> = ({ label, value }) => (
  <div className="score-row">
    <span className="score-row__label">{label}</span>
    <div className="score-row__bar-bg">
      <div
        className="score-row__bar-fill"
        style={{ width: `${Math.round(value * 100)}%` }}
      />
    </div>
    <span className="score-row__val">{Math.round(value * 100)}%</span>
  </div>
);

export const RecommendationCard: React.FC<Props> = ({ rec, rank, champMap }) => {
  const [expanded, setExpanded] = useState(false);
  const totalPct = Math.round(rec.total_score * 100);

  return (
    <div
      className="rec-card"
      role="button"
      tabIndex={0}
      aria-expanded={expanded}
      onClick={() => setExpanded((e) => !e)}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          setExpanded((x) => !x);
        }
      }}
    >
      <span className="rec-card__rank">{rank}</span>
      <ChampionIcon
        championKey={champMap.get(rec.champion_id) ?? rec.champion_key}
        size="md"
      />
      <div className="rec-card__info">
        <span className="rec-card__name">{rec.champion_name || rec.champion_key}</span>
        <span className="rec-card__reason">{rec.reason}</span>
      </div>
      <div className="rec-card__score">
        <span className="rec-card__total">{totalPct}</span>
        <div className="rec-card__bar" style={{ width: `${totalPct}%` }} />
      </div>
      {expanded && (
        <div className="rec-card__breakdown">
          <ScoreRow label="Comfort" value={rec.comfort_score} />
          <ScoreRow label="Lane Matchup" value={rec.matchup_score} />
          <ScoreRow label="Komp" value={rec.team_counter_score} />
          <ScoreRow label="Sinerji" value={rec.synergy_score} />
          <ScoreRow label="Meta" value={rec.meta_score} />
          <BuildSummary
            coreItems={rec.core_items}
            situationalItems={rec.situational_items}
            primaryRuneTree={rec.primary_rune_tree}
            keystone={rec.keystone}
            championName={rec.champion_name}
            skillOrder={rec.skill_order}
            summonerSpells={rec.summoner_spells}
            secondaryRunes={rec.secondary_runes}
            statShards={rec.stat_shards}
            buildSource={rec.build_source}
            buildNote={rec.build_note}
          />
        </div>
      )}
    </div>
  );
};
