import React, { useState } from 'react';
import { champIconUrl } from '../../lib/ddragon';
import './ChampionIcon.css';

interface Props {
  championKey?: string; // e.g. "Aatrox", "MissFortune" — from Data Dragon
  size?: 'sm' | 'md';
}

export const ChampionIcon: React.FC<Props> = ({ championKey, size = 'sm' }) => {
  const [error, setError] = useState(false);
  const dim = size === 'sm' ? 'var(--champ-icon-sm)' : 'var(--champ-icon-md)';

  if (!championKey || error) {
    return (
      <div
        className="champ-icon champ-icon--placeholder"
        style={{ width: dim, height: dim }}
      />
    );
  }

  return (
    <img
      className="champ-icon"
      src={champIconUrl(championKey)}
      alt={championKey}
      style={{ width: dim, height: dim }}
      onError={() => setError(true)}
    />
  );
};
