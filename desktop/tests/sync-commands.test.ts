// P1.3b-8 sync dilimi testleri — Riot client retry/limit davranışı (stub fetch),
// LCU player-sync parser'ları (Rust fixture'larının AYNILARI), sync komutları
// gerçek migrated DB'ye, feedback flush gerçek WASM core policy'siyle.

import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { afterEach, beforeAll, describe, expect, it } from "vitest";

import { LcuService } from "../src/main/commands/lcu";
import { syncRecommendationFeedback } from "../src/main/commands/feedback-flush";
import { saveSettings, DEFAULT_SETTINGS } from "../src/main/commands/settings";
import { syncLcuPlayerData } from "../src/main/commands/player-data";
import { syncDataPipeline } from "../src/main/commands/data-pipeline";
import {
  hashPuuid,
  RollingBudget,
  syncMatchV5Ingestion,
} from "../src/main/match-v5";
import {
  getDataSourceRegistry,
  getDataTrajectory,
  getPipelineQualityReport,
} from "../src/main/commands/data-quality";
import { getDraftBrainQualityReport } from "../src/main/commands/quality";
import {
  syncMasteries,
  syncMatchHistory,
  syncRiotPlayer,
} from "../src/main/commands/riot-sync";
import { openDatabase, runMigrations } from "../src/main/db";
import { Engine } from "../src/main/engine";
import {
  parseLcuMastery,
  parseLcuMatchHistory,
  parseLcuRankedStats,
  parseLcuSummonerName,
} from "../src/main/lcu/player-sync";
import {
  buildListIdsUrl,
  parseEnvFile,
  RiotClient,
  routingForRegion,
  runtimeClientFromEnv,
  type RiotFetch,
} from "../src/main/riot/client";

const MIGRATIONS_DIR = join(__dirname, "..", "resources", "migrations");
const FIXTURES_DIR = join(__dirname, "fixtures");
const LOCAL_PUUID = "test-puuid-local-player";

const matchHistoryFixture = () =>
  JSON.parse(readFileSync(join(FIXTURES_DIR, "lcu_match_history.json"), "utf8"));
const masteryFixture = () =>
  JSON.parse(readFileSync(join(FIXTURES_DIR, "lcu_mastery.json"), "utf8"));

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
  dir = mkdtempSync(join(tmpdir(), "csa-sync-"));
  const db = openDatabase(join(dir, "app.db"));
  runMigrations(db, MIGRATIONS_DIR);
  openDb = db;
  return db;
}

/** URL desenine göre yanıt veren stub fetch + çağrı kaydı. */
function stubFetch(
  routes: [RegExp, () => { status: number; body?: unknown; retryAfterSecs?: number }][],
): { fetch: RiotFetch; calls: string[] } {
  const calls: string[] = [];
  const fetch: RiotFetch = async (url) => {
    calls.push(url);
    const route = routes.find(([re]) => re.test(url));
    const resp = route ? route[1]() : { status: 404 };
    return {
      status: resp.status,
      retryAfterSecs: resp.retryAfterSecs,
      json: async () => resp.body ?? {},
    };
  };
  return { fetch, calls };
}

const noSleep = async () => {};

describe("RiotClient (riot/client.rs parity)", () => {
  it("parses .env content and resolves routing regions", () => {
    const env = parseEnvFile('# yorum\nRIOT_API_KEY="RGAPI-test"\nBOŞ=\nX=1');
    expect(env.RIOT_API_KEY).toBe("RGAPI-test");
    expect(routingForRegion("tr1")).toBe("europe");
    expect(routingForRegion("NA1")).toBe("americas");
    expect(routingForRegion("bilinmeyen")).toBe("europe");
    expect(runtimeClientFromEnv({})).toBeNull();
    expect(runtimeClientFromEnv({ RIOT_API_KEY: "  " })).toBeNull();
    expect(runtimeClientFromEnv({ RIOT_API_KEY: "k" })).not.toBeNull();
  });

  it("builds match id urls with optional type/queue params", () => {
    expect(buildListIdsUrl("p", "europe", null, null, 10)).not.toContain("type=");
    const url = buildListIdsUrl("p", "europe", "ranked", 420, 5);
    expect(url).toContain("&type=ranked");
    expect(url).toContain("&queue=420");
  });

  it("retries once on 429 honoring Retry-After, throws on persistent failure", async () => {
    let first = true;
    const { fetch } = stubFetch([
      [
        /ok-after-429/,
        () => {
          if (first) {
            first = false;
            return { status: 429, retryAfterSecs: 1 };
          }
          return { status: 200, body: { ok: true } };
        },
      ],
      [/always-500/, () => ({ status: 500 })],
      [/forbidden/, () => ({ status: 403 })],
    ]);
    const client = new RiotClient("k", null, fetch, noSleep);
    await expect(client.get("https://x/ok-after-429")).resolves.toEqual({ ok: true });
    await expect(client.get("https://x/always-500")).rejects.toThrow(
      /after 5xx retry/,
    );
    await expect(client.get("https://x/forbidden")).rejects.toThrow(
      /Riot API error 403/,
    );
  });

  it("proxy mode rewrites the url and drops the token header", async () => {
    let seenUrl = "";
    let seenHeaders: Record<string, string> = {};
    const fetch: RiotFetch = async (url, headers) => {
      seenUrl = url;
      seenHeaders = headers;
      return { status: 200, json: async () => ({}) };
    };
    const client = new RiotClient("", "https://proxy.example/", fetch, noSleep);
    await client.get("https://tr1.api.riotgames.com/lol/x");
    expect(seenUrl).toBe("https://proxy.example/tr1.api.riotgames.com/lol/x");
    expect(seenHeaders["X-Riot-Token"]).toBeUndefined();
  });
});

describe("riot sync commands (riot.rs parity)", () => {
  // NOT: "key yok → hata" yolu makine env'ine bağımlı olduğundan (gerçek .env'de
  // RIOT_API_KEY olabilir → GERÇEK API'ye istek atar) burada test edilmez;
  // runtimeClientFromEnv({}) === null asserti yukarıda aynı kapıyı kanıtlıyor.
  function seedSummoner(db: DatabaseSync): void {
    db.prepare(
      "INSERT INTO summoners (puuid, game_name, tag_line, region, cached_at) VALUES ('p-1', 'T', 'TR', 'tr1', 0)",
    ).run();
  }

  it("sync_riot_player upserts the summoner", async () => {
    const db = migratedDb();
    const { fetch } = stubFetch([
      [
        /by-riot-id/,
        () => ({
          status: 200,
          body: { puuid: "p-1", gameName: "ELVEDA", tagLine: "SNB" },
        }),
      ],
    ]);
    const client = new RiotClient("k", null, fetch, noSleep);
    const info = await syncRiotPlayer(db, "ELVEDA", "SNB", "tr1", client);
    expect(info.puuid).toBe("p-1");
    const row = db
      .prepare("SELECT game_name, region FROM summoners WHERE puuid = 'p-1'")
      .get() as unknown as { game_name: string; region: string };
    expect(row).toMatchObject({ game_name: "ELVEDA", region: "tr1" });
  });

  it("sync_match_history fetches details and inserts matches idempotently", async () => {
    const db = migratedDb();
    seedSummoner(db); // matches.puuid FK — gerçek akışta sync_riot_player önce koşar
    const detail = (id: string, championId: number) => ({
      metadata: { matchId: id },
      info: {
        gameDuration: 1800,
        queueId: 420,
        gameStartTimestamp: 1_700_000_000_000,
        participants: [
          {
            puuid: "p-1",
            championId,
            teamPosition: "TOP",
            win: true,
            kills: 5,
            deaths: 2,
            assists: 3,
            totalMinionsKilled: 150,
            neutralMinionsKilled: 10,
            visionScore: 23,
          },
        ],
      },
    });
    // TR1_a timeline'ı: 10. dk frame'inde 71 CS, 14 dk öncesi 1 ölüm (victimId 1).
    const timelineA = {
      metadata: { participants: ["p-1"] },
      info: {
        frames: [
          { timestamp: 0, participantFrames: { "1": { minionsKilled: 0, jungleMinionsKilled: 0 } }, events: [] },
          {
            timestamp: 660_000,
            participantFrames: { "1": { minionsKilled: 65, jungleMinionsKilled: 6 } },
            events: [
              { type: "CHAMPION_KILL", timestamp: 620_000, victimId: 1 },
              { type: "CHAMPION_KILL", timestamp: 900_000, victimId: 1 }, // 15. dk → sayılmaz
              { type: "CHAMPION_KILL", timestamp: 700_000, victimId: 2 }, // başkası → sayılmaz
            ],
          },
        ],
      },
    };
    const { fetch } = stubFetch([
      [/\/ids\?/, () => ({ status: 200, body: ["TR1_a", "TR1_b", "TR1_err"] })],
      // timeline desenleri ÖNCE: /matches\/TR1_a/ timeline URL'sini de yakalar.
      [/TR1_a\/timeline/, () => ({ status: 200, body: timelineA })],
      [/TR1_b\/timeline/, () => ({ status: 500 })], // timeline hatası → alanlar null
      [/matches\/TR1_a/, () => ({ status: 200, body: detail("TR1_a", 86) })],
      [/matches\/TR1_b/, () => ({ status: 200, body: detail("TR1_b", 122) })],
      [/matches\/TR1_err/, () => ({ status: 403 })],
    ]);
    const client = new RiotClient("k", null, fetch, noSleep);

    const result = await syncMatchHistory(db, "p-1", 20, client);
    expect(result).toEqual({ synced: 2, skipped: 0, errors: 1 });
    const row = db
      .prepare(
        "SELECT champion_id, cs, cs_at_10, deaths_pre_14, vision_score FROM matches WHERE match_id = 'TR1_a'",
      )
      .get() as unknown as Record<string, unknown>;
    expect(row).toMatchObject({
      champion_id: 86,
      cs: 160,
      cs_at_10: 71,
      deaths_pre_14: 1,
      vision_score: 23,
    });
    // Timeline'ı başarısız maç: vision detail'den gelir, timeline alanları dürüst null.
    const rowB = db
      .prepare(
        "SELECT cs_at_10, deaths_pre_14, vision_score FROM matches WHERE match_id = 'TR1_b'",
      )
      .get() as unknown as Record<string, unknown>;
    expect(rowB).toMatchObject({ cs_at_10: null, deaths_pre_14: null, vision_score: 23 });

    // İkinci koşu: INSERT OR IGNORE → hepsi skipped.
    const again = await syncMatchHistory(db, "p-1", 20, client);
    expect(again.synced).toBe(0);
    expect(again.skipped).toBe(2);
  });

  it("sync_masteries upserts entries and snapshots once per points value", async () => {
    const db = migratedDb();
    seedSummoner(db); // mastery.puuid FK
    const { fetch } = stubFetch([
      [
        /champion-masteries/,
        () => ({
          status: 200,
          body: [
            { championId: 238, championLevel: 7, championPoints: 180000, lastPlayTime: 1 },
            { championId: 61, championLevel: 5, championPoints: 65000 },
          ],
        }),
      ],
    ]);
    const client = new RiotClient("k", null, fetch, noSleep);
    const result = await syncMasteries(db, "p-1", client);
    expect(result).toEqual({ synced: 2, skipped: 0, errors: 0 });

    // Aynı puanla ikinci koşu → snapshot ÇOĞALMAZ (Rust record_mastery_snapshot).
    await syncMasteries(db, "p-1", client);
    const snaps = db
      .prepare(
        "SELECT COUNT(*) AS c FROM mastery_snapshots WHERE puuid = 'p-1' AND champion_id = 238",
      )
      .get() as unknown as { c: number };
    expect(Number(snaps.c)).toBe(1);
  });
});

describe("LCU player-sync parsers (player_sync.rs/champ_pool.rs parity)", () => {
  it("parses the match-history fixture for the local player", () => {
    const matches = parseLcuMatchHistory(matchHistoryFixture(), LOCAL_PUUID);
    expect(matches).toHaveLength(2);
    expect(matches[0]).toMatchObject({
      match_id: "LCU_9876543210",
      champion_id: 238,
      position: "MIDDLE",
      win: true,
      kills: 8,
      cs: 192, // 180 + 12
      played_at: 1_700_000_000,
    });
    expect(parseLcuMatchHistory(matchHistoryFixture(), "yabancı")).toEqual([]);
    expect(parseLcuMatchHistory({}, LOCAL_PUUID)).toEqual([]);
  });

  it("parses mastery and summoner-name fallbacks", () => {
    const mastery = parseLcuMastery(masteryFixture());
    expect(mastery).toHaveLength(3);
    expect(mastery[0]).toMatchObject({ champion_id: 238, level: 7, points: 180000 });
    expect(parseLcuMastery(null)).toEqual([]);

    expect(parseLcuSummonerName({ gameName: "A", tagLine: "B" })).toEqual({
      gameName: "A",
      tagLine: "B",
    });
    expect(parseLcuSummonerName({ displayName: "Ad#TAG" })).toEqual({
      gameName: "Ad",
      tagLine: "TAG",
    });
    expect(parseLcuSummonerName({ displayName: "SadeAd" })).toEqual({
      gameName: "SadeAd",
      tagLine: "",
    });
    expect(parseLcuSummonerName({})).toEqual({ gameName: "Summoner", tagLine: "" });
  });

  // D2: şema canlı client'ta doğrulandı (2026-06-12, Platinum IV).
  it("parses ranked stats and skips unranked queues", () => {
    const fixture = {
      queueMap: {
        RANKED_SOLO_5x5: {
          tier: "PLATINUM",
          division: "IV",
          leaguePoints: 26,
          wins: 102,
          losses: 108,
          isProvisional: false,
        },
        RANKED_FLEX_SR: {
          tier: "NONE",
          division: "NA",
          leaguePoints: 0,
          wins: 0,
          losses: 0,
          isProvisional: false,
        },
        RANKED_TFT: { tier: "GOLD" }, // SR dışı kuyruk → atlanır
      },
    };
    const parsed = parseLcuRankedStats(fixture);
    expect(parsed).toHaveLength(1); // yalnız soloQ; flex NONE, TFT haritalanmaz
    expect(parsed[0]).toMatchObject({
      queue: "soloq",
      tier: "PLATINUM",
      division: "IV",
      league_points: 26,
      wins: 102,
      losses: 108,
      is_provisional: false,
    });
    // Master+ "NA" division → boş; provisional korunur.
    const master = parseLcuRankedStats({
      queueMap: {
        RANKED_SOLO_5x5: { tier: "MASTER", division: "NA", leaguePoints: 340, isProvisional: true },
      },
    });
    expect(master[0].division).toBe("");
    expect(master[0].is_provisional).toBe(true);
    expect(parseLcuRankedStats(null)).toEqual([]);
  });

});

describe("sync_lcu_player_data", () => {
  const lockfile = {
    name: "LeagueClient",
    pid: 1,
    port: 50123,
    password: "pw",
    protocol: "https" as const,
  };

  it("syncs summoner + matches + masteries from a stubbed LCU", async () => {
    const db = migratedDb();
    const service = new LcuService({
      emit: () => {},
      findLockfileFn: () => lockfile,
      makeClient: () => ({
        getJson: async <T>(path: string) => {
          if (path.includes("current-summoner")) {
            return { puuid: LOCAL_PUUID, gameName: "Me", tagLine: "TR" } as T;
          }
          if (path.includes("match-history")) return matchHistoryFixture() as T;
          if (path.includes("champion-mastery")) return masteryFixture() as T;
          throw new Error(`beklenmedik yol: ${path}`);
        },
      }),
    });

    const result = await syncLcuPlayerData(db, service, "tr1");
    expect(result.summoner).toMatchObject({
      puuid: LOCAL_PUUID,
      game_name: "Me",
      tag_line: "TR",
      region: "tr1",
    });
    expect(result.matches_synced).toBe(2);
    expect(result.masteries_synced).toBe(3);
    expect(result.errors).toBe(0);

    const masteryCount = db
      .prepare("SELECT COUNT(*) AS c FROM mastery WHERE puuid = ?")
      .get(LOCAL_PUUID) as unknown as { c: number };
    expect(Number(masteryCount.c)).toBe(3);
  });

});

describe("sync_recommendation_feedback (feedback_flush.rs parity)", () => {
  function seedFeedback(db: DatabaseSync, hash: string | null): number {
    db.prepare(
      `INSERT INTO recommendation_feedback
         (champion_id, champion_key, feedback, session_hash, created_at)
       VALUES (238, 'Zed', 'helpful', ?, 1000)`,
    ).run(hash);
    const row = db
      .prepare("SELECT last_insert_rowid() AS id")
      .get() as unknown as { id: number };
    return Number(row.id);
  }

  /** Opt in to anonymized feedback upload (default is off → flush no-ops). */
  function enableConsent(db: DatabaseSync): void {
    saveSettings(db, { ...DEFAULT_SETTINGS, share_anonymous_feedback: true });
  }

  it("returns offline:true when no cloud base is configured", async () => {
    const db = migratedDb();
    enableConsent(db);
    const summary = await syncRecommendationFeedback(engine, db, {});
    expect(summary).toEqual({
      offline: true,
      attempted: 0,
      synced: 0,
      failed: 0,
      skipped_no_hash: 0,
    });
  });

  it("sends due hashed rows with a core idempotency key; failures stay queued", async () => {
    const db = migratedDb();
    enableConsent(db);
    const hashedId = seedFeedback(db, "0123456789abcdef0123");
    seedFeedback(db, null); // privacy gate → asla gönderilmez

    const bodies: Record<string, unknown>[] = [];
    const summary = await syncRecommendationFeedback(
      engine,
      db,
      { DRAFT_BRAIN_API_BASE: "https://cloud.example" },
      async (url, _token, body) => {
        expect(url).toBe("https://cloud.example/v1/recommendation-feedback");
        bodies.push(body as Record<string, unknown>);
        return { ok: true };
      },
    );
    expect(summary).toMatchObject({
      offline: false,
      attempted: 1,
      synced: 1,
      failed: 0,
      skipped_no_hash: 1,
    });
    expect(bodies[0].idempotency_key).toMatch(/^[0-9a-f]{16}$/);
    expect(bodies[0].user_hash).toBe("0123456789abcdef0123");

    const row = db
      .prepare("SELECT synced_at FROM recommendation_feedback WHERE rowid = ?")
      .get(hashedId) as unknown as { synced_at: number | null };
    expect(row.synced_at).not.toBeNull();

    // İkinci koşu: synced satır kuyruğa girmez.
    const again = await syncRecommendationFeedback(
      engine,
      db,
      { DRAFT_BRAIN_API_BASE: "https://cloud.example" },
      async () => ({ ok: true }),
    );
    expect(again.attempted).toBe(0);
  });

  it("never uploads without consent — privacy gate (off by default)", async () => {
    const db = migratedDb(); // no enableConsent → share_anonymous_feedback stays false
    const id = seedFeedback(db, "0123456789abcdef0123");
    let posted = false;
    const summary = await syncRecommendationFeedback(
      engine,
      db,
      { DRAFT_BRAIN_API_BASE: "https://cloud.example" },
      async () => {
        posted = true;
        return { ok: true };
      },
    );
    expect(posted).toBe(false); // POST must never fire without consent
    expect(summary).toEqual({
      offline: true,
      attempted: 0,
      synced: 0,
      failed: 0,
      skipped_no_hash: 0,
    });
    const row = db
      .prepare("SELECT synced_at FROM recommendation_feedback WHERE rowid = ?")
      .get(id) as unknown as { synced_at: number | null };
    expect(row.synced_at).toBeNull(); // row stays queued for a future opted-in flush
  });

  it("a failed POST bumps retry bookkeeping without losing the row", async () => {
    const db = migratedDb();
    enableConsent(db);
    const id = seedFeedback(db, "0123456789abcdef0123");
    const summary = await syncRecommendationFeedback(
      engine,
      db,
      { DRAFT_BRAIN_API_BASE: "https://cloud.example" },
      async () => ({ ok: false, error: "HTTP 503" }),
    );
    expect(summary.failed).toBe(1);
    const row = db
      .prepare(
        "SELECT synced_at, retry_count, last_error, next_retry_at FROM recommendation_feedback WHERE rowid = ?",
      )
      .get(id) as unknown as {
      synced_at: number | null;
      retry_count: number;
      last_error: string;
      next_retry_at: number;
    };
    expect(row.synced_at).toBeNull();
    expect(Number(row.retry_count)).toBe(1);
    expect(row.last_error).toBe("HTTP 503");
    expect(Number(row.next_retry_at)).toBeGreaterThan(0);
  });
});

describe("data-quality read trio (data_quality.rs parity)", () => {
  const caches = { version: "", items: [], runeTrees: [] };

  it("registry + quality + trajectory run honestly on an empty DB", () => {
    const db = migratedDb();

    const registry = getDataSourceRegistry(engine, db) as {
      fallback_active: boolean;
      stale_sources: string[];
      sources: { source: string }[];
    };
    expect(registry.fallback_active).toBe(true);
    expect(registry.stale_sources).toContain("data_pack");
    expect(registry.sources.some((s) => s.source === "local_seed")).toBe(true);

    const quality = getPipelineQualityReport(engine, db, caches) as {
      status: string;
      actions: unknown[];
    };
    expect(typeof quality.status).toBe("string");
    expect(Array.isArray(quality.actions)).toBe(true);

    // Scheduler yok → ramp yok → dürüst "unknown"; summoner yok → V5 kapalı.
    const trajectory = getDataTrajectory(engine, db, caches) as {
      trajectory: string;
      quality_status: string;
      match_v5_enabled: boolean;
      match_v5_age_secs: number | null;
    };
    expect(trajectory.trajectory).toBe("unknown");
    expect(trajectory.quality_status).toBe(quality.status);
    expect(trajectory.match_v5_enabled).toBe(false);
    expect(trajectory.match_v5_age_secs).toBeNull();
  });

  it("a fresh cloud pack with live rates clears the fallback flag", () => {
    const db = migratedDb();
    const now = Math.floor(Date.now() / 1000);
    db.prepare(
      `INSERT INTO draft_brain_packs (kind, version, payload_json, source, fetched_at, expires_at)
       VALUES ('data_pack', 'cloud-data-v1', '{}', 'cloud', ?, ?)`,
    ).run(now, now + 86400);
    db.prepare(
      "INSERT INTO champions (champion_id, key, name, title, cached_at) VALUES (86, 'Garen', 'Garen', 't', 0)",
    ).run();
    db.prepare(
      `INSERT INTO champion_rates
         (champion_id, position, win_rate, pick_rate, ban_rate, sample_size, patch, source, confidence, cached_at)
       VALUES (86, 'top', 0.52, 0.08, 0.05, 5000, '16.10', 'riot_match_v5', 'high', ?)`,
    ).run(now);

    const registry = getDataSourceRegistry(engine, db) as {
      fallback_active: boolean;
      sources: { source: string }[];
    };
    expect(registry.fallback_active).toBe(false);
    expect(registry.sources.some((s) => s.source === "cloud_postgres")).toBe(true);

    // Kaynak satırları (rates:riot_match_v5 + pack:cloud) core'a akar; rapor
    // dolu veriyle de çalışır (risk türetimi core json_api testinde kapsanır).
    const quality = getPipelineQualityReport(engine, db, caches) as {
      status: string;
      confidence: string;
    };
    expect(typeof quality.status).toBe("string");
    expect(["high", "medium", "low"]).toContain(quality.confidence);
  });
});

describe("sync_data_pipeline (data_quality.rs parity, Match-V5 hariç)", () => {
  it("runs the refresh end-to-end: seeds imported, honest match_v5 skip, pack promoted", async () => {
    const db = migratedDb();
    const caches = { version: undefined, items: [], runeTrees: [] };
    const lcu = new LcuService({ emit: () => {} }); // bağlı değil → guard false

    // builds/champion_matchups FK'leri champions'a işaret eder; gerçek akışta
    // seed import TAM DDragon sync'inden sonra koşar. Testte seed'lerin
    // referansladığı şampiyonları önceden ekle (post-ddragon durumunun aynısı).
    const seedIds = new Set<number>();
    for (const file of [
      join(__dirname, "..", "resources", "seeds", "builds_seed.json"),
      join(__dirname, "..", "resources", "seeds", "meta", "matchup_seed.json"),
    ]) {
      for (const entry of JSON.parse(readFileSync(file, "utf8")) as {
        champion_id: number;
        opponent_id?: number;
      }[]) {
        seedIds.add(entry.champion_id);
        if (entry.opponent_id) seedIds.add(entry.opponent_id);
      }
    }
    const insertChamp = db.prepare(
      "INSERT OR IGNORE INTO champions (champion_id, key, name, title, cached_at) VALUES (?, ?, ?, '', 0)",
    );
    for (const id of seedIds) insertChamp.run(id, String(id), `Champion ${id}`);

    // DDragon stub'ı: 1 şampiyon + boş item/rune (ddragon.test.ts deseni).
    const ddragonFetch = async (url: string): Promise<unknown> => {
      if (url.includes("versions")) return ["14.9.1"];
      if (url.includes("champion.json")) {
        return {
          data: {
            Garen: { key: "86", id: "Garen", name: "Garen", title: "t" },
          },
        };
      }
      if (url.includes("item.json")) return { data: {} };
      if (url.includes("runesReforged")) return [];
      if (url.includes("champion-summary")) return [];
      throw new Error(`beklenmedik url: ${url}`);
    };
    // Meraki ölü (by design) → hata dürüstçe errors'a düşer.
    const merakiFetch = async () => {
      throw new Error("Meraki erişilemez");
    };

    const summary = await syncDataPipeline(
      engine,
      db,
      lcu,
      caches,
      ddragonFetch,
      merakiFetch,
      null, // Riot key yok — Rust paritesi: match_v5 sessiz 0'lı başarı
    );

    expect(summary.ddragon_champions).toBe(1);
    expect(summary.meraki_rates).toBe(0);
    expect(summary.errors.some((e) => e.startsWith("meraki:"))).toBe(true);
    // Seed'ler gerçek bundled dosyalardan içe aktarıldı.
    expect(summary.errors.filter((e) => !e.startsWith("meraki:") && !e.includes("match_v5"))).toEqual([]);
    expect(summary.builds_imported).toBeGreaterThan(0);
    expect(summary.matchups_imported).toBeGreaterThan(0);
    // Match-V5: key yok → 0'lı başarı (Rust paritesi), hata DEĞİL.
    expect(summary.match_v5_matches).toBe(0);
    expect(summary.errors.some((e) => e.includes("match_v5"))).toBe(false);
    const v5Log = db
      .prepare(
        "SELECT status, decision FROM source_fetch_log WHERE source = 'match_v5'",
      )
      .get() as unknown as { status: string; decision: string };
    expect(v5Log).toMatchObject({ status: "success", decision: "refresh" });
    // Cache promotion: mevcut cache yok + seed dolu aday → local_builder pack yazıldı.
    expect(summary.cache_promoted).toBe(true);
    expect(summary.data_pack_cached).toBe(true);
    const pack = db
      .prepare("SELECT source FROM draft_brain_packs WHERE kind = 'data_pack'")
      .get() as unknown as { source: string };
    expect(pack.source).toBe("local_builder");
    expect(typeof summary.before_status).toBe("string");
    expect(typeof summary.after_status).toBe("string");
  });
});

describe("Match-V5 ingestion (data_quality.rs sync_match_v5_ingestion parity)", () => {
  const matchDetail = {
    metadata: { matchId: "TR1_900" },
    info: {
      queueId: 420,
      gameVersion: "16.10.1",
      participants: [
        {
          participantId: 1, championId: 86, teamId: 100, teamPosition: "TOP",
          win: true, kills: 5, deaths: 2, assists: 3, puuid: "raw-puuid-a",
          summoner1Id: 4, summoner2Id: 12, item0: 3071,
          perks: { styles: [{ selections: [{ perk: 8010 }] }] },
        },
        {
          participantId: 2, championId: 122, teamId: 200, teamPosition: "TOP",
          win: false, kills: 2, deaths: 5, assists: 1, puuid: "raw-puuid-b",
          summoner1Id: 4, summoner2Id: 14, item0: 6630,
          perks: { styles: [{ selections: [{ perk: 8437 }] }] },
        },
      ],
    },
  };

  it("rolling budget admits up to max within the window", () => {
    const budget = new RollingBudget(120_000, 2);
    expect(budget.tryAcquire(0)).toBe(true);
    expect(budget.tryAcquire(1_000)).toBe(true);
    expect(budget.tryAcquire(2_000)).toBe(false); // pencere dolu → 429'a gitmez
    expect(budget.tryAcquire(121_000)).toBe(true); // pencere kaydı → kapasite döner
  });

  it("ingests a ranked match end-to-end and stores only hashed identities", async () => {
    const db = migratedDb();
    db.prepare(
      "INSERT INTO summoners (puuid, game_name, tag_line, region, cached_at) VALUES ('riot-puuid-me', 'Me', 'TR', 'tr1', 0)",
    ).run();

    const { fetch } = stubFetch([
      [/\/ids\?/, () => ({ status: 200, body: ["TR1_900"] })],
      [/matches\/TR1_900/, () => ({ status: 200, body: matchDetail })],
    ]);
    const riot = new RiotClient("k", null, fetch, noSleep);

    const outcome = await syncMatchV5Ingestion(
      engine,
      db,
      { version: "16.10.1", items: [], runeTrees: [] },
      riot,
      new RollingBudget(),
    );
    expect(outcome.fetched_matches).toBe(1);
    expect(outcome.matchups).toBeGreaterThan(0);
    expect(outcome.rates).toBeGreaterThan(0);

    // Maç işlendi olarak işaretlendi; canonical satırlar yazıldı.
    const hist = db
      .prepare("SELECT status, patch FROM match_v5_fetch_history WHERE match_id = 'TR1_900'")
      .get() as unknown as { status: string; patch: string };
    expect(hist).toMatchObject({ status: "processed", patch: "16.10" });
    const matchup = db
      .prepare(
        "SELECT games FROM champion_matchups WHERE champion_id = 86 AND opponent_id = 122",
      )
      .get() as unknown as { games: number };
    expect(Number(matchup.games)).toBe(1);

    // Privacy: discovery tablosunda HAM puuid YOK, yalnız FNV hash var.
    const players = db
      .prepare("SELECT puuid_hash FROM match_discovery_players")
      .all() as unknown as { puuid_hash: string }[];
    expect(players.length).toBeGreaterThan(0);
    for (const p of players) {
      expect(p.puuid_hash).toMatch(/^[0-9a-f]{16}$/);
      expect(p.puuid_hash).not.toContain("raw-puuid");
    }
    // TS hashPuuid ↔ core FNV paritesi (aktif oyuncu hash'i TS'te üretiliyor).
    expect(players.some((p) => p.puuid_hash === hashPuuid("riot-puuid-me"))).toBe(true);
    const ingest = engine.matchV5Ingest<{
      participants: { puuid: string; hash: string }[];
    }>({ details: [matchDetail], ids: ["TR1_900"], region: "tr1" });
    const coreA = ingest.participants.find((p) => p.puuid === "raw-puuid-a");
    expect(coreA?.hash).toBe(hashPuuid("raw-puuid-a"));
  });

  it("returns zeros without a Riot client or an active summoner", async () => {
    const db = migratedDb();
    const caches = { version: undefined, items: [], runeTrees: [] };
    expect(await syncMatchV5Ingestion(engine, db, caches, null)).toMatchObject({
      fetched_matches: 0,
    });
    const { fetch } = stubFetch([]);
    const riot = new RiotClient("k", null, fetch, noSleep);
    expect(await syncMatchV5Ingestion(engine, db, caches, riot)).toMatchObject({
      fetched_matches: 0,
    });
  });
});

describe("get_draft_brain_quality_report (draft_brain.rs parity)", () => {
  it("reports counters, pack freshness and the honest Turkish notes", () => {
    const db = migratedDb();
    const empty = getDraftBrainQualityReport(db, {}, 1_800_000_000);
    expect(empty.cloud_configured).toBe(false);
    expect(empty.local_rules_version).toBe("draft-brain-rules-v2");
    expect(empty.notes).toContain(
      "DRAFT_BRAIN_API_BASE yok; local rules/data fallback aktif",
    );
    expect(empty.notes).toContain(
      "Data pack cache boş; runtime local seed kullanır, sync_data_pack önerilir",
    );

    // Taze pack (23 saat) + 1 unsynced feedback.
    const now = 1_800_000_000;
    db.prepare(
      `INSERT INTO draft_brain_packs (kind, version, payload_json, source, fetched_at, expires_at)
       VALUES ('data_pack', 'cloud-data-v1', ?, 'cloud', ?, ?)`,
    ).run(
      JSON.stringify({
        version: "cloud-data-v1",
        confidence: "high",
        generated_at: now - 23 * 3600,
      }),
      now,
      now + 86400,
    );
    db.prepare(
      `INSERT INTO recommendation_feedback (champion_id, champion_key, feedback, created_at)
       VALUES (238, 'Zed', 'helpful', 1000)`,
    ).run();

    const report = getDraftBrainQualityReport(
      db,
      { DRAFT_BRAIN_API_BASE: "https://cloud.example" },
      now,
    );
    expect(report.cloud_configured).toBe(true);
    expect(report.data_pack_version).toBe("cloud-data-v1");
    expect(report.data_pack_fresh).toBe(true);
    expect(report.feedback_unsynced).toBe(1);
    expect(report.notes).toContain("1 feedback cloud sync bekliyor");

    // 25 saat eski → stale notu.
    db.prepare(
      "UPDATE draft_brain_packs SET payload_json = ? WHERE kind = 'data_pack'",
    ).run(
      JSON.stringify({
        version: "cloud-data-v1",
        confidence: "low",
        generated_at: now - 25 * 3600,
      }),
    );
    const stale = getDraftBrainQualityReport(db, {}, now);
    expect(stale.data_pack_fresh).toBe(false);
    expect(stale.notes).toContain(
      "Data pack generated_at 24 saatten eski; sync_data_pack önerilir",
    );
    expect(stale.notes).toContain(
      "Data pack confidence low; local/seed fallback sinyali baskın",
    );
  });
});
