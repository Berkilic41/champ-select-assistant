// Koç döngüsü komutları (Faz C1+C2). Host = SQLite okuma/yazma + montaj;
// karne ve hedef seçimi TAMAMEN core'da (engine.gameReview → game_review.rs).
//
// Akış: sync sonrası karnesi olmayan maçlar ESKİDEN YENİYE işlenir; her maç
// kendi rol + queue-grubu geçmişine (o maçtan ÖNCEKİ) kıyaslanır, önceki açık
// hedef o maçta kontrol edilip kapanır, core yeni hedef önerirse açılır.

import type { DatabaseSync } from "node:sqlite";

import type { Engine } from "../engine";

/** queue_id → koçluk grubu (C3 ayrımının ilk yarısı). */
export function queueGroup(queueId: number): string {
  if (queueId === 420) return "soloq";
  if (queueId === 440) return "flex";
  if (queueId === 450) return "aram";
  return "normal";
}

/** Tek seferde en fazla bu kadar yeni karne üretilir (ilk sync seli için fren). */
const MAX_NEW_REVIEWS_PER_RUN = 10;
/** Baseline geçmişi penceresi (core medyanı bunun üzerinden alır). */
const HISTORY_WINDOW = 20;

export interface DbMatchRow {
  match_id: string;
  champion_id: number;
  champion_key: string;
  position: string | null;
  queue_id: number;
  win: number;
  kills: number;
  deaths: number;
  assists: number;
  duration_secs: number;
  played_at: number;
  cs: number | null;
  cs_at_10: number | null;
  deaths_pre_14: number | null;
  vision_score: number | null;
}

interface FocusGoalRow {
  id: number;
  metric: string;
  target: number;
  direction: string;
  label: string;
}

/** core postgame::MatchRow şekline çevir (snake_case, Option=null). */
export function toCoreMatch(r: DbMatchRow): Record<string, unknown> {
  return {
    champion_id: Number(r.champion_id),
    champion_key: String(r.champion_key ?? ""),
    position: String(r.position ?? "").toLowerCase(),
    win: Number(r.win) === 1,
    kills: Number(r.kills),
    deaths: Number(r.deaths),
    assists: Number(r.assists),
    played_at: Number(r.played_at),
    duration_secs: Number(r.duration_secs),
    cs: r.cs === null ? null : Number(r.cs),
    cs_at_10: r.cs_at_10 === null ? null : Number(r.cs_at_10),
    deaths_pre_14: r.deaths_pre_14 === null ? null : Number(r.deaths_pre_14),
    vision_score: r.vision_score === null ? null : Number(r.vision_score),
  };
}

export function recentMatches(db: DatabaseSync, puuid: string, limit: number): DbMatchRow[] {
  return db
    .prepare(
      `SELECT m.match_id, m.champion_id, COALESCE(c.key, '') AS champion_key,
              m.position, m.queue_id, m.win, m.kills, m.deaths, m.assists,
              m.duration_secs, m.played_at, m.cs, m.cs_at_10, m.deaths_pre_14,
              m.vision_score
       FROM matches m
       LEFT JOIN champions c ON c.champion_id = m.champion_id
       WHERE m.puuid = ?
       ORDER BY m.played_at DESC
       LIMIT ?`,
    )
    .all(puuid, limit) as unknown as DbMatchRow[];
}

export interface MatchHistoryRow extends DbMatchRow {
  /** 1 = bu maçın game_reviews karnesi var (Slice 2 detay paneli için). */
  has_review: number;
}

/**
 * Maç geçmişi listesi (Epic Slice 1). `recentMatches` ile aynı matches+champions
 * JOIN'i + per-maç "karne var mı" işareti (EXISTS). recentMatches/review
 * pipeline'ına DOKUNMAZ — yalnız additive okuma. Yeni Riot çağrısı/cloud yok.
 */
export function getMatchHistory(
  db: DatabaseSync,
  puuid: string,
  limit: number,
): MatchHistoryRow[] {
  return db
    .prepare(
      `SELECT m.match_id, m.champion_id, COALESCE(c.key, '') AS champion_key,
              m.position, m.queue_id, m.win, m.kills, m.deaths, m.assists,
              m.duration_secs, m.played_at, m.cs, m.cs_at_10, m.deaths_pre_14,
              m.vision_score,
              EXISTS(SELECT 1 FROM game_reviews gr WHERE gr.match_id = m.match_id) AS has_review
       FROM matches m
       LEFT JOIN champions c ON c.champion_id = m.champion_id
       WHERE m.puuid = ?
       ORDER BY m.played_at DESC
       LIMIT ?`,
    )
    .all(puuid, limit) as unknown as MatchHistoryRow[];
}

function openGoal(db: DatabaseSync, puuid: string, group: string): FocusGoalRow | undefined {
  return db
    .prepare(
      `SELECT id, metric, target, direction, label FROM focus_goals
       WHERE puuid = ? AND queue_group = ? AND result IS NULL
       ORDER BY created_at DESC LIMIT 1`,
    )
    .get(puuid, group) as unknown as FocusGoalRow | undefined;
}

/** Kapanmış hedeflerde sondan ardışık 'met' sayısı (seri). */
export function focusStreak(db: DatabaseSync, puuid: string, group: string): number {
  const rows = db
    .prepare(
      `SELECT result FROM focus_goals
       WHERE puuid = ? AND queue_group = ? AND result IN ('met','missed')
       ORDER BY created_at DESC LIMIT 30`,
    )
    .all(puuid, group) as unknown as { result: string }[];
  let streak = 0;
  for (const r of rows) {
    if (r.result === "met") streak += 1;
    else break;
  }
  return streak;
}

export interface FocusHistoryEntry {
  result: string; // 'met' | 'missed'
  label: string;
  metric: string;
}

/**
 * Son N kapanmış odak hedefi (Epic #3 — gelişim koçluğu). En yeni önce; yalnız
 * met/missed (superseded/no_data hariç) — hedef-tutturma serisini görselleştirmek
 * için. focusStreak yalnız SAYI veriyordu; bu paterni (✓✗✓) çıkarır. Core değişmez.
 */
export function getFocusHistory(
  db: DatabaseSync,
  puuid: string,
  group: string,
  limit = 8,
): FocusHistoryEntry[] {
  const rows = db
    .prepare(
      `SELECT result, label, metric FROM focus_goals
       WHERE puuid = ? AND queue_group = ? AND result IN ('met','missed')
       ORDER BY created_at DESC LIMIT ?`,
    )
    .all(puuid, group, limit) as unknown as {
    result: string;
    label: string;
    metric: string;
  }[];
  return rows.map((r) => ({
    result: String(r.result),
    label: String(r.label),
    metric: String(r.metric),
  }));
}

export interface StoredReview {
  match_id: string;
  queue_group: string;
  created_at: number;
  review: unknown;
}

export function getGameReviews(db: DatabaseSync, puuid: string, limit = 3): StoredReview[] {
  const rows = db
    .prepare(
      `SELECT match_id, queue_group, created_at, review_json FROM game_reviews
       WHERE puuid = ? ORDER BY created_at DESC LIMIT ?`,
    )
    .all(puuid, limit) as unknown as {
    match_id: string;
    queue_group: string;
    created_at: number;
    review_json: string;
  }[];
  return rows.map((r) => ({
    match_id: String(r.match_id),
    queue_group: String(r.queue_group),
    created_at: Number(r.created_at),
    review: JSON.parse(String(r.review_json)),
  }));
}

/**
 * Tek maçın karnesi (Maç Geçmişi detay paneli — Epic Slice 2). Yoksa null.
 * match_id PK olduğu için en fazla bir satır döner.
 */
export function getGameReviewByMatchId(
  db: DatabaseSync,
  matchId: string,
): StoredReview | null {
  const row = db
    .prepare(
      `SELECT match_id, queue_group, created_at, review_json FROM game_reviews
       WHERE match_id = ?`,
    )
    .get(matchId) as
    | { match_id: string; queue_group: string; created_at: number; review_json: string }
    | undefined;
  if (!row) return null;
  return {
    match_id: String(row.match_id),
    queue_group: String(row.queue_group),
    created_at: Number(row.created_at),
    review: JSON.parse(String(row.review_json)),
  };
}

// ── C4: Trend raporu ─────────────────────────────────────────────────────────

export interface TrendResult {
  report: unknown;
  role: string;
  queue_group: string;
  games: number;
}

/** Baskın queue-grubu + baskın roldeki son maçlardan trend raporu (core). */
export function getTrendReport(
  engine: Engine,
  db: DatabaseSync,
  puuid: string,
): TrendResult | null {
  const rows = recentMatches(db, puuid, 40);
  if (rows.length === 0) return null;

  // Baskın grup → grup içinde baskın rol (boş pozisyonlar rol seçiminde sayılmaz).
  const counts = new Map<string, number>();
  for (const r of rows) {
    const g = queueGroup(Number(r.queue_id));
    counts.set(g, (counts.get(g) ?? 0) + 1);
  }
  const group = [...counts.entries()].sort((a, b) => b[1] - a[1])[0][0];
  const inGroup = rows.filter((r) => queueGroup(Number(r.queue_id)) === group);

  const roleCounts = new Map<string, number>();
  for (const r of inGroup) {
    const role = String(r.position ?? "").toLowerCase();
    if (role) roleCounts.set(role, (roleCounts.get(role) ?? 0) + 1);
  }
  const role =
    [...roleCounts.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] ?? "";
  const filtered = role
    ? inGroup.filter((r) => String(r.position ?? "").toLowerCase() === role)
    : inGroup;
  const window = filtered.slice(0, 20); // en yeni 20; core eski→yeni sıralar

  const report = engine.trendReport({ matches: window.map(toCoreMatch) });
  return { report, role, queue_group: group, games: window.length };
}

// ── C5: Maç notları ──────────────────────────────────────────────────────────

export interface MatchNote {
  match_id: string;
  note: string;
  tags: string[];
  updated_at: number;
}

export function getMatchNote(db: DatabaseSync, matchId: string): MatchNote | null {
  const row = db
    .prepare("SELECT match_id, note, tags, updated_at FROM match_notes WHERE match_id = ?")
    .get(matchId) as
    | { match_id: string; note: string; tags: string; updated_at: number }
    | undefined;
  if (!row) return null;
  let tags: string[] = [];
  try {
    const parsed: unknown = JSON.parse(String(row.tags));
    if (Array.isArray(parsed)) tags = parsed.filter((t): t is string => typeof t === "string");
  } catch {
    /* bozuk tags → boş liste, not yine döner */
  }
  return {
    match_id: String(row.match_id),
    note: String(row.note),
    tags,
    updated_at: Number(row.updated_at),
  };
}

export function setMatchNote(
  db: DatabaseSync,
  puuid: string,
  matchId: string,
  note: string,
  tags: string[],
): MatchNote {
  const now = Math.floor(Date.now() / 1000);
  const cleanTags = tags.filter((t) => typeof t === "string").slice(0, 6);
  db.prepare(
    `INSERT INTO match_notes (match_id, puuid, note, tags, updated_at)
     VALUES (?, ?, ?, ?, ?)
     ON CONFLICT(match_id) DO UPDATE SET
       note = excluded.note, tags = excluded.tags, updated_at = excluded.updated_at`,
  ).run(matchId, puuid, note, JSON.stringify(cleanTags), now);
  return { match_id: matchId, note, tags: cleanTags, updated_at: now };
}

export interface GenerateResult {
  created: number;
  /** En son üretilen karne (varsa) + maç kimliği. */
  latest: StoredReview | null;
  /** En son karnenin queue-grubundaki ardışık hedef-tutturma serisi. */
  streak: number;
}

export function generateGameReviews(
  engine: Engine,
  db: DatabaseSync,
  puuid: string,
): GenerateResult {
  const rows = recentMatches(db, puuid, 60);
  if (rows.length === 0) return { created: 0, latest: null, streak: 0 };

  const reviewed = new Set(
    (
      db
        .prepare("SELECT match_id FROM game_reviews WHERE puuid = ?")
        .all(puuid) as unknown as { match_id: string }[]
    ).map((r) => String(r.match_id)),
  );

  // Karnesi olmayanlar, ESKİDEN YENİYE (hedef döngüsü sıralı işler).
  const pending = rows
    .filter((r) => !reviewed.has(String(r.match_id)))
    .sort((a, b) => Number(a.played_at) - Number(b.played_at))
    .slice(0, MAX_NEW_REVIEWS_PER_RUN);

  let created = 0;
  let latest: StoredReview | null = null;
  const now = Math.floor(Date.now() / 1000);

  for (const m of pending) {
    const group = queueGroup(Number(m.queue_id));
    const role = String(m.position ?? "").toLowerCase();
    // Geçmiş = bu maçtan ÖNCEKİ aynı rol + queue-grubu maçları (pencere 20).
    const history = rows
      .filter(
        (h) =>
          Number(h.played_at) < Number(m.played_at) &&
          queueGroup(Number(h.queue_id)) === group &&
          String(h.position ?? "").toLowerCase() === role,
      )
      .slice(0, HISTORY_WINDOW);

    const prev = openGoal(db, puuid, group);
    const review = engine.gameReview<{
      focus_check?: { result: string } | null;
      next_focus?: { metric: string; target: number; direction: string; label: string } | null;
    }>({
      match: toCoreMatch(m),
      history: history.map(toCoreMatch),
      prev_goal: prev
        ? {
            metric: prev.metric,
            target: Number(prev.target),
            direction: prev.direction,
            label: prev.label,
          }
        : null,
    });

    db.prepare(
      `INSERT INTO game_reviews (match_id, puuid, queue_group, created_at, review_json)
       VALUES (?, ?, ?, ?, ?)
       ON CONFLICT(match_id) DO NOTHING`,
    ).run(String(m.match_id), puuid, group, now, JSON.stringify(review));

    // Önceki hedefi bu maçın sonucuyla kapat.
    if (prev && review.focus_check) {
      db.prepare(
        "UPDATE focus_goals SET result = ?, checked_match_id = ? WHERE id = ?",
      ).run(String(review.focus_check.result), String(m.match_id), prev.id);
    }

    // Yeni hedef: önce aynı gruptaki kalan açıkları 'superseded' yap (tek açık
    // hedef ilkesi), sonra core'un önerdiğini aç.
    if (review.next_focus) {
      db.prepare(
        `UPDATE focus_goals SET result = 'superseded'
         WHERE puuid = ? AND queue_group = ? AND result IS NULL`,
      ).run(puuid, group);
      db.prepare(
        `INSERT INTO focus_goals
           (puuid, queue_group, metric, target, direction, label, created_at, source_match_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
      ).run(
        puuid,
        group,
        String(review.next_focus.metric),
        Number(review.next_focus.target),
        String(review.next_focus.direction),
        String(review.next_focus.label),
        now,
        String(m.match_id),
      );
    }

    created += 1;
    latest = {
      match_id: String(m.match_id),
      queue_group: group,
      created_at: now,
      review,
    };
  }

  const streakGroup = latest?.queue_group ?? "soloq";
  return { created, latest, streak: focusStreak(db, puuid, streakGroup) };
}
