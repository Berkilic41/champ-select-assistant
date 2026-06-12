import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { TrendingUp, TrendingDown } from 'lucide-react';
import { invoke } from '../../lib/host';

interface MetaTrend {
  delta_wr: number;
  hours_apart: number;
}

/** Görünür olmak için gereken en küçük WR değişimi (mutlak). */
const MIN_DELTA = 0.01;

/**
 * D4: meta trend çipi — aktif önerinin win-rate'i son u.gg snapshot'ından beri
 * anlamlı oynadıysa ▲/▼ gösterir. GÖRSEL-yalnız: skora hiçbir etkisi yok
 * (delta'lar kararlılık kanıtlayana dek Bayesian sinyale karıştırılmaz).
 */
export const MetaTrendChip: React.FC<{ championId: number; position: string }> = ({
  championId,
  position,
}) => {
  const { t } = useTranslation();
  const [trend, setTrend] = useState<MetaTrend | null>(null);

  useEffect(() => {
    let alive = true;
    setTrend(null);
    if (!championId || !position) return;
    invoke<MetaTrend | null>('get_meta_trend', { championId, position })
      .then((res) => {
        if (alive) setTrend(res);
      })
      .catch(() => {
        /* snapshot yok — çip gizli */
      });
    return () => {
      alive = false;
    };
  }, [championId, position]);

  if (!trend || Math.abs(trend.delta_wr) < MIN_DELTA) return null;
  const up = trend.delta_wr > 0;
  const pct = (Math.abs(trend.delta_wr) * 100).toFixed(1);

  return (
    <span
      className={`meta-trend-chip ${up ? 'meta-trend-chip--up' : 'meta-trend-chip--down'}`}
      title={t('metaTrend.tooltip', { hours: trend.hours_apart })}
    >
      {up ? <TrendingUp size={12} /> : <TrendingDown size={12} />}
      {t(up ? 'metaTrend.rising' : 'metaTrend.falling', { pct })}
    </span>
  );
};
