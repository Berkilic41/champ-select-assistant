// FAZ 3 / Sprint 3C — getComboOutcomes host testleri. Saf host (core yok):
// resolved outcome'lardan (picked + allies + win) co-pick maç/galibiyet sayımı.

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { afterEach, describe, expect, it } from "vitest";

import { getComboOutcomes, comboPairKey } from "../src/main/commands/combo-outcomes";
import { openDatabase, runMigrations } from "../src/main/db";

const MIGRATIONS_DIR = join(__dirname, "..", "resources", "migrations");

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
  dir = mkdtempSync(join(tmpdir(), "csa-combo-"));
  const db = openDatabase(join(dir, "app.db"));
  runMigrations(db, MIGRATIONS_DIR);
  openDb = db;
  return db;
}

function seedChampion(db: DatabaseSync, id: number, key: string): void {
  db.prepare(
    "INSERT OR IGNORE INTO champions (champion_id, key, name, title, cached_at) VALUES (?, ?, ?, '', 0)",
  ).run(id, key, key);
}

function seedOutcome(
  db: DatabaseSync,
  picked: number,
  allies: number[],
  won: boolean,
): void {
  const ctx = JSON.stringify({ picked, recommended: [], allies });
  db.prepare(
    `INSERT INTO recommendation_outcomes
       (puuid, champion_id, champion_key, queue_id, position, rec_rank, rec_score,
        model_version, context_json, match_id, win, created_at, resolved_at)
     VALUES ('p', ?, 'k', 420, 'jungle', 1, 0.7, 'v', ?, ?, ?, 100, 200)`,
  ).run(picked, ctx, `m-${Math.random()}`, won ? 1 : 0);
}

describe("getComboOutcomes (FAZ 3 / 3C)", () => {
  it("counts co-pick games/wins per canonical pair", () => {
    const db = migratedDb();
    seedChampion(db, 64, "LeeSin");
    seedChampion(db, 51, "Caitlyn");
    // 3 maç Lee+Caitlyn: 2 galibiyet 1 mağlubiyet
    seedOutcome(db, 64, [51], true);
    seedOutcome(db, 64, [51], true);
    seedOutcome(db, 64, [51], false);

    const out = getComboOutcomes(db);
    const key = comboPairKey("LeeSin", "Caitlyn");
    expect(out[key]).toEqual({ games: 3, wins: 2 });
  });

  it("is order-independent (pick/ally swap → same key)", () => {
    const db = migratedDb();
    seedChampion(db, 64, "LeeSin");
    seedChampion(db, 51, "Caitlyn");
    seedOutcome(db, 64, [51], true); // Lee picked, Caitlyn ally
    seedOutcome(db, 51, [64], false); // Caitlyn picked, Lee ally
    const out = getComboOutcomes(db);
    const key = comboPairKey("Caitlyn", "LeeSin");
    expect(out[key]).toEqual({ games: 2, wins: 1 });
  });

  it("dedups a repeated ally within one match + excludes self", () => {
    const db = migratedDb();
    seedChampion(db, 64, "LeeSin");
    seedChampion(db, 51, "Caitlyn");
    seedOutcome(db, 64, [51, 51, 64], true); // tekrarlı Caitlyn + kendisi (64)
    const out = getComboOutcomes(db);
    expect(out[comboPairKey("LeeSin", "Caitlyn")]).toEqual({ games: 1, wins: 1 });
    // kendisiyle çift yok
    expect(out["leesin|leesin"]).toBeUndefined();
  });

  it("skips allies with no champions row (unknown key)", () => {
    const db = migratedDb();
    seedChampion(db, 64, "LeeSin");
    // 999 champions'ta yok
    seedOutcome(db, 64, [999], true);
    const out = getComboOutcomes(db);
    expect(Object.keys(out)).toHaveLength(0);
  });

  it("ignores unresolved rows", () => {
    const db = migratedDb();
    seedChampion(db, 64, "LeeSin");
    seedChampion(db, 51, "Caitlyn");
    seedOutcome(db, 64, [51], true);
    // pending (resolved_at/win NULL)
    db.prepare(
      `INSERT INTO recommendation_outcomes
         (puuid, champion_id, champion_key, queue_id, position, rec_rank, rec_score,
          model_version, context_json, created_at)
       VALUES ('p', 64, 'LeeSin', 420, 'jungle', 1, 0.7, 'v', ?, 100)`,
    ).run(JSON.stringify({ picked: 64, allies: [51] }));
    const out = getComboOutcomes(db);
    expect(out[comboPairKey("LeeSin", "Caitlyn")]).toEqual({ games: 1, wins: 1 });
  });
});
