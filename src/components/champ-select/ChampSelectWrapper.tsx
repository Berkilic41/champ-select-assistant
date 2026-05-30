import React, { useEffect, useCallback, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useChampSelect } from '../../hooks/useChampSelect';
import { detectPhaseView } from '../../hooks/useChampSelectPhase';
import { ChampSelectScreen } from './ChampSelectScreen';
import { BanSuggestion, EnemyPoolSummary } from '../../types/recommendation';
import './ChampSelectWrapper.css';

interface ChampionRecord {
  champion_id: number;
  key: string;
}

interface Props {
  summonerName: string;
  port: number;
  addToast: (message: string, type?: 'info' | 'success' | 'warning' | 'error') => void;
}

export const ChampSelectWrapper: React.FC<Props> = ({ addToast }) => {
  const [puuid, setPuuid] = useState<string>('');
  const { session, recommendations, loading, error } = useChampSelect(puuid);
  const [champMap, setChampMap] = useState<Map<number, string>>(new Map());
  const [banSuggestions, setBanSuggestions] = useState<BanSuggestion[]>([]);
  const [enemyPools, setEnemyPools] = useState<EnemyPoolSummary[]>([]);

  // Resolve the active player's puuid once on mount. Empty string is acceptable
  // — backend gracefully returns empty mastery/stats — but causes empty recs.
  useEffect(() => {
    invoke<string | null>('get_active_summoner_puuid')
      .then((p) => setPuuid(p ?? ''))
      .catch(() => setPuuid(''));
  }, []);

  useEffect(() => {
    invoke<ChampionRecord[]>('get_champions')
      .then((records) => setChampMap(new Map(records.map((r) => [r.champion_id, r.key]))))
      .catch(() => setChampMap(new Map()));
  }, []);

  // Fetch ban suggestions and enemy pools when it is the local player's ban turn.
  useEffect(() => {
    if (!session || session.action_type !== 'ban') {
      setBanSuggestions([]);
      setEnemyPools([]);
      return;
    }
    invoke<BanSuggestion[]>('get_ban_suggestions', {
      sessionJson: session,
      puuid,
    }).then(setBanSuggestions).catch(() => setBanSuggestions([]));

    invoke<EnemyPoolSummary[]>('get_enemy_champion_pools', {
      sessionJson: session,
    }).then(setEnemyPools).catch(() => setEnemyPools([]));
  }, [session?.action_type, session?.my_bans.length, session?.their_bans.length, puuid]);

  // Surface recommendation errors as toasts (non-blocking).
  useEffect(() => {
    if (error) addToast(error, 'warning');
  }, [error, addToast]);

  const handleHoverChampion = useCallback(
    async (championId: number) => {
      try {
        await invoke<void>('hover_champion', { championId });
      } catch (err) {
        addToast('Hover başarısız: ' + String(err), 'error');
      }
    },
    [addToast],
  );

  if (!session) {
    return <div className="status-placeholder">Champion Select bekleniyor…</div>;
  }

  const phaseView = detectPhaseView(session);

  return (
    <>
      {session.queue_id === 450 && <span className="aram-badge">ARAM</span>}
      {session.queue_id === 1700 && <span className="aram-badge">ARENA</span>}
      <ChampSelectScreen
        session={session}
        recommendations={recommendations}
        champMap={champMap}
        loading={loading}
        phaseView={phaseView}
        recError={error}
        banSuggestions={banSuggestions}
        enemyPools={enemyPools}
        onHoverChampion={handleHoverChampion}
      />
    </>
  );
};
