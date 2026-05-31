import React from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { Minus } from 'lucide-react';
import './IngameView.css';

interface Props {
  summonerName?: string;
}

// Policy-safe: sadece oyuncunun kendi istatistikleri + spawn timer görünür
// Hidden info YOK: enemy cooldowns, wards, timers
export const IngameView: React.FC<Props> = ({ summonerName }) => {
  const { t } = useTranslation();
  return (
    <div className="ingame-view">
      <div className="ingame-header">
        <span className="ingame-title">{summonerName ?? t('connection.gameInProgress')}</span>
        <button
          className="ingame-minimize"
          onClick={() => invoke('hide_window')}
          title={t('connection.hide')}
          aria-label={t('connection.hide')}
        >
          <Minus size={16} />
        </button>
      </div>
      <div className="ingame-tip">
        <p>{t('connection.goodLuck')}</p>
        <p className="ingame-tip-sub">{t('connection.afterGameStats')}</p>
      </div>
    </div>
  );
};
