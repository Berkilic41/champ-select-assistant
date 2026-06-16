import React from 'react';
import { champIconUrl } from '../../lib/ddragon';

/**
 * Ban slotu görseli. Key yoksa VEYA görsel yüklenemezse (404/403 — ör. DDragon
 * henüz sync olmadan) baş-harf/kırık-görsel yerine dürüst "unknown" yedek kutusu
 * gösterir. ChampSelectScreen'de hem kendi hem rakip ban kolonunda kullanılır.
 */
export const BanIcon: React.FC<{ champKey?: string }> = ({ champKey }) => {
  const [err, setErr] = React.useState(false);
  if (!champKey || err) {
    return <div className="cs-ban-img cs-ban-img--unknown" />;
  }
  return (
    <img
      src={champIconUrl(champKey)}
      alt={champKey}
      className="cs-ban-img"
      onError={() => setErr(true)}
    />
  );
};
