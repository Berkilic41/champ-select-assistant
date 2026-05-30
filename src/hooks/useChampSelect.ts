import { useState, useEffect, useCallback, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { ChampSelectSession, Recommendation } from '../types/recommendation';

export function useChampSelect(puuid: string = ''): {
  session: ChampSelectSession | null;
  recommendations: Recommendation[];
  isActive: boolean;
  loading: boolean;
  error: string | null;
} {
  const [session, setSession] = useState<ChampSelectSession | null>(null);
  const [recommendations, setRecommendations] = useState<Recommendation[]>([]);
  const [isActive, setIsActive] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const prevSessionRef = useRef<ChampSelectSession | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const fetchRecommendations = useCallback(
    async (payload: ChampSelectSession) => {
      setLoading(true);
      setError(null);
      try {
        const recs = await invoke<Recommendation[]>('get_recommendations', {
          sessionJson: payload,
          puuid,
        });
        setRecommendations(recs);
      } catch (e) {
        setError('Öneri alınamadı: ' + String(e));
      } finally {
        setLoading(false);
      }
    },
    [puuid],
  );

  useEffect(() => {
    const unlistenPromise = listen<ChampSelectSession | null>(
      'champ-select-session',
      (event) => {
        if (!event.payload) {
          setSession(null);
          setIsActive(false);
          setRecommendations([]);
          setError(null);
          prevSessionRef.current = null;
          if (debounceRef.current) clearTimeout(debounceRef.current);
          return;
        }

        const prev = prevSessionRef.current;
        const next = event.payload;
        prevSessionRef.current = next;

        setSession(next);
        setIsActive(true);

        const phaseChanged = prev?.phase !== next.phase;
        const locksChanged = JSON.stringify(
          [...next.my_team, ...next.their_team].map(s => s.champion_id),
        ) !== JSON.stringify(
          [...(prev?.my_team ?? []), ...(prev?.their_team ?? [])].map(s => s.champion_id),
        );

        if (phaseChanged || locksChanged) {
          if (debounceRef.current) clearTimeout(debounceRef.current);
          fetchRecommendations(next);
        } else {
          if (debounceRef.current) clearTimeout(debounceRef.current);
          debounceRef.current = setTimeout(() => fetchRecommendations(next), 800);
        }
      },
    );

    return () => {
      unlistenPromise.then((fn) => fn());
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [fetchRecommendations]);

  return { session, recommendations, isActive, loading, error };
}
