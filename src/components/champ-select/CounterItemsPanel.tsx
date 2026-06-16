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
                    <CounterItemIcon key={id} id={id} />
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

/**
 * Tek counter-item ikonu: görsel yüklenemezse (404/403 — DDragon sync öncesi veya
 * geçersiz id) kırık görsel yerine dürüst boş kutu gösterir.
 */
const CounterItemIcon: React.FC<{ id: number }> = ({ id }) => {
  const [err, setErr] = React.useState(false);
  if (err || !id) {
    return <div className="counter-items__icon counter-items__icon--empty" />;
  }
  return (
    <img
      src={itemIconUrl(id)}
      alt=""
      className="counter-items__icon"
      onError={() => setErr(true)}
    />
  );
};
