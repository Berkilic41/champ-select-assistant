// u.gg + Leaguepedia kaynak portu testleri — fixture'lar Rust testlerinin
// AYNILARI (ugg.rs / leaguepedia.rs #[cfg(test)] blokları): parser drift'i
// iki host arasında bu dosyayla yakalanır. Sync fonksiyonları gerçek
// migration'lı DB'ye stub fetch'le yazar (ağ YOK).

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { afterEach, describe, expect, it } from "vitest";

import { openDatabase, runMigrations } from "../src/main/db";
import {
  aggregatePresence,
  leaguepediaUrl,
  normalizeChampionName,
  parseDraftRows,
  parseUggMatchups,
  parseUggOverview,
  syncEdgeRates,
  syncLeaguepedia,
  syncUgg,
  toUggPatch,
} from "../src/main/sources";

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
  dir = mkdtempSync(join(tmpdir(), "csa-src-"));
  const db = openDatabase(join(dir, "app.db"));
  runMigrations(db, MIGRATIONS_DIR);
  openDb = db;
  return db;
}

// Rust ugg.rs OVERVIEW_FIXTURE verbatim (region 12 → rank 8 → role 1 jungle).
const OVERVIEW_FIXTURE = JSON.parse(`{
  "12": { "8": { "1": [[
    [26,17,8000,8300,[8010,8299,8304,8347,9104,9111]],
    [9801,5080,[4,11]],
    [63,40,[1102,2003]],
    [61,39,[6692,6610,3047]],
    [285,157,["E","Q","W"],"QWE"],
    [[[6333,85,153]],[],[],[],[],[]],
    [5081,9803],
    false,
    [101,58,["5005","5008","5011"]],
    [],[],[],[828128.16]
  ],"2026-06-07T12:06:15Z"] } },
  "1": { "8": { "1": [[],""] } }
}`);

// Rust ugg.rs MATCHUPS_FIXTURE verbatim.
const MATCHUPS_FIXTURE = JSON.parse(`{
  "12": { "8": { "1": [[
    [104, 28682, 56764, 6413418, 1668825],
    [234, 22438, 44539, 6151228, -173641],
    [99, 5, 9, 0, 0]
  ],"2026-06-07T12:06:15Z"] } }
}`);

// Rust leaguepedia.rs DRAFT_FIXTURE verbatim.
const DRAFT_FIXTURE = JSON.parse(`{
  "cargoquery": [
    { "title": {
        "Team1Pick1": "Aurelion Sol", "Team1Pick2": "Lee Sin", "Team1Pick3": "Karma",
        "Team1Pick4": "Jinx", "Team1Pick5": "Nautilus",
        "Team2Pick1": "Ahri", "Team2Pick2": "Wukong", "Team2Pick3": "Renata Glasc",
        "Team2Pick4": "Aphelios", "Team2Pick5": "Maokai",
        "Team1Ban1": "Karma", "Team1Ban2": "Kai'Sa", "Team1Ban3": "",
        "Team2Ban1": "Lee Sin", "Team2Ban2": "Vi",
        "DateTime UTC": "2026-06-07 21:13:00"
    } },
    { "title": {
        "Team1Pick1": "Karma", "Team1Pick2": "Sejuani",
        "Team2Pick1": "Lee Sin", "Team2Pick2": "Orianna",
        "Team1Ban1": "Karma", "Team2Ban1": "Karma",
        "DateTime UTC": "2026-06-07 22:00:00"
    } }
  ]
}`);

function nameMap(): Map<string, number> {
  const entries: [string, number][] = [
    ["Karma", 43],
    ["Lee Sin", 64],
    ["Aurelion Sol", 136],
    ["Wukong", 62],
    ["Kai'Sa", 145],
  ];
  return new Map(entries.map(([n, id]) => [normalizeChampionName(n), id]));
}

describe("u.gg parsers (ugg.rs parity)", () => {
  it("maps patch major.minor to the u.gg path form", () => {
    expect(toUggPatch("16.11")).toBe("16_11");
    expect(toUggPatch("16.11.1")).toBe("16_11");
    expect(toUggPatch("9.1")).toBe("9_1");
  });

  it("decodes overview into rate and build rows", () => {
    const { rates, builds } = parseUggOverview(OVERVIEW_FIXTURE, 64, "16.11", "all");
    expect(rates).toHaveLength(1);
    const r = rates[0];
    expect(r).toMatchObject({
      champion_id: 64,
      position: "jungle",
      source: "u_gg",
      sample_size: 9803,
      confidence: "high",
      patch: "16.11",
      region: "all",
      ban_rate: 0,
    });
    expect(Math.abs(Number(r.win_rate) - 5081 / 9803)).toBeLessThan(1e-6);

    expect(builds).toHaveLength(1);
    const b = builds[0];
    expect(b.item_ids).toEqual([6692, 6610, 3047]);
    // Seed format paritesi: [keystone, primary_tree] (A1 spike — d[0][2] stil id'si).
    expect(b.rune_ids).toEqual([8010, 8000]);
    expect(b.secondary_runes).toEqual([8300, 9104, 9111]);
    expect(b.stat_shards).toEqual([5005, 5008, 5011]);
    expect(b.skill_order).toBe("Q→W→E");
    expect(b.summoner_spells).toEqual([4, 11]);
    expect(b.games).toBe(9803);
  });

  it("decodes matchup win-rates and drops low-sample rows", () => {
    const rows = parseUggMatchups(MATCHUPS_FIXTURE, 64, "16.11", "all");
    expect(rows).toHaveLength(2); // opp 99: 9 oyun < 500 taban → atıldı
    const warwick = rows.find((r) => r.opponent_id === 104)!;
    expect(warwick).toMatchObject({
      champion_id: 64,
      position: "jungle",
      games: 56764,
      wins: 28682,
      source: "u_gg",
      confidence: "high",
    });
    expect(Math.abs(Number(warwick.win_rate) - 28682 / 56764)).toBeLessThan(1e-6);
    expect(rows.every((r) => r.opponent_id !== 99)).toBe(true);
  });

  it("drops low-sample roles and missing region without panicking", () => {
    const low = JSON.parse(`{"12":{"8":{"1":[[
      [26,17,8000,8300,[8010]],[10,5,[4,11]],[1,1,[1102]],[1,1,[6692]],
      [1,1,["Q"],"Q"],[[],[],[],[],[],[]],[5,10],false,[1,1,["5005"]],[],[],[],[0]
    ],"t"]}}}`);
    const a = parseUggOverview(low, 1, "16.11", "all");
    expect(a.rates).toHaveLength(0); // 10 maç < 200 tabanı
    expect(a.builds).toHaveLength(0);

    const b = parseUggOverview(JSON.parse(`{"1":{"8":{"1":[[],""]}}}`), 1, "16.11", "all");
    expect(b.rates).toHaveLength(0);
  });
});

describe("Leaguepedia parsers (leaguepedia.rs parity)", () => {
  it("normalizes names across spaces, apostrophes and ampersands", () => {
    expect(normalizeChampionName("Lee Sin")).toBe("leesin");
    expect(normalizeChampionName("Kai'Sa")).toBe("kaisa");
    expect(normalizeChampionName("Nunu & Willump")).toBe("nunuwillump");
    expect(normalizeChampionName("Aurelion Sol")).toBe("aurelionsol");
  });

  it("extracts picks and bans dropping empty slots", () => {
    const rows = parseDraftRows(DRAFT_FIXTURE);
    expect(rows).toHaveLength(2);
    expect(rows[0][0]).toHaveLength(10);
    expect(rows[0][1]).toEqual(["Karma", "Kai'Sa", "Lee Sin", "Vi"]);
  });

  it("aggregates pick/ban presence with per-game dedup and no fabrication", () => {
    const out = aggregatePresence(parseDraftRows(DRAFT_FIXTURE), nameMap());
    const karma = out.find((p) => p.champion_id === 43)!;
    expect(karma.pick_games).toBe(2);
    expect(karma.ban_games).toBe(2); // g2'de iki takım da banladı → oyun başına 1
    expect(Math.abs(karma.pick_rate - 1)).toBeLessThan(1e-6);
    expect(Math.abs(karma.presence - 2)).toBeLessThan(1e-6);

    const lee = out.find((p) => p.champion_id === 64)!;
    expect(lee.pick_games).toBe(2);
    expect(lee.ban_games).toBe(1);

    // Eşleşmeyen adlar (Jinx, Ahri, …) atlanır.
    expect(out.every((p) => [43, 64, 136, 62, 145].includes(p.champion_id))).toBe(true);
    expect(aggregatePresence([], nameMap())).toEqual([]);
  });

  it("builds a polite cargoquery url", () => {
    const url = leaguepediaUrl(0);
    expect(url).toContain("lol.fandom.com/api.php");
    expect(url).toContain("action=cargoquery");
    expect(url).toContain("maxlag=5");
    expect(url).toContain("PicksAndBansS7");
    expect(url).toContain("limit=200");
  });
});

describe("sync runners write canonical rows (sync_*_inner parity)", () => {
  it("syncUgg fetches overview+matchups per champion and upserts", async () => {
    const db = migratedDb();
    db.prepare(
      "INSERT INTO champions (champion_id, key, name, title, cached_at) VALUES (64, 'LeeSin', 'Lee Sin', '', 0)",
    ).run();

    const calls: string[] = [];
    const out = await syncUgg(db, { version: "16.11.1", items: [], runeTrees: [] }, async (url) => {
      calls.push(url);
      if (url.includes("/overview/")) return OVERVIEW_FIXTURE;
      if (url.includes("/matchups/")) return MATCHUPS_FIXTURE;
      throw new Error(`beklenmedik url: ${url}`);
    });

    expect(out).toEqual({ rates: 1, builds: 1, matchups: 2 });
    // Patch probe + overview + matchups; path patch "16_11".
    expect(calls.every((u) => u.includes("16_11"))).toBe(true);
    const rate = db
      .prepare("SELECT win_rate, sample_size, region FROM champion_rates WHERE source = 'u_gg'")
      .get() as unknown as { win_rate: number; sample_size: number; region: string };
    expect(rate.sample_size).toBe(9803);
    expect(rate.region).toBe("all");
    // Matchup opponent'ları (104/234) champions FK'ine placeholder olarak eklendi.
    const mu = db
      .prepare("SELECT COUNT(*) AS c FROM champion_matchups WHERE source = 'u_gg'")
      .get() as unknown as { c: number };
    expect(Number(mu.c)).toBe(2);
  });

  it("labels u.gg rows with the real source patch when the live patch is ahead", async () => {
    // Canlı patch 16.11 ama u.gg yalnız 16_9 sunuyor → satırlar 16.9 etiketlenmeli
    // (16.11 değil); yoksa bayat veri 'güncel patch' sanılıp patch_fresh yanlış olur.
    const db = migratedDb();
    db.prepare(
      "INSERT INTO champions (champion_id, key, name, title, cached_at) VALUES (64, 'LeeSin', 'Lee Sin', '', 0)",
    ).run();

    const out = await syncUgg(db, { version: "16.11.1", items: [], runeTrees: [] }, async (url) => {
      if (url.includes("/overview/")) {
        if (url.includes("16_9")) return OVERVIEW_FIXTURE;
        throw new Error("404"); // 16_11 ve 16_10 veri sunmuyor
      }
      if (url.includes("/matchups/")) {
        if (url.includes("16_9")) return MATCHUPS_FIXTURE;
        throw new Error("404");
      }
      throw new Error(`beklenmedik url: ${url}`);
    });

    expect(out.rates).toBe(1);
    const rate = db
      .prepare("SELECT patch FROM champion_rates WHERE source = 'u_gg'")
      .get() as unknown as { patch: string };
    expect(rate.patch).toBe("16.9");
    const mu = db
      .prepare("SELECT patch_version FROM champion_matchups WHERE source = 'u_gg' LIMIT 1")
      .get() as unknown as { patch_version: string };
    expect(mu.patch_version).toBe("16.9");
  });

  it("syncUgg fails honestly when no patch serves data", async () => {
    const db = migratedDb();
    db.prepare(
      "INSERT INTO champions (champion_id, key, name, title, cached_at) VALUES (64, 'LeeSin', 'Lee Sin', '', 0)",
    ).run();
    await expect(
      syncUgg(db, { version: "16.11", items: [], runeTrees: [] }, async () => {
        throw new Error("404");
      }),
    ).rejects.toThrow("u.gg");
  });

  it("syncLeaguepedia writes pro presence under the synthetic pro position", async () => {
    const db = migratedDb();
    const insert = db.prepare(
      "INSERT INTO champions (champion_id, key, name, title, cached_at) VALUES (?, ?, ?, '', 0)",
    );
    insert.run(43, "Karma", "Karma");
    insert.run(64, "LeeSin", "Lee Sin");

    const n = await syncLeaguepedia(
      db,
      { version: "16.11", items: [], runeTrees: [] },
      async () => DRAFT_FIXTURE,
    );
    expect(n).toBe(2); // yalnız tabloda eşleşen adlar (Karma + Lee Sin)
    const rows = db
      .prepare(
        "SELECT champion_id, position, region, pick_rate, ban_rate, win_rate FROM champion_rates WHERE source = 'leaguepedia' ORDER BY champion_id",
      )
      .all() as unknown as Record<string, number | string>[];
    expect(rows).toHaveLength(2);
    expect(rows[0]).toMatchObject({ champion_id: 43, position: "pro", region: "pro", win_rate: 0 });
    expect(Math.abs(Number(rows[0].pick_rate) - 1)).toBeLessThan(1e-6);
    expect(Math.abs(Number(rows[0].ban_rate) - 1)).toBeLessThan(1e-6);
  });

  it("syncLeaguepedia surfaces API error codes honestly", async () => {
    const db = migratedDb();
    await expect(
      syncLeaguepedia(db, { version: "16.11", items: [], runeTrees: [] }, async () => ({
        error: { code: "ratelimited" },
      })),
    ).rejects.toThrow("ratelimited");
  });
});

describe("edge rates source (cloudflare-worker /v1/rates app-wiring)", () => {
  it("syncEdgeRates pulls aggregate rates + matchups and upserts as cloud_edge", async () => {
    const db = migratedDb();
    db.prepare(
      "INSERT INTO summoners (puuid, game_name, tag_line, region, cached_at) VALUES ('p1', 'Me', 'TR', 'eun1', 5)",
    ).run();

    const calls: string[] = [];
    const n = await syncEdgeRates(db, "https://edge.example/", async (url) => {
      calls.push(url);
      if (url.includes("/v1/matchups")) {
        return {
          patch: "16.11",
          region: "eun1",
          matchups: [
            { champion_id: 86, opponent_id: 103, role: "top", games: 30, wins: 17, win_rate: 17 / 30 },
            { champion_id: 86, opponent_id: 0, role: "top", games: 5, wins: 3, win_rate: 0.6 }, // geçersiz opponent → atılır
          ],
        };
      }
      if (url.includes("/v1/builds")) {
        return {
          patch: "16.11",
          region: "eun1",
          builds: [
            {
              champion_id: 86,
              role: "top",
              item_ids: [3078, 3071, 3053],
              rune_ids: [8000, 8010, 9111],
              summoner_spells: [4, 12],
              games: 25,
              wins: 14,
              win_rate: 14 / 25,
            },
            // boş loadout → atılır
            { champion_id: 99, role: "middle", item_ids: [], rune_ids: [], summoner_spells: [], games: 9, wins: 5, win_rate: 5 / 9 },
          ],
        };
      }
      return {
        patch: "16.11",
        region: "eun1",
        total_games: 120,
        rates: [
          { champion_id: 86, role: "top", games: 120, win_rate: 0.52, pick_rate: 0.1, ban_rate: 0.05 },
          { champion_id: 0, role: "top", games: 50, win_rate: 0.5, pick_rate: 0, ban_rate: 0 }, // geçersiz id → atılır
        ],
      };
    });

    expect(n).toEqual({ rates: 1, matchups: 1, builds: 1 });
    // Bölge aktif summoner'dan; base URL'deki sondaki / temizlenir.
    expect(calls[0]).toBe("https://edge.example/v1/rates?region=eun1");
    expect(calls[1]).toBe("https://edge.example/v1/matchups?region=eun1");
    expect(calls[2]).toBe("https://edge.example/v1/builds?region=eun1");
    const row = db
      .prepare(
        "SELECT position, patch, region, sample_size, confidence FROM champion_rates WHERE source = 'cloud_edge'",
      )
      .get() as unknown as Record<string, unknown>;
    expect(row).toMatchObject({
      position: "top",
      patch: "16.11",
      region: "eun1",
      sample_size: 120,
      confidence: "low", // 120 < 200
    });
    const mu = db
      .prepare(
        "SELECT champion_id, opponent_id, position, games, wins, patch_version, region, confidence FROM champion_matchups WHERE source = 'cloud_edge'",
      )
      .get() as unknown as Record<string, unknown>;
    expect(mu).toMatchObject({
      champion_id: 86,
      opponent_id: 103,
      position: "top",
      games: 30,
      wins: 17,
      patch_version: "16.11",
      region: "eun1",
      confidence: "low", // 30 < 200
    });
    const bu = db
      .prepare(
        "SELECT champion_id, position, item_ids, rune_ids, summoner_spells, games, confidence FROM builds WHERE source = 'cloud_edge'",
      )
      .get() as unknown as Record<string, unknown>;
    expect(bu).toMatchObject({
      champion_id: 86,
      position: "top",
      item_ids: JSON.stringify([3078, 3071, 3053]),
      rune_ids: JSON.stringify([8000, 8010, 9111]),
      summoner_spells: JSON.stringify([4, 12]),
      games: 25,
      confidence: "low", // 25 < 200
    });
  });

  it("syncEdgeRates keeps rates when matchups/builds endpoints fail (best-effort)", async () => {
    const db = migratedDb();
    const n = await syncEdgeRates(db, "https://edge.example", async (url) => {
      if (url.includes("/v1/matchups") || url.includes("/v1/builds")) {
        throw new Error("HTTP 500");
      }
      return {
        patch: "16.11",
        region: "tr1",
        total_games: 10,
        rates: [
          { champion_id: 86, role: "top", games: 10, win_rate: 0.5, pick_rate: 1, ban_rate: 0 },
        ],
      };
    });
    expect(n).toEqual({ rates: 1, matchups: 0, builds: 0 });
    const count = db
      .prepare("SELECT COUNT(*) AS c FROM champion_matchups WHERE source = 'cloud_edge'")
      .get() as unknown as { c: number };
    expect(Number(count.c)).toBe(0);
    const bcount = db
      .prepare("SELECT COUNT(*) AS c FROM builds WHERE source = 'cloud_edge'")
      .get() as unknown as { c: number };
    expect(Number(bcount.c)).toBe(0);
  });

  it("syncEdgeRates rejects a malformed response honestly", async () => {
    const db = migratedDb();
    await expect(
      syncEdgeRates(db, "https://edge.example", async () => ({ rates: "bozuk" })),
    ).rejects.toThrow("geçersiz /v1/rates yanıtı");
  });
});
