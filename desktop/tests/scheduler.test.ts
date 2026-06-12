// Arka plan pipeline scheduler testleri — gerçek WASM core + gerçek migration'lı
// DB ile uçtan uca tick (plan kararları core'da), champ-select guard'ı,
// trajectory'nin "unknown"dan çıkışı ve zamanlayıcı davranışı. Ağ YOK: tüm
// fetch'ler stub, Riot client null/stub.

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import { getDataTrajectory } from "../src/main/commands/data-quality";
import { LcuService } from "../src/main/commands/lcu";
import { openDatabase, runMigrations } from "../src/main/db";
import { Engine } from "../src/main/engine";
import { PipelineScheduler } from "../src/main/scheduler";

const MIGRATIONS_DIR = join(__dirname, "..", "resources", "migrations");

let engine: Engine;
beforeAll(() => {
  engine = Engine.load();
});

let dir: string | undefined;
let openDb: DatabaseSync | undefined;
afterEach(() => {
  vi.useRealTimers();
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
  dir = mkdtempSync(join(tmpdir(), "csa-sched-"));
  const db = openDatabase(join(dir, "app.db"));
  runMigrations(db, MIGRATIONS_DIR);
  openDb = db;
  return db;
}

/** DDragon stub'ı: Garen + boş item/rune (sync-commands.test.ts deseni). */
const ddragonStub = async (url: string): Promise<unknown> => {
  if (url.includes("versions")) return ["14.9.1"];
  if (url.includes("champion.json")) {
    return { data: { Garen: { key: "86", id: "Garen", name: "Garen", title: "t" } } };
  }
  if (url.includes("item.json")) return { data: {} };
  if (url.includes("runesReforged")) return [];
  if (url.includes("champion-summary")) return [];
  throw new Error(`beklenmedik url: ${url}`);
};

/** Meraki stub'ı: Garen top için 1 rate satırı (sayısal id anahtarı). */
const merakiStub = async () => ({
  patch: "14.9",
  data: { "86": { TOP: { playRate: 0.1, winRate: 0.51, banRate: 0.02, count: 500 } } },
});

/** u.gg stub'ı: her şampiyon için jungle overview + 1 yüksek-örneklem matchup. */
const uggStub = async (url: string): Promise<unknown> => {
  if (url.includes("/overview/")) {
    return {
      "12": {
        "8": {
          "1": [
            [
              [26, 17, 8000, 8300, [8010, 8299]],
              [9801, 5080, [4, 11]],
              [63, 40, [1102]],
              [61, 39, [6692, 6610]],
              [285, 157, ["Q"], "Q"],
              [[], [], [], [], [], []],
              [5081, 9803],
              false,
              [101, 58, ["5005"]],
              [], [], [], [0],
            ],
            "t",
          ],
        },
      },
    };
  }
  if (url.includes("/matchups/")) {
    return { "12": { "8": { "1": [[[104, 300, 600, 0, 0]], "t"] } } };
  }
  throw new Error(`beklenmedik u.gg url: ${url}`);
};

/** Leaguepedia stub'ı: Garen'in pick+ban olduğu tek pro draft. */
const leaguepediaStub = async () => ({
  cargoquery: [
    { title: { Team1Pick1: "Garen", Team2Ban1: "Garen", "DateTime UTC": "2026-06-10 12:00:00" } },
  ],
});

function fetchLogs(db: DatabaseSync): { source: string; status: string; decision: string }[] {
  return db
    .prepare("SELECT source, status, decision FROM source_fetch_log ORDER BY id")
    .all() as unknown as { source: string; status: string; decision: string }[];
}

function logFor(
  logs: { source: string; status: string; decision: string }[],
  source: string,
) {
  const row = logs.find((l) => l.source === source);
  expect(row, `${source} fetch-log satırı bekleniyordu`).toBeDefined();
  return row!;
}

describe("PipelineScheduler.tick (run_scheduler_tick paritesi)", () => {
  it("runs a full tick: core plan drives refresh/skip, ramp recorded, trajectory leaves unknown", async () => {
    const db = migratedDb();
    const caches = { version: undefined, items: [], runeTrees: [] };
    const lcu = new LcuService({ emit: () => {} }); // bağlı değil → champ-select false

    const scheduler = new PipelineScheduler({
      engine,
      db,
      lcu,
      caches,
      ddragonFetch: ddragonStub,
      merakiFetch: merakiStub,
      uggFetch: uggStub,
      leaguepediaFetch: leaguepediaStub,
      riotClient: null, // key yok → match_v5 plan'da skip_disabled
      edgeBaseUrl: null, // edge worker yapılandırılmadı → cloud_edge skip_disabled
    });

    // İlk tick öncesi trajectory dürüstçe "unknown".
    const before = getDataTrajectory(engine, db, caches, scheduler.lastCoverageRamp()) as {
      trajectory: string;
    };
    expect(before.trajectory).toBe("unknown");

    await scheduler.tick();

    const logs = fetchLogs(db);
    expect(logFor(logs, "ddragon")).toMatchObject({ status: "success", decision: "refresh" });
    // Meraki A5 kararı: kaynak bozuk/bayat → disabled-by-default, dürüst skip.
    expect(logFor(logs, "meraki")).toMatchObject({
      status: "skipped",
      decision: "skip_disabled",
    });
    // Aggregate kaynaklar da taşındı → gerçek refresh (stub fetch'lerle).
    expect(logFor(logs, "u_gg")).toMatchObject({ status: "success", decision: "refresh" });
    expect(logFor(logs, "leaguepedia")).toMatchObject({
      status: "success",
      decision: "refresh",
    });
    // Riot key yok → match_v5 dürüst skip_disabled.
    expect(logFor(logs, "match_v5")).toMatchObject({
      status: "skipped",
      decision: "skip_disabled",
    });
    // Edge worker URL'i yok → cloud_edge dürüst skip_disabled.
    expect(logFor(logs, "cloud_edge")).toMatchObject({
      status: "skipped",
      decision: "skip_disabled",
    });
    // u.gg canonical satırları + Leaguepedia pro presence gerçekten yazıldı.
    const uggRates = db
      .prepare("SELECT COUNT(*) AS c FROM champion_rates WHERE source = 'u_gg'")
      .get() as unknown as { c: number };
    expect(Number(uggRates.c)).toBeGreaterThan(0);
    const proRows = db
      .prepare(
        "SELECT COUNT(*) AS c FROM champion_rates WHERE source = 'leaguepedia' AND position = 'pro'",
      )
      .get() as unknown as { c: number };
    expect(Number(proRows.c)).toBe(1); // Garen pick+ban
    // Başarı vardı → promotion adımı koştu (karar core'da; sonuç success|skipped).
    expect(["success", "skipped"]).toContain(logFor(logs, "data_pack_cache").status);

    // u.gg satırları yazıldı → coverage büyüdü → progressing.
    const ramp = scheduler.lastCoverageRamp();
    expect(ramp).not.toBeNull();
    expect(ramp!.ramp_state).toBe("progressing");
    expect(ramp!.data_growing).toBe(true);

    // Trajectory artık "unknown" değil: hedefin altında ama büyüyor → warming_up.
    const after = getDataTrajectory(engine, db, caches, ramp) as {
      trajectory: string;
      ramp_state: string;
    };
    expect(after.ramp_state).toBe("progressing");
    expect(after.trajectory).toBe("warming_up");

    // İkinci tick: kaynaklar TTL içinde + healthy → skip_fresh, yeniden fetch YOK.
    // (meraki disabled → skip_fresh değil skip_disabled üretir.)
    await scheduler.tick();
    const second = fetchLogs(db).filter((l) => l.decision === "skip_fresh");
    for (const source of ["ddragon", "u_gg", "leaguepedia"]) {
      expect(second.some((l) => l.source === source), `${source} skip_fresh`).toBe(true);
    }
  });

  it("defers everything during champ-select and records an honest no_budget ramp", async () => {
    const db = migratedDb();
    const caches = { version: undefined, items: [], runeTrees: [] };

    // Bağlı LCU: gameflow-phase = ChampSelect (ağ refresh'i yasak penceresi).
    const lcu = new LcuService({
      emit: () => {},
      findLockfileFn: () => ({
        name: "LeagueClient",
        pid: 1,
        port: 50123,
        password: "pw",
        protocol: "https",
      }),
      makeClient: () => ({
        getJson: async <T,>(path: string) =>
          (path.includes("gameflow-phase")
            ? "ChampSelect"
            : { gameName: "A", tagLine: "B" }) as T,
      }),
    });
    lcu.startWsListener = () => {};
    await lcu.connect();

    const failIfFetched = async () => {
      throw new Error("champ-select sırasında ağ çağrısı yapılmamalı");
    };
    const scheduler = new PipelineScheduler({
      engine,
      db,
      lcu,
      caches,
      ddragonFetch: failIfFetched,
      merakiFetch: failIfFetched,
      uggFetch: failIfFetched,
      leaguepediaFetch: failIfFetched,
      riotClient: null,
    });

    await scheduler.tick();

    const logs = fetchLogs(db);
    for (const source of ["ddragon", "meraki", "u_gg", "leaguepedia", "match_v5", "cloud_edge"]) {
      expect(logFor(logs, source)).toMatchObject({
        status: "skipped",
        decision: "skip_champ_select",
      });
    }
    // Hiç başarı yok → promotion adımı koşmadı.
    expect(logs.some((l) => l.source === "data_pack_cache")).toBe(false);

    // Ertelenen tick dürüst no_budget → trajectory "deferred" (sahte ölçüm yok).
    const ramp = scheduler.lastCoverageRamp();
    expect(ramp!.ramp_state).toBe("no_budget");
    const view = getDataTrajectory(engine, db, caches, ramp) as { trajectory: string };
    expect(view.trajectory).toBe("deferred");
  });

  it("start() schedules the first tick after the initial delay and stop() halts the loop", async () => {
    vi.useFakeTimers();
    const db = migratedDb();
    const lcu = new LcuService({ emit: () => {} });
    const scheduler = new PipelineScheduler({
      engine,
      db,
      lcu,
      caches: { version: undefined, items: [], runeTrees: [] },
      edgeBaseUrl: null,
      initialDelayMs: 1_000,
      intervalMs: 5_000,
    });
    const tick = vi.spyOn(scheduler, "tick").mockResolvedValue(undefined);

    scheduler.start();
    scheduler.start(); // idempotent — ikinci çağrı ikinci zincir başlatmaz
    expect(tick).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1_000);
    expect(tick).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(5_000);
    expect(tick).toHaveBeenCalledTimes(2);

    scheduler.stop();
    await vi.advanceTimersByTimeAsync(20_000);
    expect(tick).toHaveBeenCalledTimes(2);
  });
});
