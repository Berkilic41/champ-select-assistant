// ddragon.ts testleri — saf parser'lar + injectable fetch ile sync komutları
// (gerçek migration'lı DB'ye upsert; ağ YOK).

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { afterEach, describe, expect, it } from "vitest";

import {
  emptyCaches,
  getDdragonVersion,
  isMeleeFromRoles,
  parseChampionList,
  parseItems,
  syncCdragonMeta,
  syncDdragonChampions,
  type FetchJson,
} from "../src/main/ddragon";
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
  dir = mkdtempSync(join(tmpdir(), "csa-dd-"));
  const db = openDatabase(join(dir, "app.db"));
  runMigrations(db, MIGRATIONS_DIR);
  openDb = db;
  return db;
}

describe("parsers (cdragon.rs parity)", () => {
  it("parses champion.json (key=numeric id, id=champion key)", () => {
    const list = parseChampionList({
      data: {
        Garen: { key: "86", id: "Garen", name: "Garen", title: "Güç" },
        Bad: { key: "abc", id: "Bad", name: "x", title: "y" },
      },
    });
    expect(list).toEqual([
      { id: 86, key: "Garen", name: "Garen", title: "Güç" },
    ]);
  });

  it("filters items: purchasable + SR + ≥1800g + no requiredAlly", () => {
    const items = parseItems({
      data: {
        "3071": {
          name: "Black Cleaver",
          tags: ["Health"],
          gold: { total: 3000, purchasable: true },
          maps: { "11": true },
          into: [],
        },
        "1001": {
          name: "Boots",
          gold: { total: 300, purchasable: true },
          maps: { "11": true },
          into: ["3006"],
        },
        "9999": {
          name: "ARAM item",
          gold: { total: 2500, purchasable: true },
          maps: { "12": true },
          into: [],
        },
        "3599": {
          name: "Oathsworn",
          gold: { total: 2000, purchasable: true },
          maps: { "11": true },
          into: [],
          requiredAlly: "Kalista",
        },
      },
    });
    expect(items).toHaveLength(1);
    expect(items[0]).toMatchObject({
      id: 3071,
      name: "Black Cleaver",
      is_completed: true,
    });
  });

  it("is_melee_from_roles parity", () => {
    expect(isMeleeFromRoles(["fighter", "tank"])).toBe(true);
    expect(isMeleeFromRoles(["fighter", "marksman"])).toBe(false);
    expect(isMeleeFromRoles(["mage"])).toBe(false);
  });
});

describe("sync commands (ddragon.rs parity)", () => {
  it("sync_ddragon_champions: upserts champions, fills caches, sets version", async () => {
    const db = migratedDb();
    const caches = emptyCaches();
    const fetchJson: FetchJson = async (url) => {
      if (url.endsWith("/api/versions.json")) return ["14.9.1", "14.8.1"];
      if (url.endsWith("champion.json")) {
        return {
          data: {
            Garen: { key: "86", id: "Garen", name: "Garen", title: "Güç" },
          },
        };
      }
      if (url.endsWith("item.json")) {
        return {
          data: {
            "3071": {
              name: "Black Cleaver",
              gold: { total: 3000, purchasable: true },
              maps: { "11": true },
              into: [],
            },
          },
        };
      }
      if (url.endsWith("runesReforged.json")) {
        return [
          { id: 8000, key: "Precision", slots: [{ runes: [{ id: 8005, key: "PressTheAttack" }] }] },
        ];
      }
      throw new Error(`beklenmeyen url: ${url}`);
    };

    const count = await syncDdragonChampions(db, caches, fetchJson);
    expect(count).toBe(1);
    expect(getDdragonVersion(caches)).toBe("14.9.1");
    expect(caches.items).toHaveLength(1);
    expect(caches.runeTrees[0]?.key).toBe("Precision");

    const row = db
      .prepare("SELECT key, name FROM champions WHERE champion_id = 86")
      .get() as unknown as { key: string; name: string };
    expect(row).toMatchObject({ key: "Garen", name: "Garen" });
  });

  it("getDdragonVersion: pre-sync fallback is a servable version, not the 'unknown' sentinel", () => {
    // Regression: "unknown" ikon URL'ine gömülünce (.../cdn/unknown/img/...) 403 →
    // ikonlar baş-harf yedeğine düşer (paketli build'de görünür). Sync bitmeden de
    // servable bir patch sürümü dönmeli; sync sonrası canlı patch'le güncellenir.
    const v = getDdragonVersion(emptyCaches());
    expect(v).not.toBe("unknown");
    expect(v).toMatch(/^\d+\.\d+\.\d+$/);
  });

  it("sync_cdragon_meta: writes roles + melee heuristic to champion_meta", async () => {
    const db = migratedDb();
    const fetchJson: FetchJson = async () => [
      { id: -1, alias: "None", roles: [] },
      { id: 86, alias: "Garen", roles: ["fighter", "tank"] },
      { id: 51, alias: "Caitlyn", roles: ["marksman"] },
    ];
    const count = await syncCdragonMeta(db, fetchJson);
    expect(count).toBe(2);
    const garen = db
      .prepare("SELECT roles, is_melee, attack_range FROM champion_meta WHERE champion_id = 86")
      .get() as unknown as { roles: string; is_melee: number; attack_range: number };
    expect(JSON.parse(garen.roles)).toEqual(["fighter", "tank"]);
    expect(garen.is_melee).toBe(1);
    expect(garen.attack_range).toBe(175);
  });
});
