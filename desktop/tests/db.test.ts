import { existsSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { afterEach, describe, expect, it } from "vitest";

import {
  loadMigrations,
  openDatabase,
  openDatabaseWithRecovery,
  runMigrations,
} from "../src/main/db";

/** The real migration set — the .sql contract shared with the Tauri app. */
const MIGRATIONS_DIR = join(__dirname, "..", "resources", "migrations");

let dir: string | undefined;
afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true });
});

describe("loadMigrations", () => {
  it("loads the real migration set in version order", () => {
    const migs = loadMigrations(MIGRATIONS_DIR);
    expect(migs.length).toBeGreaterThanOrEqual(19);
    expect(migs[0].version).toBe(1);
    const versions = migs.map((m) => m.version);
    expect(versions).toEqual([...versions].sort((a, b) => a - b));
  });
});

describe("openDatabaseWithRecovery (G2)", () => {
  it("opens a healthy DB without recovery", () => {
    dir = mkdtempSync(join(tmpdir(), "csa-rec-db-"));
    const path = join(dir, "app.db");
    const opened = openDatabaseWithRecovery(path, MIGRATIONS_DIR);
    expect(opened.recovered).toBe(false);
    expect(opened.migrations.applied.length).toBeGreaterThanOrEqual(19);
    opened.db.close();
    // İkinci açılış: idempotent, yine kurtarmasız.
    const second = openDatabaseWithRecovery(path, MIGRATIONS_DIR);
    expect(second.recovered).toBe(false);
    expect(second.migrations.applied).toEqual([]);
    second.db.close();
  });

  it("sets a corrupt file aside (never deletes) and rebuilds a fresh schema", () => {
    dir = mkdtempSync(join(tmpdir(), "csa-rec-db-"));
    const path = join(dir, "app.db");
    // SQLite başlığı olmayan çöp dosya = bozuk DB.
    writeFileSync(path, "bu bir sqlite dosyasi degil — bozuk veri");
    const opened = openDatabaseWithRecovery(path, MIGRATIONS_DIR);
    expect(opened.recovered).toBe(true);
    expect(opened.migrations.applied.length).toBeGreaterThanOrEqual(19);
    // Taze şema gerçekten çalışıyor + bozuk dosya .corrupt-* olarak duruyor.
    const count = opened.db
      .prepare("SELECT COUNT(*) AS c FROM champions")
      .get() as { c: number };
    expect(Number(count.c)).toBe(0);
    opened.db.close();
    expect(existsSync(path)).toBe(true);
    expect(readdirSync(dir).some((f) => f.includes(".corrupt-"))).toBe(true);
  });
});

describe("runMigrations", () => {
  function freshDb(): DatabaseSync {
    dir = mkdtempSync(join(tmpdir(), "csa-db-"));
    return openDatabase(join(dir, "app.db"));
  }

  it("applies the full real migration set on a fresh DB", () => {
    const db = freshDb();
    const result = runMigrations(db, MIGRATIONS_DIR);
    expect(result.applied.length).toBeGreaterThanOrEqual(19);
    expect(result.skipped).toEqual([]);

    // Spot-check core tables exist after V001..V019.
    const tables = (
      db
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .all() as unknown as { name: string }[]
    ).map((r) => r.name);
    for (const t of ["champions", "matches", "builds", "champion_rates"]) {
      expect(tables).toContain(t);
    }
    db.close();
  });

  it("is idempotent: second run skips everything", () => {
    const db = freshDb();
    runMigrations(db, MIGRATIONS_DIR);
    const second = runMigrations(db, MIGRATIONS_DIR);
    expect(second.applied).toEqual([]);
    expect(second.skipped.length).toBeGreaterThanOrEqual(19);
    db.close();
  });

  it("records refinery-compatible history rows", () => {
    const db = freshDb();
    runMigrations(db, MIGRATIONS_DIR);
    const row = db
      .prepare(
        "SELECT version, name, applied_on, checksum FROM refinery_schema_history WHERE version = 1",
      )
      .get() as unknown as {
      version: number;
      name: string;
      applied_on: string;
      checksum: string;
    };
    expect(row.name).toBe("init");
    expect(row.checksum).toMatch(/^[0-9a-f]{64}$/);
    expect(new Date(row.applied_on).getTime()).not.toBeNaN();
    db.close();
  });
});
