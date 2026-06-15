// FAZ 3 / Sprint 3A — win-prob host komutu testleri. GERÇEK wasm core köprüsüyle
// (engine.winProbEstimates) + migrated DB. Kalibrasyon mantığı core'da test edili;
// burada host: DB → örnek → core → rapor yolu ve min-sample gate'i doğrulanır.

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { afterEach, beforeAll, describe, expect, it } from "vitest";

import { getWinProbEstimates } from "../src/main/commands/win-prob";
import { openDatabase, runMigrations } from "../src/main/db";
import { Engine } from "../src/main/engine";

const MIGRATIONS_DIR = join(__dirname, "..", "resources", "migrations");

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

function migratedDb(): DatabaseSync {
  dir = mkdtempSync(join(tmpdir(), "csa-winprob-"));
  const db = openDatabase(join(dir, "app.db"));
  runMigrations(db, MIGRATIONS_DIR);
  openDb = db;
  return db;
}

/** N adet çözülmüş outcome satırı (rec_score + win) seed et. */
function seedResolved(db: DatabaseSync, rows: { score: number; won: boolean }[]): void {
  const stmt = db.prepare(
    `INSERT INTO recommendation_outcomes
       (puuid, champion_id, champion_key, queue_id, position, rec_rank, rec_score,
        model_version, context_json, match_id, win, created_at, resolved_at)
     VALUES ('p', 64, 'LeeSin', 420, 'jungle', 1, ?, 'v', '{}', 'm', ?, 100, 200)`,
  );
  for (const r of rows) stmt.run(r.score, r.won ? 1 : 0);
}

function band(score: number, wins: number, losses: number): { score: number; won: boolean }[] {
  const out: { score: number; won: boolean }[] = [];
  for (let i = 0; i < wins; i++) out.push({ score, won: true });
  for (let i = 0; i < losses; i++) out.push({ score, won: false });
  return out;
}

describe("getWinProbEstimates (FAZ 3 / 3A)", () => {
  it("gates below min-sample — available=false, no estimates", () => {
    const db = migratedDb();
    seedResolved(db, band(0.7, 6, 4)); // 10 < 20
    const report = getWinProbEstimates(engine, db, [0.7]);
    expect(report.available).toBe(false);
    expect(report.estimates).toHaveLength(0);
    expect(report.total).toBe(10);
    expect(report.base_rate).toBeCloseTo(0.6);
  });

  it("calibrates above min-sample — monotone, per-score estimates", () => {
    const db = migratedDb();
    seedResolved(db, [...band(0.2, 2, 8), ...band(0.5, 5, 5), ...band(0.8, 9, 3)]);
    const report = getWinProbEstimates(engine, db, [0.2, 0.5, 0.8]);
    expect(report.available).toBe(true);
    expect(report.total).toBe(32);
    expect(report.estimates).toHaveLength(3);
    const p = report.estimates.map((e) => e.probability);
    expect(p[0]).toBeLessThanOrEqual(p[1]);
    expect(p[1]).toBeLessThanOrEqual(p[2]);
    // her tahmin 0..1 + confidence string
    for (const e of report.estimates) {
      expect(e.probability).toBeGreaterThanOrEqual(0);
      expect(e.probability).toBeLessThanOrEqual(1);
      expect(["low", "medium", "high"]).toContain(e.confidence);
    }
  });

  it("excludes off-list picks (rec_score NULL) from the label set", () => {
    const db = migratedDb();
    seedResolved(db, band(0.6, 12, 12)); // 24 skorlu etiket
    // rec_score NULL (liste-dışı pick) satırı — sayılmamalı
    db.prepare(
      `INSERT INTO recommendation_outcomes
         (puuid, champion_id, champion_key, queue_id, position, rec_rank, rec_score,
          model_version, context_json, match_id, win, created_at, resolved_at)
       VALUES ('p', 99, 'Draven', 420, 'bottom', NULL, NULL, 'v', '{}', 'm2', 1, 100, 200)`,
    ).run();
    const report = getWinProbEstimates(engine, db, [0.6]);
    expect(report.total).toBe(24); // NULL skorlu satır hariç
  });

  it("ignores unresolved (pending) rows", () => {
    const db = migratedDb();
    seedResolved(db, band(0.6, 12, 12));
    // pending (resolved_at/win NULL) satır — sayılmamalı
    db.prepare(
      `INSERT INTO recommendation_outcomes
         (puuid, champion_id, champion_key, queue_id, position, rec_rank, rec_score,
          model_version, context_json, created_at)
       VALUES ('p', 64, 'LeeSin', 420, 'jungle', 1, 0.7, 'v', '{}', 100)`,
    ).run();
    const report = getWinProbEstimates(engine, db, [0.6]);
    expect(report.total).toBe(24);
  });
});
