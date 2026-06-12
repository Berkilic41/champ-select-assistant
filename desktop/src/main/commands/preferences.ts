// D3 veto/tercihler + D4 meta trend deltası. Host = SQLite okuma/yazma;
// tercihlerin öneriye ETKİSİ core'da (json_api vetoed/learning girdileri).

import type { DatabaseSync } from "node:sqlite";

// ── D3: şampiyon tercihleri ──────────────────────────────────────────────────

export interface ChampionPreferences {
  never: number[];
  learning: number[];
}

export function getChampionPreferences(
  db: DatabaseSync,
  puuid: string,
): ChampionPreferences {
  const rows = db
    .prepare("SELECT champion_id, preference FROM user_preferences WHERE puuid = ?")
    .all(puuid) as unknown as { champion_id: number; preference: string }[];
  return {
    never: rows.filter((r) => r.preference === "never").map((r) => Number(r.champion_id)),
    learning: rows
      .filter((r) => r.preference === "learning")
      .map((r) => Number(r.champion_id)),
  };
}

/** preference null → kayıt silinir (tercih kaldırma). */
export function setChampionPreference(
  db: DatabaseSync,
  puuid: string,
  championId: number,
  preference: "never" | "learning" | null,
): ChampionPreferences {
  if (preference === null) {
    db.prepare(
      "DELETE FROM user_preferences WHERE puuid = ? AND champion_id = ?",
    ).run(puuid, championId);
  } else {
    db.prepare(
      `INSERT INTO user_preferences (puuid, champion_id, preference, created_at)
       VALUES (?, ?, ?, ?)
       ON CONFLICT(puuid, champion_id) DO UPDATE SET
         preference = excluded.preference, created_at = excluded.created_at`,
    ).run(puuid, championId, preference, Math.floor(Date.now() / 1000));
  }
  return getChampionPreferences(db, puuid);
}

// ── D4: meta trend snapshot + delta ──────────────────────────────────────────

/** Snapshot aralığı: bundan daha taze snapshot varsa yenisi alınmaz. */
const SNAPSHOT_MIN_GAP_SECS = 6 * 60 * 60;
/** Tarihçe budama penceresi. */
const HISTORY_RETENTION_SECS = 14 * 24 * 60 * 60;
/** Delta için karşılaştırılan snapshot bundan eski olmalı (gürültü freni). */
const DELTA_MIN_AGE_SECS = 6 * 60 * 60;

/**
 * Başarılı u_gg sync'i sonrası scheduler çağırır: u_gg satırlarının anlık
 * görüntüsünü alır (≥6 saat aralıkla) + 14 günden eskiyi budar.
 * Dönen sayı = yazılan snapshot satırı (0 = aralık dolmadı).
 */
export function recordRatesSnapshot(db: DatabaseSync, nowSecs?: number): number {
  const now = nowSecs ?? Math.floor(Date.now() / 1000);
  const fresh = db
    .prepare(
      "SELECT COUNT(*) AS c FROM champion_rates_history WHERE recorded_at > ?",
    )
    .get(now - SNAPSHOT_MIN_GAP_SECS) as unknown as { c: number };
  if (Number(fresh.c) > 0) return 0;

  db.prepare(
    `INSERT OR IGNORE INTO champion_rates_history
       (champion_id, position, source, win_rate, sample_size, patch, recorded_at)
     SELECT champion_id, position, source, win_rate, sample_size,
            COALESCE(patch, ''), ?
     FROM champion_rates WHERE source = 'u_gg'`,
  ).run(now);
  db.prepare("DELETE FROM champion_rates_history WHERE recorded_at < ?").run(
    now - HISTORY_RETENTION_SECS,
  );
  const written = db
    .prepare("SELECT COUNT(*) AS c FROM champion_rates_history WHERE recorded_at = ?")
    .get(now) as unknown as { c: number };
  return Number(written.c);
}

export interface MetaTrend {
  delta_wr: number;
  hours_apart: number;
}

/**
 * Güncel u_gg win-rate'i ile ≥6 saat önceki en yeni snapshot arasındaki delta.
 * Snapshot/satır yoksa null (görsel-yalnız çip; skora karışmaz — D4 kuralı).
 */
export function getMetaTrend(
  db: DatabaseSync,
  championId: number,
  position: string,
  nowSecs?: number,
): MetaTrend | null {
  const now = nowSecs ?? Math.floor(Date.now() / 1000);
  const current = db
    .prepare(
      `SELECT win_rate FROM champion_rates
       WHERE champion_id = ? AND position = ? AND source = 'u_gg'`,
    )
    .get(championId, position) as unknown as { win_rate: number } | undefined;
  if (!current) return null;
  const past = db
    .prepare(
      `SELECT win_rate, recorded_at FROM champion_rates_history
       WHERE champion_id = ? AND position = ? AND source = 'u_gg'
         AND recorded_at <= ?
       ORDER BY recorded_at DESC LIMIT 1`,
    )
    .get(championId, position, now - DELTA_MIN_AGE_SECS) as unknown as
    | { win_rate: number; recorded_at: number }
    | undefined;
  if (!past) return null;
  return {
    delta_wr: Number(current.win_rate) - Number(past.win_rate),
    hours_apart: Math.round((now - Number(past.recorded_at)) / 3600),
  };
}
