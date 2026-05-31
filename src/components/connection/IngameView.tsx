import React from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Minus } from 'lucide-react';
import './IngameView.css';

interface Props {
  summonerName?: string;
}

// Policy-safe: sadece oyuncunun kendi istatistikleri + spawn timer görünür
// Hidden info YOK: enemy cooldowns, wards, timers
export const IngameView: React.FC<Props> = ({ summonerName }) => (
  <div className="ingame-view">
    <div className="ingame-header">
      <span className="ingame-title">{summonerName ?? 'Oyun devam ediyor'}</span>
      <button
        className="ingame-minimize"
        onClick={() => invoke('hide_window')}
        title="Gizle"
        aria-label="Gizle"
      >
        <Minus size={16} />
      </button>
    </div>
    <div className="ingame-tip">
      <p>İyi şanslar!</p>
      <p className="ingame-tip-sub">Oyun bittikten sonra istatistikler burada görünecek.</p>
    </div>
  </div>
);
