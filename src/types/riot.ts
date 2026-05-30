export interface SummonerInfo {
  puuid: string;
  game_name: string;
  tag_line: string;
  region: string;
}

export interface SyncResult {
  synced: number;
  skipped: number;
  errors: number;
}

export interface ChampionStats {
  champion_id: number;
  games: number;
  wins: number;
  kda_avg: number;
}

export interface MasteryEntry {
  champion_id: number;
  mastery_level: number;
  mastery_points: number;
}

export interface ChampionRecord {
  champion_id: number;
  key: string;   // "Aatrox", "MissFortune"
  name: string;
  title: string;
}

/** Returned by the `sync_lcu_player_data` Tauri command. */
export interface LcuPlayerDataSyncResult {
  summoner: SummonerInfo;
  matches_synced: number;
  matches_skipped: number;
  masteries_synced: number;
  errors: number;
}
