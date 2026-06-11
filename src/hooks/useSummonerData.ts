import { useState, useCallback } from 'react';
import { invoke } from '../lib/host';
import {
  ChampionRecord,
  ChampionStats,
  LcuPlayerDataSyncResult,
  MasteryEntry,
  SummonerInfo,
  SyncResult,
} from '../types/riot';
import { setDdragonVersion } from '../lib/ddragon';

interface SummonerState {
  loading: boolean;
  error?: string;
  summonerInfo?: SummonerInfo;
  stats: ChampionStats[];
  masteries: MasteryEntry[];
  champMap: Map<number, string>; // champion_id → key ("Aatrox")
}

const INITIAL_STATE: SummonerState = {
  loading: false,
  stats: [],
  masteries: [],
  champMap: new Map(),
};

/** Load masteries, stats, and champMap from DB for a known puuid. */
async function loadPlayerDataFromDb(puuid: string): Promise<{
  masteries: MasteryEntry[];
  stats: ChampionStats[];
  champMap: Map<number, string>;
  ddragonVersion?: string;
}> {
  const [masteryRes, statsRes, championsRes, ddragonVerRes] = await Promise.allSettled([
    invoke<MasteryEntry[]>('get_masteries', { puuid }),
    invoke<ChampionStats[]>('get_player_stats', { puuid }),
    invoke<ChampionRecord[]>('get_champions'),
    invoke<string>('get_ddragon_version'),
  ]);

  const masteries = masteryRes.status === 'fulfilled' ? masteryRes.value : [];
  const stats = statsRes.status === 'fulfilled' ? statsRes.value : [];
  const champMap =
    championsRes.status === 'fulfilled'
      ? new Map(championsRes.value.map(r => [r.champion_id, r.key]))
      : new Map<number, string>();
  const ddragonVersion =
    ddragonVerRes.status === 'fulfilled' ? ddragonVerRes.value : undefined;

  return { masteries, stats, champMap, ddragonVersion };
}

/**
 * Sync player data. LCU-first: tries the League client directly without
 * requiring a Riot developer API key. Falls back to the Riot API if available.
 *
 * If both fail (or Riot returns NotConfigured), shows "League Client açık değil"
 * instead of asking the user for a developer key.
 */
export function useSummonerData(gameName: string, tagLine: string, region: string) {
  const [state, setState] = useState<SummonerState>(INITIAL_STATE);

  const sync = useCallback(async () => {
    setState({ loading: true, stats: [], masteries: [], champMap: new Map() });

    // ── Path 1: LCU (no API key required) ────────────────────────────────
    try {
      const lcuResult = await invoke<LcuPlayerDataSyncResult>('sync_lcu_player_data', {
        region: region !== 'europe' ? region : null,
      });

      const { masteries, stats, champMap, ddragonVersion } = await loadPlayerDataFromDb(
        lcuResult.summoner.puuid,
      );

      if (ddragonVersion) setDdragonVersion(ddragonVersion);

      // Also kick off background tasks that don't block the UI
      void Promise.allSettled([
        invoke('start_ws_listener'),
        invoke('sync_ddragon_champions'),
        invoke('sync_cdragon_meta'),
      ]);

      setState({ loading: false, summonerInfo: lcuResult.summoner, stats, masteries, champMap });
      return;
    } catch {
      // LCU not available — fall through to Riot API path
    }

    // ── Path 2: Riot API (requires developer key) ─────────────────────────
    if (!gameName || !tagLine) {
      setState({
        loading: false,
        error: 'League Client açık değil',
        stats: [],
        masteries: [],
        champMap: new Map(),
      });
      return;
    }

    try {
      const summonerInfo = await invoke<SummonerInfo>('sync_riot_player', {
        gameName,
        tagLine,
        region,
      });

      const [, , , , , ddragonVerResult] = await Promise.allSettled([
        invoke<SyncResult>('sync_masteries', { puuid: summonerInfo.puuid }),
        invoke<SyncResult>('sync_match_history', { puuid: summonerInfo.puuid, count: 20 }),
        invoke<number>('sync_ddragon_champions'),
        invoke('start_ws_listener'),
        invoke('sync_cdragon_meta'),
        invoke<string>('get_ddragon_version'),
      ]);

      if (ddragonVerResult.status === 'fulfilled' && ddragonVerResult.value) {
        setDdragonVersion(ddragonVerResult.value);
      }

      const { masteries, stats, champMap } = await loadPlayerDataFromDb(summonerInfo.puuid);
      setState({ loading: false, summonerInfo, stats, masteries, champMap });
    } catch (e) {
      const msg = String(e);
      // If Riot key is missing, show the same "League Client" message —
      // normal users should not be asked for a developer key.
      const isConfigError =
        msg.includes('Yapılandırma') ||
        msg.includes('yapılandırılmamış') ||
        msg.includes('NotConfigured') ||
        msg.includes('API key');
      setState({
        loading: false,
        error: isConfigError ? 'League Client açık değil' : msg,
        stats: [],
        masteries: [],
        champMap: new Map(),
      });
    }
  }, [gameName, tagLine, region]);

  return { ...state, sync };
}
