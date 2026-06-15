// FAZ 3 / Sprint 3B — trainLocalModelPack host testleri. GERÇEK wasm core ile
// (engine.trainModelPack) + migrated DB. Eğitim mantığı core'da test edili; burada
// host yolu: context_json.features → core → draft_brain_packs kind='model_pack'.

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { afterEach, beforeAll, describe, expect, it } from "vitest";

import { trainLocalModelPack } from "../src/main/commands/model-training";
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
  dir = mkdtempSync(join(tmpdir(), "csa-train-"));
  const db = openDatabase(join(dir, "app.db"));
  runMigrations(db, MIGRATIONS_DIR);
  openDb = db;
  return db;
}

interface Features {
  comfort: number;
  matchup: number;
  team_counter: number;
  synergy: number;
  meta: number;
  role_fit: number;
  risk: number;
}

function seedLabeled(
  db: DatabaseSync,
  rows: { features: Features | null; won: boolean }[],
): void {
  const stmt = db.prepare(
    `INSERT INTO recommendation_outcomes
       (puuid, champion_id, champion_key, queue_id, position, rec_rank, rec_score,
        model_version, context_json, match_id, win, created_at, resolved_at)
     VALUES ('p', 64, 'LeeSin', 420, 'jungle', 1, 0.7, 'v', ?, 'm', ?, 100, 200)`,
  );
  for (const r of rows) {
    const ctx = JSON.stringify({ picked: 64, recommended: [], features: r.features });
    stmt.run(ctx, r.won ? 1 : 0);
  }
}

function packRow(db: DatabaseSync): { version: string; payload_json: string } | undefined {
  return db
    .prepare("SELECT version, payload_json FROM draft_brain_packs WHERE kind = 'model_pack'")
    .get() as { version: string; payload_json: string } | undefined;
}

function labeledSet(count: number): { features: Features; won: boolean }[] {
  const out: { features: Features; won: boolean }[] = [];
  for (let i = 0; i < count; i++) {
    const win = i % 2 === 0;
    out.push({
      features: {
        comfort: 0.5,
        matchup: win ? 0.9 : 0.2, // matchup ayırt edici
        team_counter: 0.5,
        synergy: 0.5,
        meta: 0.5,
        role_fit: 0.5,
        risk: 0.2,
      },
      won: win,
    });
  }
  return out;
}

describe("trainLocalModelPack (FAZ 3 / 3B)", () => {
  it("trains + persists a model_pack above the sample gate", () => {
    const db = migratedDb();
    seedLabeled(db, labeledSet(60));
    const result = trainLocalModelPack(engine, db);
    expect(result.trained).toBe(true);
    expect(result.samples).toBe(60);
    expect(result.version).toBe("draft-brain-learned-v1");

    const row = packRow(db);
    expect(row).toBeDefined();
    const pack = JSON.parse(row!.payload_json) as { weights: Record<string, number> };
    // Öğrenilen ağırlık prior'dan ±%50 içinde (sert güvenlik) + matchup ↑.
    expect(pack.weights.matchup).toBeGreaterThan(0.18);
    expect(pack.weights.matchup).toBeLessThanOrEqual(0.18 * 1.5 + 1e-4);
    expect(pack.weights.comfort).toBeGreaterThanOrEqual(0.17 * 0.5 - 1e-4);
  });

  it("does not write a pack below the gate (rules fallback)", () => {
    const db = migratedDb();
    seedLabeled(db, labeledSet(10)); // < 40
    const result = trainLocalModelPack(engine, db);
    expect(result.trained).toBe(false);
    expect(result.samples).toBe(10);
    expect(packRow(db)).toBeUndefined();
  });

  it("ignores rows without a feature vector (off-list picks)", () => {
    const db = migratedDb();
    seedLabeled(db, labeledSet(45));
    // 5 feature'sız satır (liste-dışı pick) — eğitime girmez
    seedLabeled(db, [
      { features: null, won: true },
      { features: null, won: false },
      { features: null, won: true },
    ]);
    const result = trainLocalModelPack(engine, db);
    expect(result.samples).toBe(45); // yalnız feature'lı satırlar
    expect(result.trained).toBe(true);
  });

  it("re-training upserts (single model_pack row)", () => {
    const db = migratedDb();
    seedLabeled(db, labeledSet(50));
    trainLocalModelPack(engine, db, 1000);
    trainLocalModelPack(engine, db, 2000);
    const count = db
      .prepare("SELECT COUNT(*) AS n FROM draft_brain_packs WHERE kind = 'model_pack'")
      .get() as { n: number };
    expect(Number(count.n)).toBe(1);
  });
});
