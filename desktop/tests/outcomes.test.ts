// Sprint 2B — match-outcome label testleri: RecsCache + recordRecommendationPick
// + resolveRecommendationOutcomes + OutcomeTracker. Gerçek migrated DB (V026
// dahil) + repos-write yardımcılarıyla; uydurma kolon yok.

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { afterEach, describe, expect, it } from "vitest";

import {
  OutcomeTracker,
  RecsCache,
  recordRecommendationPick,
  resolveRecommendationOutcomes,
} from "../src/main/commands/outcomes";
import { openDatabase, runMigrations } from "../src/main/db";
import { ensureChampion, insertMatch, upsertSummoner } from "../src/main/repos-write";

const MIGRATIONS_DIR = join(__dirname, "..", "resources", "migrations");
const PUUID = "puuid-local";
const NOW = 1_700_000_000; // sabit unix saniye (deterministik pencere testi)

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
  dir = mkdtempSync(join(tmpdir(), "csa-outcomes-"));
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

function insertMatchRow(
  db: DatabaseSync,
  m: { match_id: string; champion_id: number; win: boolean; queue_id: number; played_at: number },
): void {
  upsertSummoner(db, { puuid: PUUID, game_name: "T", tag_line: "TR", region: "tr1" });
  ensureChampion(db, m.champion_id);
  insertMatch(db, PUUID, {
    match_id: m.match_id,
    champion_id: m.champion_id,
    position: "jungle",
    win: m.win,
    kills: 0,
    deaths: 0,
    assists: 0,
    duration_secs: 1800,
    queue_id: m.queue_id,
    played_at: m.played_at,
    cs: 0,
  });
}

function recsFor(puuid: string): RecsCache {
  const recs = new RecsCache();
  recs.set(puuid, [
    { champion_id: 64, champion_key: "LeeSin", total_score: 0.91 },
    { champion_id: 121, champion_key: "Khazix", total_score: 0.77 },
  ]);
  return recs;
}

function pendingRows(db: DatabaseSync): Record<string, unknown>[] {
  return db
    .prepare("SELECT * FROM recommendation_outcomes ORDER BY id")
    .all() as unknown as Record<string, unknown>[];
}

describe("RecsCache", () => {
  it("ranks recs 1-based, looks up + clears", () => {
    const recs = recsFor(PUUID);
    expect(recs.isEmpty()).toBe(false);
    expect(recs.puuidValue()).toBe(PUUID);
    expect(recs.lookup(64)).toMatchObject({
      champion_id: 64,
      champion_key: "LeeSin",
      score: 0.91,
      rank: 1,
    });
    expect(recs.lookup(121)?.rank).toBe(2);
    expect(recs.lookup(999)).toBeNull(); // liste-dışı
    recs.clear();
    expect(recs.isEmpty()).toBe(true);
    expect(recs.puuidValue()).toBe("");
  });

  it("skips malformed rec entries (no champion_id)", () => {
    const recs = new RecsCache();
    recs.set(PUUID, [{ foo: 1 }, { champion_id: 64, champion_key: "LeeSin", total_score: 0.5 }]);
    expect(recs.lookup(64)?.rank).toBe(1); // bozuk giriş atlandı → tek geçerli rec rank 1
  });
});

describe("recordRecommendationPick", () => {
  it("writes a pending row with rank/score for an in-list pick", () => {
    const db = migratedDb();
    const id = recordRecommendationPick(
      db,
      recsFor(PUUID),
      { champion_id: 64, queue_id: 420, position: "jungle" },
      NOW,
    );
    expect(id).not.toBeNull();
    const rows = pendingRows(db);
    expect(rows).toHaveLength(1);
    const r = rows[0];
    expect(r.champion_id).toBe(64);
    expect(r.champion_key).toBe("LeeSin");
    expect(r.queue_id).toBe(420);
    expect(r.rec_rank).toBe(1);
    expect(r.rec_score).toBeCloseTo(0.91);
    expect(r.win).toBeNull(); // pending
    expect(r.resolved_at).toBeNull();
    const ctx = JSON.parse(String(r.context_json));
    expect(ctx.picked).toBe(64);
    expect(ctx.recommended).toHaveLength(2);
  });

  it("records an off-list pick with NULL rank/score + key from champions", () => {
    const db = migratedDb();
    seedChampion(db, 222, "Draven");
    const id = recordRecommendationPick(
      db,
      recsFor(PUUID),
      { champion_id: 222, queue_id: 420, position: "bottom" },
      NOW,
    );
    expect(id).not.toBeNull();
    const r = pendingRows(db)[0];
    expect(r.champion_id).toBe(222);
    expect(r.champion_key).toBe("Draven"); // championKeyById fallback
    expect(r.rec_rank).toBeNull();
    expect(r.rec_score).toBeNull();
  });

  it("returns null + writes nothing when no recs were shown", () => {
    const db = migratedDb();
    const id = recordRecommendationPick(
      db,
      new RecsCache(),
      { champion_id: 64, queue_id: 420, position: "jungle" },
      NOW,
    );
    expect(id).toBeNull();
    expect(pendingRows(db)).toHaveLength(0);
  });

  it("dedups the same pending (puuid,champion,queue) within the window", () => {
    const db = migratedDb();
    const recs = recsFor(PUUID);
    const first = recordRecommendationPick(db, recs, { champion_id: 64, queue_id: 420, position: "jungle" }, NOW);
    const second = recordRecommendationPick(db, recs, { champion_id: 64, queue_id: 420, position: "jungle" }, NOW + 60);
    expect(first).not.toBeNull();
    expect(second).toBeNull();
    expect(pendingRows(db)).toHaveLength(1);
  });
});

describe("resolveRecommendationOutcomes", () => {
  it("binds a pending pick to its real match (win + match_id)", () => {
    const db = migratedDb();
    recordRecommendationPick(db, recsFor(PUUID), { champion_id: 64, queue_id: 420, position: "jungle" }, NOW);
    insertMatchRow(db, { match_id: "TR1_1", champion_id: 64, win: true, queue_id: 420, played_at: NOW + 120 });

    const res = resolveRecommendationOutcomes(db, NOW + 3600);
    expect(res.resolved).toBe(1);
    expect(res.pending).toBe(0);
    const r = pendingRows(db)[0];
    expect(r.match_id).toBe("TR1_1");
    expect(r.win).toBe(1);
    expect(r.resolved_at).toBe(NOW + 3600);
  });

  it("leaves a pick pending when no match matches (champ/queue/window)", () => {
    const db = migratedDb();
    recordRecommendationPick(db, recsFor(PUUID), { champion_id: 64, queue_id: 420, position: "jungle" }, NOW);
    // farklı şampiyon + pencere dışı → eşleşme yok
    insertMatchRow(db, { match_id: "TR1_2", champion_id: 121, win: true, queue_id: 420, played_at: NOW + 120 });
    insertMatchRow(db, { match_id: "TR1_3", champion_id: 64, win: false, queue_id: 420, played_at: NOW + 50 * 3600 });

    const res = resolveRecommendationOutcomes(db, NOW + 3600);
    expect(res.resolved).toBe(0);
    expect(res.pending).toBe(1);
    expect(pendingRows(db)[0].resolved_at).toBeNull();
  });

  it("binds two picks to distinct matches (no double-link, nearest wins)", () => {
    const db = migratedDb();
    // iki ayrı champ-select'te aynı şampiyon iki kez oynandı
    recordRecommendationPick(db, recsFor(PUUID), { champion_id: 64, queue_id: 420, position: "jungle" }, NOW);
    recordRecommendationPick(db, recsFor(PUUID), { champion_id: 64, queue_id: 420, position: "jungle" }, NOW + 10 * 3600);
    insertMatchRow(db, { match_id: "G_A", champion_id: 64, win: true, queue_id: 420, played_at: NOW + 100 });
    insertMatchRow(db, { match_id: "G_B", champion_id: 64, win: false, queue_id: 420, played_at: NOW + 10 * 3600 + 100 });

    const res = resolveRecommendationOutcomes(db, NOW);
    expect(res.resolved).toBe(2);
    const rows = pendingRows(db);
    const byMatch = new Set(rows.map((r) => r.match_id));
    expect(byMatch).toEqual(new Set(["G_A", "G_B"])); // her maç tek pick'e bağlı
    expect(rows.find((r) => r.match_id === "G_A")?.win).toBe(1);
    expect(rows.find((r) => r.match_id === "G_B")?.win).toBe(0);
  });

  it("is idempotent — a second resolve does not re-bind", () => {
    const db = migratedDb();
    recordRecommendationPick(db, recsFor(PUUID), { champion_id: 64, queue_id: 420, position: "jungle" }, NOW);
    insertMatchRow(db, { match_id: "TR1_1", champion_id: 64, win: true, queue_id: 420, played_at: NOW + 120 });
    expect(resolveRecommendationOutcomes(db, NOW).resolved).toBe(1);
    expect(resolveRecommendationOutcomes(db, NOW).resolved).toBe(0);
  });
});

describe("OutcomeTracker (gameflow-driven)", () => {
  function tracker(db: DatabaseSync, recs: RecsCache) {
    let syncCalls = 0;
    const t = new OutcomeTracker({
      getDb: () => db,
      recs,
      syncMatches: async () => {
        syncCalls += 1;
      },
    });
    return { t, syncCalls: () => syncCalls };
  }

  it("records the committed pick once on InProgress, resets on a new ChampSelect", () => {
    const db = migratedDb();
    const recs = new RecsCache();
    const { t } = tracker(db, recs);

    // gerçek akış: ChampSelect fazı recs'i temizler → renderer get_recommendations çağırır
    t.onGameflowPhase("ChampSelect");
    recs.set(PUUID, [{ champion_id: 64, champion_key: "LeeSin", total_score: 0.9 }]);
    // hover (champion_id 0) yok sayılır; commit (64) hatırlanır
    t.onChampSelectSession({ queue_id: 420, local_player: { champion_id: 0, assigned_position: "jungle" } });
    t.onChampSelectSession({ queue_id: 420, local_player: { champion_id: 64, assigned_position: "jungle" } });
    t.onGameflowPhase("InProgress");
    t.onGameflowPhase("InProgress"); // idempotent

    const rows = pendingRows(db);
    expect(rows).toHaveLength(1);
    expect(rows[0].champion_id).toBe(64);
    expect(rows[0].rec_rank).toBe(1);

    // yeni champ-select → cache temizlenir, recs yoksa pick yazılmaz
    t.onGameflowPhase("ChampSelect");
    expect(recs.isEmpty()).toBe(true);
    t.onChampSelectSession({ queue_id: 420, local_player: { champion_id: 99, assigned_position: "mid" } });
    t.onGameflowPhase("InProgress");
    expect(pendingRows(db)).toHaveLength(1); // recs boş → yeni satır yok
  });

  it("triggers match sync once on a postgame phase", () => {
    const db = migratedDb();
    const { t, syncCalls } = tracker(db, new RecsCache());
    t.onGameflowPhase("InProgress");
    t.onGameflowPhase("EndOfGame");
    t.onGameflowPhase("EndOfGame"); // aynı oyun-sonu için tek tetik
    expect(syncCalls()).toBe(1);
  });

  it("end-to-end: pick → match ingest → resolve", () => {
    const db = migratedDb();
    const recs = new RecsCache();
    const { t } = tracker(db, recs);
    t.onGameflowPhase("ChampSelect");
    recs.set(PUUID, [{ champion_id: 64, champion_key: "LeeSin", total_score: 0.9 }]);
    t.onChampSelectSession({ queue_id: 420, local_player: { champion_id: 64, assigned_position: "jungle" } });
    t.onGameflowPhase("InProgress");
    // oyun-sonu maç ingest (player-data sync'in yaptığı işi taklit et)
    insertMatchRow(db, { match_id: "TR1_9", champion_id: 64, win: true, queue_id: 420, played_at: Math.floor(Date.now() / 1000) });
    const res = resolveRecommendationOutcomes(db);
    expect(res.resolved).toBe(1);
    expect(pendingRows(db)[0].win).toBe(1);
  });

  it("captures ally champions from my_team into context_json (3C)", () => {
    const db = migratedDb();
    const recs = new RecsCache();
    const { t } = tracker(db, recs);
    t.onGameflowPhase("ChampSelect");
    recs.set(PUUID, [{ champion_id: 64, champion_key: "LeeSin", total_score: 0.9 }]);
    t.onChampSelectSession({
      queue_id: 420,
      local_player: { champion_id: 64, assigned_position: "jungle" },
      my_team: [
        { champion_id: 64 }, // kendisi → hariç
        { champion_id: 51 },
        { champion_id: 0 }, // hover/boş → hariç
        { champion_id: 222 },
      ],
    });
    t.onGameflowPhase("InProgress");
    const ctx = JSON.parse(String(pendingRows(db)[0].context_json));
    expect(ctx.allies).toEqual([51, 222]);
  });

  it("retries the pick record on the next in-game phase when the first DB write throws", () => {
    const realDb = migratedDb();
    const recs = new RecsCache();
    let mode: "throw" | "real" = "real";
    // İlk denemede DB INSERT'ini throw ettir (geçici DB hatası simülasyonu).
    const throwingDb = {
      prepare: () => {
        throw new Error("db busy");
      },
    } as unknown as DatabaseSync;
    const t = new OutcomeTracker({
      getDb: () => (mode === "throw" ? throwingDb : realDb),
      recs,
      syncMatches: async () => {},
    });

    t.onGameflowPhase("ChampSelect");
    recs.set(PUUID, [{ champion_id: 64, champion_key: "LeeSin", total_score: 0.9 }]);
    t.onChampSelectSession({ queue_id: 420, local_player: { champion_id: 64, assigned_position: "jungle" } });

    // 1. IN_GAME: DB yazımı throw → kayıt yok; pickRecorded false KALMALI (yoksa retry ölür).
    mode = "throw";
    t.onGameflowPhase("InProgress");
    expect(pendingRows(realDb)).toHaveLength(0);

    // 2. IN_GAME: gerçek DB → retry çalışır, eğitim etiketi yazılır.
    mode = "real";
    t.onGameflowPhase("InProgress");
    expect(pendingRows(realDb)).toHaveLength(1);
    expect(pendingRows(realDb)[0].champion_id).toBe(64);
  });
});
