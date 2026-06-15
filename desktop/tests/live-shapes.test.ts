// V3a — canlı-şekil parser doğrulaması. The outcome/overlay unit tests hand-feed
// already-parsed, minimal snake_case shapes (e.g. `{queue_id, local_player}`). They
// never prove the REAL Live wire shapes parse: the raw LCU /lol-champ-select/v1/
// session and the Live Client /liveclientdata/allgamedata payloads. A field rename
// or a shape drift on Riot's side would slip past every existing test and only
// surface mid-game. This runs the actual core parsers on faithful fixtures and
// then threads the parsed champ-select state through the real outcome-label path.

import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { afterEach, beforeAll, describe, expect, it } from "vitest";

import { OutcomeTracker, RecsCache } from "../src/main/commands/outcomes";
import { openDatabase, runMigrations } from "../src/main/db";
import { Engine } from "../src/main/engine";

const FIXTURES = join(__dirname, "fixtures");
const MIGRATIONS_DIR = join(__dirname, "..", "resources", "migrations");

function loadFixture(name: string): unknown {
  return JSON.parse(readFileSync(join(FIXTURES, name), "utf8"));
}

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
  dir = mkdtempSync(join(tmpdir(), "csa-liveshapes-"));
  const db = openDatabase(join(dir, "app.db"));
  runMigrations(db, MIGRATIONS_DIR);
  openDb = db;
  return db;
}

interface ParsedSession {
  queue_id: number;
  local_player: { champion_id: number };
  my_team: unknown[];
}

describe("live wire-shape parsers", () => {
  it("parses a raw LCU champ-select session and threads it through the outcome label", () => {
    const raw = loadFixture("lcu_champ_select_session.json");
    const parsed = engine.parseSession<ParsedSession>(raw);

    // Output is the parsed ChampSelectState (snake_case), NOT the raw session.
    expect(parsed.queue_id).toBe(420); // gameConfig.queueId → queue_id
    expect(parsed.local_player.champion_id).toBe(64); // localPlayerCellId=2 → cellId 2
    expect(parsed.my_team).toHaveLength(5);
    expect(parsed).not.toHaveProperty("actions"); // proves parsed, not raw passthrough

    // The real parsed shape must drive the outcome-label pipeline end-to-end.
    const db = migratedDb();
    const recs = new RecsCache();
    const tracker = new OutcomeTracker({
      getDb: () => db,
      recs,
      syncMatches: async () => {},
    });
    tracker.onGameflowPhase("ChampSelect");
    recs.set("puuid-live", [{ champion_id: 64, champion_key: "LeeSin", total_score: 0.9 }]);
    tracker.onChampSelectSession(parsed);
    tracker.onGameflowPhase("InProgress");

    const rows = db
      .prepare("SELECT champion_id, rec_rank, context_json FROM recommendation_outcomes")
      .all() as unknown as Record<string, unknown>[];
    expect(rows).toHaveLength(1);
    expect(Number(rows[0].champion_id)).toBe(64);
    expect(Number(rows[0].rec_rank)).toBe(1);
    const ctx = JSON.parse(String(rows[0].context_json)) as { allies: number[] };
    // myTeam champions minus the local player (64), in wire order.
    expect(ctx.allies).toEqual([51, 111, 238, 122]);
  });

  it("accepts a real Live Client allgamedata shape (macro state + ingame plan)", () => {
    const raw = loadFixture("lcu_allgamedata.json");

    // gameData + events present → a populated MacroState object (objective timers).
    const macro = engine.macroStateFromAllgamedata<unknown>(raw);
    expect(macro).not.toBeNull();
    expect(typeof macro).toBe("object");

    // ingame plan resolves the active player's champion vs all_champions; with no
    // champions seeded it returns null, but MUST NOT throw on the real shape — the
    // point here is shape-acceptance, not a specific plan.
    expect(() => engine.ingamePlan({ allgamedata: raw, all_champions: [] })).not.toThrow();
  });
});
