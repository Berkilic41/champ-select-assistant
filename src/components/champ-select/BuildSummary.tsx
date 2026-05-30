import React from 'react';
import {
  itemIconUrl,
  perkIconUrl,
  statShardLabel,
  summonerIconUrl,
} from '../../lib/ddragon';
import './BuildSummary.css';

interface Props {
  coreItems: number[];
  situationalItems: number[];
  primaryRuneTree: number;
  keystone: number;
  championName: string;
  skillOrder?: string;
  summonerSpells?: number[];
  secondaryRunes?: number[];
  statShards?: number[];
}

export const BuildSummary: React.FC<Props> = ({
  coreItems,
  situationalItems,
  primaryRuneTree: _primaryRuneTree,
  keystone,
  championName,
  skillOrder,
  summonerSpells = [],
  secondaryRunes = [],
  statShards = [],
}) => {
  if (coreItems.length === 0 && keystone === 0) {
    return <p className="build-empty">Build verisi yükleniyor…</p>;
  }
  const hasSummoners = summonerSpells.length >= 2;
  const hasSecondary = secondaryRunes.length >= 1;
  const hasShards = statShards.length >= 1;

  return (
    <div className="build-summary">
      <p className="build-summary__title">{championName} Build</p>

      <div className="build-summary__section">
        <span className="build-label">Core</span>
        <div className="build-items">
          {coreItems.map((id) => (
            <ItemIcon key={id} id={id} />
          ))}
        </div>
      </div>

      {situationalItems.length > 0 && (
        <div className="build-summary__section">
          <span className="build-label">Duruma Göre</span>
          <div className="build-items">
            {situationalItems.map((id) => (
              <ItemIcon key={id} id={id} />
            ))}
          </div>
        </div>
      )}

      {skillOrder && (
        <div className="build-summary__section">
          <span className="build-label">Skill Order</span>
          <span className="build-skill-order">{skillOrder}</span>
        </div>
      )}

      {hasSummoners && (
        <div className="build-summary__section">
          <span className="build-label">Summoners</span>
          <div className="build-items">
            {summonerSpells.map((id) => (
              <SummonerIcon key={id} id={id} />
            ))}
          </div>
        </div>
      )}

      {hasSecondary && (
        <div className="build-summary__section">
          <span className="build-label">Secondary Runes</span>
          <div className="build-items">
            {secondaryRunes.map((id) => (
              <PerkIcon key={id} id={id} />
            ))}
          </div>
        </div>
      )}

      {hasShards && (
        <div className="build-summary__section">
          <span className="build-label">Stat Shards</span>
          <div className="build-shards">
            {statShards.map((id, i) => (
              <span key={`${id}-${i}`} className="build-shard-chip">
                {statShardLabel(id)}
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};

const ItemIcon: React.FC<{ id: number }> = ({ id }) => {
  const [err, setErr] = React.useState(false);
  if (err || !id) return <div className="item-icon item-icon--empty" />;
  return (
    <img
      className="item-icon"
      src={itemIconUrl(id)}
      alt={`item-${id}`}
      onError={() => setErr(true)}
    />
  );
};

const SummonerIcon: React.FC<{ id: number }> = ({ id }) => {
  const [err, setErr] = React.useState(false);
  const url = summonerIconUrl(id);
  if (err || !url) return <div className="item-icon item-icon--empty" />;
  return (
    <img
      className="item-icon"
      src={url}
      alt={`summoner-${id}`}
      onError={() => setErr(true)}
    />
  );
};

const PerkIcon: React.FC<{ id: number }> = ({ id }) => {
  const [err, setErr] = React.useState(false);
  if (err || !id) return <div className="item-icon item-icon--empty" />;
  return (
    <img
      className="item-icon"
      src={perkIconUrl(id)}
      alt={`perk-${id}`}
      onError={() => setErr(true)}
    />
  );
};
