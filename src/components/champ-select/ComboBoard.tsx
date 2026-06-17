import React from 'react';
import { useTranslation } from 'react-i18next';
import type { ComboBoardEntry } from '../../types/recommendation';
import { ChampionIcon } from '../shared/ChampionIcon';
import './ComboBoard.css';

interface Props {
  combos: ComboBoardEntry[];
  /** Oyuncunun bu pick'le her müttefik combo'sundaki geçmiş kaydı
   *  (ally_champion_key → {games,wins}); host yalnız ≥2 maçlık çiftleri verir.
   *  Yoksa track-record satırı gizli (yeni oyuncuda boş — sahte istatistik yok). */
  trackRecord?: Record<string, { games: number; wins: number }>;
}

/**
 * Synergy board: known combos between the local player's pick and the locked
 * allies, strongest first. Surfaces the "logical draft" depth. Hidden when none.
 * When the player has a co-pick history with an ally (≥2 games), shows their real
 * track record next to the theoretical strength.
 */
export const ComboBoard: React.FC<Props> = ({ combos, trackRecord }) => {
  const { t } = useTranslation();
  if (!combos.length) return null;

  return (
    <div className="combo-board">
      <span className="combo-board__label">{t('comboBoard.title')}</span>
      <div className="combo-board__list">
        {combos.map((c, i) => {
          const record = trackRecord?.[c.ally_champion_key];
          return (
            <div key={i} className="combo-board__row">
              <ChampionIcon championKey={c.ally_champion_key} size="sm" />
              <div className="combo-board__body">
                <div className="combo-board__head">
                  <span className="combo-board__name">{c.name}</span>
                  <span className="combo-board__type">{t(`comboBoard.type_${c.combo_type}`)}</span>
                </div>
                <span className="combo-board__text">{c.combo_text}</span>
                {record && record.games >= 2 && (
                  <span className="combo-board__record">
                    {t('comboBoard.trackRecord', {
                      n: record.games,
                      wr: Math.round((record.wins / record.games) * 100),
                    })}
                  </span>
                )}
                <div className="combo-board__track">
                  <div
                    className="combo-board__fill"
                    style={{ width: `${Math.round(Math.max(0, Math.min(1, c.strength)) * 100)}%` }}
                  />
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};
