import { useEffect, useState } from 'react';
import { invoke } from '../lib/host';

/**
 * Aktif summoner puuid'ini çöz. İlk açılışta LCU/sync henüz oturmadıysa boş
 * gelebilir → birkaç kez (8 deneme, 1.5sn) retry et; çözülünce puuid string'i döner
 * ('' = henüz yok). Stats kartlarının (RankCard/TrendPanel/WeeklySummaryCard) ortak
 * puuid kaynağı: eskiden her kart puuid null'da kalıcı boş kalıyordu ve retry yoktu;
 * bu hook puuid çözülünce kartların verisini yeniden çekmesini sağlar.
 */
export function useActiveSummonerPuuid(): string {
  const [puuid, setPuuid] = useState('');

  useEffect(() => {
    let cancelled = false;
    let attempts = 0;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const fetchPuuid = () => {
      invoke<string | null>('get_active_summoner_puuid')
        .then((p) => {
          if (cancelled) return;
          if (p) setPuuid(p);
          else if (attempts++ < 8) timer = setTimeout(fetchPuuid, 1500);
        })
        .catch(() => {
          if (!cancelled && attempts++ < 8) timer = setTimeout(fetchPuuid, 1500);
        });
    };
    fetchPuuid();
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, []);

  return puuid;
}
