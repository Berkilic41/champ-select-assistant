// SQLite write paths shared by the sync commands — TS port of the write halves
// of src-tauri/src/db/{summoner,champion,match,mastery}_repo.rs (same SQL, same
// conflict semantics). Reads stay in repos.ts.

import type { DatabaseSync } from "node:sqlite";

import type { SummonerInfo } from "./riot/client";

const nowSecs = () => Math.floor(Date.now() / 1000);

export function upsertSummoner(db: DatabaseSync, info: SummonerInfo): void {
  db.prepare(
    `INSERT INTO summoners (puuid, game_name, tag_line, region, cached_at)
     VALUES (?, ?, ?, ?, ?)
     ON CONFLICT(puuid) DO UPDATE SET
         game_name = excluded.game_name,
         tag_line  = excluded.tag_line,
         region    = excluded.region,
         cached_at = excluded.cached_at`,
  ).run(info.puuid, info.game_name, info.tag_line, info.region, nowSecs());
}

/** Kayıtlı summoner'ın platform region'ı; yoksa Rust paritesiyle "euw1". */
export function summonerRegion(db: DatabaseSync, puuid: string): string {
  const row = db
    .prepare("SELECT region FROM summoners WHERE puuid = ?")
    .get(puuid) as unknown as { region?: string } | undefined;
  return row?.region ?? "euw1";
}

/** FK garantisi: champions satırı yoksa placeholder ile oluştur (Rust parity —
 *  isim "Champion {id}", gerçek isim sonraki DDragon sync'inde düzelir). */
export function ensureChampion(db: DatabaseSync, championId: number): void {
  const exists = db
    .prepare("SELECT 1 FROM champions WHERE champion_id = ?")
    .get(championId);
  if (exists) return;
  db.prepare(
    `INSERT INTO champions (champion_id, key, name, title, cached_at)
     VALUES (?, ?, ?, '', ?)
     ON CONFLICT(champion_id) DO NOTHING`,
  ).run(championId, String(championId), `Champion ${championId}`, nowSecs());
}

export interface MatchInsert {
  match_id: string;
  champion_id: number;
  position: string | null;
  win: boolean;
  kills: number;
  deaths: number;
  assists: number;
  duration_secs: number;
  queue_id: number;
  played_at: number;
  cs: number | null;
  // Timeline-türevi alanlar (V020) — yalnız Riot sync doldurur; LCU path ve
  // timeline'sız maçlar null bırakır (rapor null'u dürüstçe atlar).
  cs_at_10?: number | null;
  deaths_pre_14?: number | null;
  vision_score?: number | null;
}

/** INSERT OR IGNORE; true = yeni satır (Rust insert_match bool paritesi). */
export function insertMatch(
  db: DatabaseSync,
  puuid: string,
  m: MatchInsert,
): boolean {
  const result = db
    .prepare(
      `INSERT OR IGNORE INTO matches
         (match_id, puuid, champion_id, position, win, kills, deaths, assists,
          duration_secs, queue_id, played_at, cs, cs_at_10, deaths_pre_14, vision_score)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .run(
      m.match_id,
      puuid,
      m.champion_id,
      m.position,
      m.win ? 1 : 0,
      m.kills,
      m.deaths,
      m.assists,
      m.duration_secs,
      m.queue_id,
      m.played_at,
      m.cs,
      m.cs_at_10 ?? null,
      m.deaths_pre_14 ?? null,
      m.vision_score ?? null,
    );
  return Number(result.changes) > 0;
}

export function upsertMastery(
  db: DatabaseSync,
  puuid: string,
  championId: number,
  level: number,
  points: number,
  lastPlayTime: number | null,
): void {
  db.prepare(
    `INSERT INTO mastery (puuid, champion_id, mastery_level, mastery_points, last_play_time)
     VALUES (?, ?, ?, ?, ?)
     ON CONFLICT(puuid, champion_id) DO UPDATE SET
         mastery_level  = excluded.mastery_level,
         mastery_points = excluded.mastery_points,
         last_play_time = excluded.last_play_time`,
  ).run(puuid, championId, level, points, lastPlayTime);
}

/** Puan değişmediyse snapshot YAZMAZ (Rust record_mastery_snapshot paritesi). */
export function recordMasterySnapshot(
  db: DatabaseSync,
  puuid: string,
  championId: number,
  level: number,
  points: number,
  snapshotAt: number,
): boolean {
  const last = db
    .prepare(
      `SELECT mastery_points FROM mastery_snapshots
       WHERE puuid = ? AND champion_id = ?
       ORDER BY snapshot_at DESC LIMIT 1`,
    )
    .get(puuid, championId) as unknown as
    | { mastery_points?: number }
    | undefined;
  if (last && Number(last.mastery_points) === points) return false;
  db.prepare(
    `INSERT INTO mastery_snapshots
       (puuid, champion_id, mastery_points, mastery_level, snapshot_at)
     VALUES (?, ?, ?, ?, ?)`,
  ).run(puuid, championId, points, level, snapshotAt);
  return true;
}
