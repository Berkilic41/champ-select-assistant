import React from 'react';
import { useTranslation } from 'react-i18next';
import type { ComboBoardEntry } from '../../types/recommendation';
import { ChampionIcon } from '../shared/ChampionIcon';
import './ComboBoard.css';

interface Props {
  combos: ComboBoardEntry[];
}

/**
 * Synergy board: known combos between the local player's pick and the locked
 * allies, strongest first. Surfaces the "logical draft" depth. Hidden when none.
 */
export const ComboBoard: React.FC<Props> = ({ combos }) => {
  const { t } = useTranslation();
  if (!combos.length) return null;

  return (
    <div className="combo-board">
      <span className="combo-board__label">{t('comboBoard.title')}</span>
      <div className="combo-board__list">
        {combos.map((c, i) => (
          <div key={i} className="combo-board__row">
            <ChampionIcon championKey={c.ally_champion_key} size="sm" />
            <div className="combo-board__body">
              <div className="combo-board__head">
                <span className="combo-board__name">{c.name}</span>
                <span className="combo-board__type">{t(`comboBoard.type_${c.combo_type}`)}</span>
              </div>
              <span className="combo-board__text">{c.combo_text}</span>
              <div className="combo-board__track">
                <div
                  className="combo-board__fill"
                  style={{ width: `${Math.round(Math.max(0, Math.min(1, c.strength)) * 100)}%` }}
                />
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};
