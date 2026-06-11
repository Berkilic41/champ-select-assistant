import { useState, useEffect, useCallback } from 'react';
import { invoke } from '../lib/host';
import { AppStatus } from '../types/app';

interface LcuStatus {
  connected: boolean;
  summoner_name?: string;
  port?: number;
  error?: string;
}

export function useLcuStatus(): { status: AppStatus; retry: () => void } {
  const [status, setStatus] = useState<AppStatus>({ kind: 'connecting' });

  const connect = useCallback(async () => {
    setStatus({ kind: 'connecting' });
    try {
      const r = await invoke<LcuStatus>('connect_lcu');
      if (r.connected) {
        setStatus({ kind: 'lobby', summonerName: r.summoner_name ?? 'Summoner', port: r.port ?? 0 });
      } else {
        setStatus({ kind: 'disconnected', error: r.error });
      }
    } catch (e) {
      setStatus({ kind: 'disconnected', error: String(e) });
    }
  }, []);

  useEffect(() => {
    connect();
  }, [connect]);

  // Auto-reconnect: while disconnected, quietly retry every 4s so the app picks
  // up League the moment it launches — no manual "Retry" needed. A quiet probe
  // (no 'connecting' flash); flips to 'lobby' on success and the effect tears down.
  useEffect(() => {
    if (status.kind !== 'disconnected') return;
    const id = setInterval(() => {
      invoke<LcuStatus>('connect_lcu')
        .then((r) => {
          if (r.connected) {
            setStatus({
              kind: 'lobby',
              summonerName: r.summoner_name ?? 'Summoner',
              port: r.port ?? 0,
            });
          }
        })
        .catch(() => {});
    }, 4000);
    return () => clearInterval(id);
  }, [status.kind]);

  return { status, retry: connect };
}
