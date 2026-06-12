// LCU-based player data sync — port of src-tauri/src/commands/lcu.rs
// `sync_lcu_player_data`. No Riot developer key needed: everything reads the
// local League Client. Partial failures degrade quietly (counters / skipped
// steps), never throw the whole command away — Rust parity.

import type { DatabaseSync } from "node:sqlite";

import {
  parseLcuMastery,
  parseLcuMatchHistory,
  parseLcuSummonerName,
} from "../lcu/player-sync";
import {
  ensureChampion,
  insertMatch,
  recordMasterySnapshot,
  upsertMastery,
  upsertSummoner,
} from "../repos-write";
import type { SummonerInfo } from "../riot/client";
import type { LcuHttp, LcuService } from "./lcu";

export interface LcuPlayerDataSyncResult {
  summoner: SummonerInfo;
  matches_synced: number;
  matches_skipped: number;
  masteries_synced: number;
  errors: number;
}

async function getJsonOrNull(client: LcuHttp, path: string): Promise<unknown> {
  try {
    return await client.getJson(path);
  } catch {
    return null; // non-fatal: ilgili adım atlanır (Rust unwrap_or(Null))
  }
}

/** Maç geçmişi + mastery + summoner'ı çalışan League Client'tan senkronla. */
export async function syncLcuPlayerData(
  db: DatabaseSync,
  lcu: LcuService,
  region: string | undefined,
): Promise<LcuPlayerDataSyncResult> {
  const client = lcu.ensureClient(); // League kapalıysa lockfile hatası fırlar

  // 1. current-summoner — burada hata ölümcül (kimlik yoksa sync anlamsız).
  let summonerJson: Record<string, unknown>;
  try {
    summonerJson = await client.getJson("/lol-summoner/v1/current-summoner");
  } catch (err) {
    throw new Error(`current-summoner hatası: ${(err as Error).message}`);
  }
  const puuid =
    typeof summonerJson.puuid === "string" ? summonerJson.puuid : "";
  if (!puuid) throw new Error("LCU current-summoner yanıtında puuid yok");

  const { gameName, tagLine } = parseLcuSummonerName(summonerJson);
  let platformRegion = region?.trim() ?? "";
  if (!platformRegion) {
    console.warn(
      "sync_lcu_player_data: region parametresi eksik, 'euw1' kullanılıyor",
    );
    platformRegion = "euw1";
  }

  const summoner: SummonerInfo = {
    puuid,
    game_name: gameName,
    tag_line: tagLine,
    region: platformRegion,
  };
  upsertSummoner(db, summoner);

  // 2. Maç geçmişi (hata → atla, sessiz).
  const historyJson = await getJsonOrNull(
    client,
    `/lol-match-history/v1/products/lol/${puuid}/matches?begIndex=0&endIndex=20`,
  );
  let matchesSynced = 0;
  let matchesSkipped = 0;
  let errors = 0;
  for (const m of parseLcuMatchHistory(historyJson, puuid)) {
    try {
      ensureChampion(db, m.champion_id);
      if (insertMatch(db, puuid, m)) matchesSynced += 1;
      else matchesSkipped += 1;
    } catch {
      errors += 1;
    }
  }

  // 3. Mastery: önce local-player ucu, 404'te summonerId yolu (Rust paritesi:
  //    yalnız STRING summonerId fallback'i tetikler).
  let masteryJson = await getJsonOrNull(
    client,
    "/lol-champion-mastery/v1/local-player/champion-mastery",
  );
  if (masteryJson === null) {
    const summonerId =
      typeof summonerJson.summonerId === "string" ? summonerJson.summonerId : "";
    if (summonerId) {
      masteryJson = await getJsonOrNull(
        client,
        `/lol-champion-mastery/v1/${summonerId}/champion-mastery`,
      );
    }
  }

  let masteriesSynced = 0;
  const now = Math.floor(Date.now() / 1000);
  for (const m of parseLcuMastery(masteryJson)) {
    try {
      ensureChampion(db, m.champion_id);
      upsertMastery(db, puuid, m.champion_id, m.level, m.points, m.last_play_time);
      masteriesSynced += 1;
    } catch {
      errors += 1;
    }
    try {
      recordMasterySnapshot(db, puuid, m.champion_id, m.level, m.points, now);
    } catch {
      /* snapshot kritik değil */
    }
  }

  return {
    summoner,
    matches_synced: matchesSynced,
    matches_skipped: matchesSkipped,
    masteries_synced: masteriesSynced,
    errors,
  };
}

