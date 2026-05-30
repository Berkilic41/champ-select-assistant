import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
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

  return { status, retry: connect };
}
