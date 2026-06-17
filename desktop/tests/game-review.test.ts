// Koç döngüsü (C1+C2) E2E — GERÇEK WASM + GERÇEK migration'lı DB:
// maçlar → generateGameReviews → karneler + hedef döngüsü (aç/kontrol/kapat).

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { afterEach, beforeAll, describe, expect, it } from "vitest";

import {
  focusStreak,
  generateGameReviews,
  getGameReviewByMatchId,
  getGameReviews,
  getMatchHistory,
  getMatchNote,
  getTrendReport,
  queueGroup,
  setMatchNote,
} from "../src/main/commands/game-review";
import {
  getChampionPreferences,
  getMetaTrend,
  recordRatesSnapshot,
  setChampionPreference,
} from "../src/main/commands/preferences";
import { getSessionCoach, getWeeklySummary } from "../src/main/commands/session-coach";
import { openDatabase, runMigrations } from "../src/main/db";
import { Engine } from "../src/main/engine";

const MIGRATIONS_DIR = join(__dirname, "..", "resources", "migrations");
const PUUID = "review-puuid";

let engine: Engine;
beforeAll(() => {
  engine = Engine.load();
});

let dir: string | undefined;
let openDb: DatabaseSync | undefined;
afterEach(() => {
  try {
    openDb?.close();
  } catch {
    /* zaten kapalı */
  }
  openDb = undefined;
  if (dir) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

function seededDb(): DatabaseSync {
  dir = mkdtempSync(join(tmpdir(), "csa-review-"));
  const db = openDatabase(join(dir, "app.db"));
  runMigrations(db, MIGRATIONS_DIR);
  openDb = db;

  db.prepare(
    "INSERT INTO summoners (puuid, game_name, tag_line, region, cached_at) VALUES (?, 'R', 'TR', 'tr1', 0)",
  ).run(PUUID);
  db.prepare(
    "INSERT INTO champions (champion_id, key, name, title, cached_at) VALUES (86, 'Garen', 'Garen', 't', 0)",
  ).run();

  // 7 soloQ top maçı (eski→yeni): ilk 6'sı baseline havuzu, 7.si zayıf CS'li.
  const ins = db.prepare(
    `INSERT INTO matches (match_id, puuid, champion_id, position, win, kills,
       deaths, assists, duration_secs, queue_id, played_at, cs, vision_score)
     VALUES (?, ?, 86, 'top', ?, 5, ?, 5, 1800, 420, ?, ?, ?)`,
  );
  const rows: Array<[string, number, number, number, number, number]> = [
    ["TR1_r1", 1, 4, 1000, 150, 14],
    ["TR1_r2", 0, 5, 2000, 165, 16],
    ["TR1_r3", 1, 4, 3000, 180, 18],
    ["TR1_r4", 0, 6, 4000, 195, 20],
    ["TR1_r5", 1, 5, 5000, 210, 22],
    ["TR1_r6", 1, 4, 6000, 200, 19],
    // 7. maç: CS belirgin düşük → karne "worse" + CS odaklı hedef beklenir.
    ["TR1_r7", 0, 5, 7000, 110, 18],
  ];
  for (const [id, win, deaths, at, cs, vision] of rows) {
    ins.run(id, PUUID, win, deaths, at, cs, vision);
  }
  return db;
}

describe("koç döngüsü (game_review.rs + game-review.ts)", () => {
  it("maps queue ids to coaching groups", () => {
    expect(queueGroup(420)).toBe("soloq");
    expect(queueGroup(440)).toBe("flex");
    expect(queueGroup(450)).toBe("aram");
    expect(queueGroup(400)).toBe("normal");
  });

  it("lists match history newest-first with champion JOIN + has_review flag (Epic Slice 1)", () => {
    const db = seededDb();
    // Yalnız bir maça karne ekle → has_review tek satırda 1, diğerlerinde 0.
    db.prepare(
      `INSERT INTO game_reviews (match_id, puuid, queue_group, created_at, review_json)
       VALUES ('TR1_r3', ?, 'soloq', 0, '{}')`,
    ).run(PUUID);

    const hist = getMatchHistory(db, PUUID, 20);
    expect(hist).toHaveLength(7);
    // played_at DESC: en yeni (7000) ilk, en eski (1000) son.
    expect(hist[0].match_id).toBe("TR1_r7");
    expect(hist[6].match_id).toBe("TR1_r1");
    // champions JOIN → champion_key dolu.
    expect(hist[0].champion_key).toBe("Garen");
    // has_review yalnız karnesi olan maçta.
    expect(hist.find((r) => r.match_id === "TR1_r3")!.has_review).toBe(1);
    expect(hist.find((r) => r.match_id === "TR1_r7")!.has_review).toBe(0);
    // limit uygulanır.
    expect(getMatchHistory(db, PUUID, 3)).toHaveLength(3);
  });

  it("fetches a single match review by match_id, null when absent (Slice 2)", () => {
    const db = seededDb();
    db.prepare(
      `INSERT INTO game_reviews (match_id, puuid, queue_group, created_at, review_json)
       VALUES ('TR1_r5', ?, 'soloq', 0, '{"champion_key":"Garen","win":true}')`,
    ).run(PUUID);

    const found = getGameReviewByMatchId(db, "TR1_r5");
    expect(found).not.toBeNull();
    expect(found!.match_id).toBe("TR1_r5");
    expect(found!.queue_group).toBe("soloq");
    expect((found!.review as { champion_key: string }).champion_key).toBe("Garen");
    // Karnesi olmayan match_id → null.
    expect(getGameReviewByMatchId(db, "NOPE")).toBeNull();
  });

  it("generates reviews oldest-first, runs the goal loop, and is idempotent", () => {
    const db = seededDb();

    const first = generateGameReviews(engine, db, PUUID);
    expect(first.created).toBe(7);
    expect(first.latest?.match_id).toBe("TR1_r7");

    // En yeni karne: CS kişisel medyanın altında → satır 'worse' + dürüst
    // kilitli timeline satırları.
    const latest = first.latest!.review as {
      lines: { metric: string; verdict: string; baseline?: number }[];
      next_focus?: { metric: string } | null;
      focus_check?: { result: string } | null;
      partial: boolean;
    };
    expect(latest.partial).toBe(false);
    const cs = latest.lines.find((l) => l.metric === "cs_per_min")!;
    expect(cs.verdict).toBe("worse");
    expect(latest.lines.find((l) => l.metric === "cs_at_10")!.verdict).toBe("locked");

    // Hedef döngüsü: 6. maç ilk hedefi açtı, 7. maç onu kontrol edip kapattı,
    // yenisi açıldı → tam 1 açık hedef + en az 1 kapanmış (met/missed) hedef.
    const goals = db
      .prepare("SELECT result FROM focus_goals WHERE puuid = ?")
      .all(PUUID) as unknown as { result: string | null }[];
    expect(goals.filter((g) => g.result === null)).toHaveLength(1);
    expect(goals.some((g) => g.result === "met" || g.result === "missed")).toBe(true);
    expect(latest.focus_check).toBeTruthy();

    // Okuma + idempotans.
    const stored = getGameReviews(db, PUUID, 3);
    expect(stored.length).toBe(3);
    expect(stored[0].queue_group).toBe("soloq");
    const second = generateGameReviews(engine, db, PUUID);
    expect(second.created).toBe(0);

    // Streak dürüst: sonuç ne olursa olsun sayı negatif olamaz.
    expect(focusStreak(db, PUUID, "soloq")).toBeGreaterThanOrEqual(0);
  });

  it("trend report uses dominant role+queue and yields half-median verdicts (C4)", () => {
    const db = seededDb();
    // 7 maç < 8 → partial; yine de noktalar döner.
    const thin = getTrendReport(engine, db, PUUID)!;
    expect(thin.queue_group).toBe("soloq");
    expect(thin.role).toBe("top");
    const thinReport = thin.report as { partial: boolean; points: unknown[] };
    expect(thinReport.partial).toBe(true);
    expect(thinReport.points.length).toBe(7);

    // 8. maç eklenince hükümler gelir.
    db.prepare(
      `INSERT INTO matches (match_id, puuid, champion_id, position, win, kills,
         deaths, assists, duration_secs, queue_id, played_at, cs, vision_score)
       VALUES ('TR1_r8', ?, 86, 'top', 1, 5, 3, 5, 1800, 420, 8000, 215, 24)`,
    ).run(PUUID);
    const full = getTrendReport(engine, db, PUUID)!;
    const report = full.report as {
      partial: boolean;
      verdicts: { metric: string; direction: string }[];
    };
    expect(report.partial).toBe(false);
    expect(report.verdicts.some((v) => v.metric === "win_rate")).toBe(true);
    expect(report.verdicts.some((v) => v.metric === "cs_per_min")).toBe(true);
  });

  it("match notes roundtrip with tag list (C5)", () => {
    const db = seededDb();
    expect(getMatchNote(db, "TR1_r7")).toBeNull();
    const saved = setMatchNote(db, PUUID, "TR1_r7", "wave kontrolü kaçtı", ["wave", "tilt"]);
    expect(saved.tags).toEqual(["wave", "tilt"]);
    const read = getMatchNote(db, "TR1_r7")!;
    expect(read.note).toBe("wave kontrolü kaçtı");
    expect(read.tags).toEqual(["wave", "tilt"]);
    // Üzerine yazma.
    setMatchNote(db, PUUID, "TR1_r7", "güncellendi", []);
    expect(getMatchNote(db, "TR1_r7")!.note).toBe("güncellendi");
    expect(getMatchNote(db, "TR1_r7")!.tags).toEqual([]);
  });

  it("session coach reads only in-session matches and escalates honestly (F1)", () => {
    const db = seededDb(); // 7 maç, played_at 1000..7000; son maç (r7) kayıp
    // boot=8000 → seansta maç yok → fresh_session + ok.
    const fresh = getSessionCoach(engine, db, PUUID, 8000);
    expect(fresh.fresh_session).toBe(true);
    expect((fresh.read as { verdict: string }).verdict).toBe("ok");
    // boot=0 → 7 maç seansta; r7 kayıp ama r6/r5 galibiyet → streak 1 → ok.
    const all = getSessionCoach(engine, db, PUUID, 0);
    expect(all.fresh_session).toBe(false);
    const read = all.read as { games: number; loss_streak: number; verdict: string };
    expect(read.games).toBe(7);
    expect(read.loss_streak).toBe(1);
    expect(read.verdict).toBe("ok");
    // 2 kayıp daha ekle → streak 3 → break (ara öner).
    const ins = db.prepare(
      `INSERT INTO matches (match_id, puuid, champion_id, position, win, kills,
         deaths, assists, duration_secs, queue_id, played_at, cs, vision_score)
       VALUES (?, ?, 86, 'top', 0, 2, 7, 2, 1500, 420, ?, 100, 10)`,
    );
    ins.run("TR1_r8", PUUID, 9000);
    ins.run("TR1_r9", PUUID, 9100);
    const tilted = getSessionCoach(engine, db, PUUID, 0)
      .read as { loss_streak: number; verdict: string; note?: string };
    expect(tilted.loss_streak).toBe(3);
    expect(tilted.verdict).toBe("break");
  });

  it("weekly summary aggregates last-7-day goals and matches (F3)", () => {
    const db = seededDb();
    generateGameReviews(engine, db, PUUID); // hedef döngüsü kapanmış hedef üretir
    const now = Math.floor(Date.now() / 1000);
    // seededDb maçları played_at 1000..7000 (epoch başı) → haftalık pencere DIŞI.
    const old = getWeeklySummary(db, PUUID, now);
    expect(old.games).toBe(0);
    // Pencere "şimdi"yse (epoch 8000) hepsi içeride + kapanmış hedef sayılır.
    const inWindow = getWeeklySummary(db, PUUID, 8000);
    expect(inWindow.games).toBe(7);
    expect(inWindow.reviews).toBe(7);
    expect(inWindow.goals_met + inWindow.goals_missed).toBeGreaterThan(0);
    expect(inWindow.hit_rate).not.toBeNull();
  });

  it("champion preferences roundtrip (D3)", () => {
    const db = seededDb();
    expect(getChampionPreferences(db, PUUID)).toEqual({ never: [], learning: [] });
    setChampionPreference(db, PUUID, 86, "never");
    setChampionPreference(db, PUUID, 54, "learning");
    expect(getChampionPreferences(db, PUUID)).toEqual({ never: [86], learning: [54] });
    // null → tercih kaldırma.
    setChampionPreference(db, PUUID, 86, null);
    expect(getChampionPreferences(db, PUUID).never).toEqual([]);
  });

  it("meta trend snapshot honors the 6h gap and yields honest deltas (D4)", () => {
    const db = seededDb();
    db.prepare(
      `INSERT INTO champion_rates (champion_id, position, win_rate, pick_rate,
         ban_rate, sample_size, patch, source, confidence, cached_at)
       VALUES (86, 'top', 0.50, 0, 0, 5000, '16.12', 'u_gg', 'high', 0)`,
    ).run();
    const t0 = 1_000_000;
    expect(recordRatesSnapshot(db, t0)).toBe(1);
    expect(recordRatesSnapshot(db, t0 + 60)).toBe(0); // 6 saat dolmadı

    // WR 0.50 → 0.53; 7 saat sonra delta dürüstçe +0.03.
    db.prepare(
      "UPDATE champion_rates SET win_rate = 0.53 WHERE champion_id = 86 AND source = 'u_gg'",
    ).run();
    const trend = getMetaTrend(db, 86, "top", t0 + 7 * 3600)!;
    expect(trend.delta_wr).toBeCloseTo(0.03, 5);
    expect(trend.hours_apart).toBe(7);
    // Snapshot'ı olmayan şampiyon → null (çip gizli kalır).
    expect(getMetaTrend(db, 999, "top", t0 + 7 * 3600)).toBeNull();
  });

  it("thin history yields an honest partial review with no goal", () => {
    const db = seededDb();
    // Yalnız ilk 3 maçı bırak.
    db.prepare("DELETE FROM matches WHERE match_id IN ('TR1_r4','TR1_r5','TR1_r6','TR1_r7')").run();
    const res = generateGameReviews(engine, db, PUUID);
    expect(res.created).toBe(3);
    const latest = res.latest!.review as { partial: boolean; next_focus?: unknown };
    expect(latest.partial).toBe(true);
    expect(latest.next_focus ?? null).toBeNull();
    expect(
      (db.prepare("SELECT COUNT(*) AS c FROM focus_goals").get() as { c: number }).c,
    ).toBe(0);
  });
});
