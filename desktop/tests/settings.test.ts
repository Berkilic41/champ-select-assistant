// Settings persistence (app_config JSON blob) — compact_overlay optional-default
// (Overlay HUD Slice 2). Eski ayarlarda alan yoksa default'a düşer, diğerleri kalır.

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { afterEach, describe, expect, it } from "vitest";

import {
  DEFAULT_SETTINGS,
  getSettings,
  saveSettings,
} from "../src/main/commands/settings";
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
  dir = mkdtempSync(join(tmpdir(), "csa-settings-"));
  const db = openDatabase(join(dir, "app.db"));
  runMigrations(db, MIGRATIONS_DIR);
  openDb = db;
  return db;
}

describe("getSettings — compact_overlay (Overlay HUD Slice 2)", () => {
  it("round-trips compact_overlay through save/get", () => {
    const db = migratedDb();
    saveSettings(db, { ...DEFAULT_SETTINGS, compact_overlay: true });
    expect(getSettings(db).compact_overlay).toBe(true);
  });

  it("defaults compact_overlay for older settings without it, keeping other values", () => {
    const db = migratedDb();
    // Eski ayar JSON'u (compact_overlay alanı YOK) — REQUIRED_KEYS dışı olduğundan
    // diğer değerler resetlenmemeli, yalnız compact_overlay default'a düşmeli.
    const old = { ...DEFAULT_SETTINGS, sounds_enabled: true } as Record<string, unknown>;
    delete old.compact_overlay;
    db.prepare(
      "INSERT OR REPLACE INTO app_config (key, value) VALUES ('settings', ?)",
    ).run(JSON.stringify(old));

    const s = getSettings(db);
    expect(s.compact_overlay).toBe(false); // optional-default
    expect(s.sounds_enabled).toBe(true); // diğer değerler korundu (reset YOK)
  });
});
