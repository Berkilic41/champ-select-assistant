// Seans koçluğu (Faz F): tilt koruması okuması + haftalık koç özeti.
// Host = SQLite okuma + montaj; tilt HÜKMÜ core'da (session_read_json).
//
// Seans tanımı: main sürecinin başlangıcından beri (BOOT_SECS) oynanan maçlar.
// ">1 saat boşluk = yeni seans" inceltmesi bilinçli ertelendi — uygulama
// kapanınca seans zaten sıfırlanır; açık bırakılan uygulama için yeterince iyi.

import type { DatabaseSync } from "node:sqlite";

import type { Engine } from "../engine";
import { recentMatches, toCoreMatch } from "./game-review";

/** Main süreç başlangıcı (modül ilk import'ta bir kez yakalanır). */
const BOOT_SECS = Math.floor(Date.now() / 1000);

export interface SessionCoach {
  read: unknown; // core SessionRead
  /** Bu oturumda hiç maç yok → ısınma checklist'i gösterilebilir. */
  fresh_session: boolean;
}

export function getSessionCoach(
  engine: Engine,
  db: DatabaseSync,
  puuid: string,
  bootSecs: number = BOOT_SECS,
): SessionCoach {
  const inSession = recentMatches(db, puuid, 40).filter(
    (m) => Number(m.played_at) >= bootSecs,
  );
  const read = engine.sessionRead({ matches: inSession.map(toCoreMatch) });
  return { read, fresh_session: inSession.length === 0 };
}

export interface WeeklySummary {
  /** Son 7 günde kapanan hedefler. */
  goals_met: number;
  goals_missed: number;
  /** met / (met+missed); kapanan hedef yoksa null (dürüst). */
  hit_rate: number | null;
  games: number;
  wins: number;
  reviews: number;
}

export function getWeeklySummary(
  db: DatabaseSync,
  puuid: string,
  nowSecs?: number,
): WeeklySummary {
  const now = nowSecs ?? Math.floor(Date.now() / 1000);
  const since = now - 7 * 24 * 60 * 60;

  const goals = db
    .prepare(
      `SELECT result, COUNT(*) AS c FROM focus_goals
       WHERE puuid = ? AND created_at >= ? AND result IN ('met','missed')
       GROUP BY result`,
    )
    .all(puuid, since) as unknown as { result: string; c: number }[];
  const met = Number(goals.find((g) => g.result === "met")?.c ?? 0);
  const missed = Number(goals.find((g) => g.result === "missed")?.c ?? 0);

  const matches = db
    .prepare(
      `SELECT COUNT(*) AS games, COALESCE(SUM(win), 0) AS wins FROM matches
       WHERE puuid = ? AND played_at >= ?`,
    )
    .get(puuid, since) as unknown as { games: number; wins: number };

  const reviews = db
    .prepare(
      "SELECT COUNT(*) AS c FROM game_reviews WHERE puuid = ? AND created_at >= ?",
    )
    .get(puuid, since) as unknown as { c: number };

  return {
    goals_met: met,
    goals_missed: missed,
    hit_rate: met + missed > 0 ? met / (met + missed) : null,
    games: Number(matches.games),
    wins: Number(matches.wins),
    reviews: Number(reviews.c),
  };
}
