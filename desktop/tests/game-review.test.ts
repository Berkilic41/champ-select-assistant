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
  getGameReviews,
  queueGroup,
} from "../src/main/commands/game-review";
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
