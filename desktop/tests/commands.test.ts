import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { afterEach, describe, expect, it } from "vitest";

import { getChampions } from "../src/main/commands/champions";
import {
  findOwnPickActionId,
  LcuService,
} from "../src/main/commands/lcu";
import {
  completeOnboarding,
  DEFAULT_SETTINGS,
  getSettings,
  isOnboardingComplete,
  saveSettings,
} from "../src/main/commands/settings";
import { presetSize } from "../src/main/commands/window";
import { openDatabase, runMigrations } from "../src/main/db";

const MIGRATIONS_DIR = join(__dirname, "..", "resources", "migrations");

let dir: string | undefined;
let openDb: DatabaseSync | undefined;
afterEach(() => {
  // Test başarısız olsa da DB handle'ını kapat — yoksa WAL kilidi temp dizin
  // silmeyi EPERM'le bozuyor.
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
  dir = mkdtempSync(join(tmpdir(), "csa-cmd-"));
  const db = openDatabase(join(dir, "app.db"));
  runMigrations(db, MIGRATIONS_DIR);
  openDb = db;
  return db;
}

describe("settings commands (settings.rs parity)", () => {
  it("returns defaults when nothing is stored", () => {
    const db = migratedDb();
    expect(getSettings(db)).toEqual(DEFAULT_SETTINGS);
    db.close();
  });

  it("round-trips saved settings", () => {
    const db = migratedDb();
    const next = { ...DEFAULT_SETTINGS, weight_meta: 0.3, language: "en" };
    saveSettings(db, next);
    expect(getSettings(db)).toEqual(next);
    db.close();
  });

  it("honours the weight_lane_counter alias (Sprint E rename)", () => {
    const db = migratedDb();
    const legacy: Record<string, unknown> = { ...DEFAULT_SETTINGS };
    delete legacy.weight_matchup;
    legacy.weight_lane_counter = 0.4;
    db.prepare(
      "INSERT OR REPLACE INTO app_config (key, value) VALUES ('settings', ?)",
    ).run(JSON.stringify(legacy));
    expect(getSettings(db).weight_matchup).toBe(0.4);
    db.close();
  });

  it("falls back to FULL defaults when a required field is missing (serde parity)", () => {
    const db = migratedDb();
    db.prepare(
      "INSERT OR REPLACE INTO app_config (key, value) VALUES ('settings', ?)",
    ).run(JSON.stringify({ weight_matchup: 0.9 }));
    expect(getSettings(db)).toEqual(DEFAULT_SETTINGS);
    db.close();
  });

  it("tracks onboarding completion", () => {
    const db = migratedDb();
    expect(isOnboardingComplete(db)).toBe(false);
    completeOnboarding(db);
    expect(isOnboardingComplete(db)).toBe(true);
    db.close();
  });
});

describe("window presets (window.rs parity)", () => {
  it("maps every preset to the Tauri sizes", () => {
    expect(presetSize("compact")).toEqual({ width: 320, height: 480 });
    expect(presetSize("wide")).toEqual({ width: 1100, height: 700 });
    expect(presetSize("overlay")).toEqual({ width: 560, height: 180 });
    expect(presetSize("standard")).toEqual({ width: 800, height: 600 });
    expect(presetSize("bilinmeyen")).toEqual({ width: 800, height: 600 });
  });
});

describe("get_champions (ddragon.rs parity)", () => {
  it("lists champions ordered by name", () => {
    const db = migratedDb();
    const insert = db.prepare(
      "INSERT INTO champions (champion_id, key, name, title, cached_at) VALUES (?, ?, ?, ?, 0)",
    );
    insert.run(122, "Darius", "Darius", "Noxus'un Eli");
    insert.run(103, "Ahri", "Ahri", "Dokuz Kuyruklu Tilki");
    const champs = getChampions(db);
    expect(champs.map((c) => c.name)).toEqual(["Ahri", "Darius"]);
    expect(champs[0]).toEqual({
      champion_id: 103,
      key: "Ahri",
      name: "Ahri",
      title: "Dokuz Kuyruklu Tilki",
    });
    db.close();
  });
});

describe("connect_lcu (lcu.rs parity)", () => {
  it("resolves connected:false (not a rejection) when the lockfile is missing", async () => {
    const service = new LcuService({
      emit: () => {},
      findLockfileFn: () => {
        throw new Error("League Client lockfile bulunamadı");
      },
    });
    const status = await service.connect();
    expect(status.connected).toBe(false);
    expect(status.error).toContain("lockfile bulunamadı");
    expect(service.status().connected).toBe(false);
  });

  it("formats summoner as gameName#tagLine and remembers the status", async () => {
    const service = new LcuService({
      emit: () => {},
      findLockfileFn: () => ({
        name: "LeagueClient",
        pid: 1,
        port: 50123,
        password: "pw",
        protocol: "https",
      }),
      makeClient: () => ({
        getJson: async <T>() =>
          ({ gameName: "ELVEDA", tagLine: "SNB" }) as T,
      }),
    });
    // startWsListener gerçek WS açmasın diye stub'la (bağlantı testi değil).
    service.startWsListener = () => {};
    const status = await service.connect();
    expect(status).toEqual({
      connected: true,
      summoner_name: "ELVEDA#SNB",
      region: null,
      port: 50123,
      error: null,
    });
  });

  it("hover_champion PATCHes the local player's active pick action", async () => {
    const calls: { method: string; path: string; body: unknown }[] = [];
    const session = {
      localPlayerCellId: 2,
      actions: [
        [
          { actorCellId: 1, type: "ban", completed: true, id: 3 },
          { actorCellId: 2, type: "pick", completed: false, id: 7 },
        ],
      ],
    };
    const service = new LcuService({
      emit: () => {},
      findLockfileFn: () => ({
        name: "LeagueClient",
        pid: 1,
        port: 50123,
        password: "pw",
        protocol: "https",
      }),
      makeClient: () => ({
        getJson: async <T>(path: string) =>
          (path.includes("champ-select") ? session : { gameName: "A", tagLine: "B" }) as T,
        request: async (method, path, body) => {
          calls.push({ method, path, body });
          return { status: 204, body: "" };
        },
      }),
    });
    service.startWsListener = () => {};
    await service.connect();
    await service.hoverChampion(99);
    expect(calls).toEqual([
      {
        method: "PATCH",
        path: "/lol-champ-select/v1/session/actions/7",
        body: { championId: 99 },
      },
    ]);
  });

  it("hover_champion throws when no active pick action exists", () => {
    expect(
      findOwnPickActionId({ localPlayerCellId: 2, actions: [] } as never, 2),
    ).toBeNull();
    const service = new LcuService({ emit: () => {} });
    return expect(service.hoverChampion(99)).rejects.toThrow("LCU bağlantısı yok");
  });

  it("resolves with an honest error when the LCU request fails", async () => {
    const service = new LcuService({
      emit: () => {},
      findLockfileFn: () => ({
        name: "LeagueClient",
        pid: 1,
        port: 50123,
        password: "pw",
        protocol: "https",
      }),
      makeClient: () => ({
        getJson: async () => {
          throw new Error("ECONNREFUSED");
        },
      }),
    });
    const status = await service.connect();
    expect(status.connected).toBe(false);
    expect(status.port).toBe(50123);
    expect(status.error).toContain("LCU isteği başarısız");
  });
});
