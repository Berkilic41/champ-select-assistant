import React from 'react';
import { useTranslation } from 'react-i18next';
import type { CounterItemHint } from '../../types/recommendation';
import { itemIconUrl } from '../../lib/ddragon';
import './CounterItemsPanel.css';

interface Props {
  items: CounterItemHint[];
}

/**
 * Defensive counter-itemization vs the enemy comp: category + reason + (when a
 * reliable item tag exists) item icons. Hidden when no threat.
 */
export const CounterItemsPanel: React.FC<Props> = ({ items }) => {
  const { t } = useTranslation();
  if (!items.length) return null;

  return (
    <div className="counter-items">
      <span className="counter-items__label">{t('counterItems.title')}</span>
      <div className="counter-items__list">
        {items.map((h, i) => (
          <div key={i} className="counter-items__row">
            <span className="counter-items__cat">{h.category}</span>
            <div className="counter-items__body">
              <span className="counter-items__reason">{h.reason}</span>
              {h.item_ids.length > 0 && (
                <div className="counter-items__icons">
                  {h.item_ids.map((id) => (
                    <img key={id} src={itemIconUrl(id)} alt="" className="counter-items__icon" />
                  ))}
                </div>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};
