// get_recommendations port testi — GERÇEK WASM motoru + GERÇEK migration'lı DB ile
// uçtan uca: repos okumaları → core blend/feedback/parse → engine.recommendations.

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { afterEach, beforeAll, describe, expect, it } from "vitest";

import {
  getBanSuggestions,
  getChampionAnalysis,
  getChampionArchetypes,
  getChampionDetail,
  getDraftFork,
  getDraftSimulation,
  getGamePlan,
  getTeamComp,
} from "../src/main/commands/analysis";
import { getIngamePlan, getMacroState } from "../src/main/commands/overlay";
import {
  getChampionPoolPlan,
  getFeedbackObservability,
  getPoolSuggestions,
} from "../src/main/commands/insights";
import {
  getActiveSummonerPuuid,
  getMasteryProgress,
  getPerformanceReport,
  getPlayerStats,
  submitRecommendationFeedback,
} from "../src/main/commands/player";
import {
  getRecommendations,
  parseSessionArg,
} from "../src/main/commands/recommendations";
import { openDatabase, runMigrations } from "../src/main/db";
import { Engine } from "../src/main/engine";

const MIGRATIONS_DIR = join(__dirname, "..", "resources", "migrations");
const PUUID = "test-puuid";

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

function seededDb(): DatabaseSync {
  dir = mkdtempSync(join(tmpdir(), "csa-rec-"));
  const db = openDatabase(join(dir, "app.db"));
  runMigrations(db, MIGRATIONS_DIR);
  openDb = db;

  db.prepare(
    "INSERT INTO summoners (puuid, game_name, tag_line, region, cached_at) VALUES (?, ?, ?, ?, 0)",
  ).run(PUUID, "Test", "TR1", "tr1");

  const champ = db.prepare(
    "INSERT INTO champions (champion_id, key, name, title, cached_at) VALUES (?, ?, ?, ?, 0)",
  );
  champ.run(86, "Garen", "Garen", "Demacia'nın Gücü");
  champ.run(122, "Darius", "Darius", "Noxus'un Eli");
  champ.run(54, "Malphite", "Malphite", "Monolitin Parçası");
  champ.run(103, "Ahri", "Ahri", "Dokuz Kuyruklu Tilki");

  const meta = db.prepare(
    "INSERT INTO champion_meta (champion_id, roles, is_melee, attack_range) VALUES (?, ?, ?, ?)",
  );
  meta.run(86, '["fighter","tank"]', 1, 175);
  meta.run(122, '["fighter","tank"]', 1, 175);
  meta.run(54, '["tank","mage"]', 1, 125);
  meta.run(103, '["mage","assassin"]', 0, 550);

  db.prepare(
    "INSERT INTO mastery (puuid, champion_id, mastery_level, mastery_points, last_play_time) VALUES (?, ?, ?, ?, ?)",
  ).run(PUUID, 86, 7, 250_000, null);

  const rate = db.prepare(
    `INSERT INTO champion_rates
       (champion_id, position, win_rate, pick_rate, ban_rate, sample_size, patch, source, confidence, cached_at)
     VALUES (?, ?, ?, ?, ?, ?, '14.1', ?, 'high', 0)`,
  );
  rate.run(86, "top", 0.52, 0.08, 0.05, 5000, "u.gg");
  rate.run(54, "top", 0.5, 0.04, 0.01, 3000, "u.gg");

  db.prepare(
    `INSERT INTO champion_matchups
       (champion_id, opponent_id, position, games, wins, win_rate, source, patch_version, cached_at)
     VALUES (?, ?, 'top', ?, ?, ?, 'seed', '14.1', 0)`,
  ).run(86, 122, 400, 220, 0.55);

  const fb = db.prepare(
    "INSERT INTO recommendation_feedback (champion_id, champion_key, feedback, created_at) VALUES (?, ?, ?, 0)",
  );
  fb.run(86, "Garen", "helpful");
  fb.run(86, "Garen", "helpful");

  return db;
}

interface RecLike {
  champion_id: number;
  total_score: number;
}

/** Parse edilmiş (snake_case) state — renderer'ın round-trip ettiği şekil. */
function parsedTopSession() {
  const slot = (cellId: number, championId: number, position: string) => ({
    cell_id: cellId,
    champion_id: championId,
    intent_champion_id: 0,
    assigned_position: position,
    is_locked: championId !== 0,
  });
  return {
    my_cell_id: 0,
    local_player: slot(0, 0, "top"),
    my_team: [slot(0, 0, "top")],
    their_team: [slot(5, 122, "top")],
    my_bans: [],
    their_bans: [],
    phase: "BAN_PICK",
    time_left_ms: 27_000,
    action_type: "pick",
    queue_id: 420,
    pick_order: 2,
  };
}

describe("get_recommendations (champ_select.rs parity)", () => {
  it("scores candidates end-to-end from a parsed session", () => {
    const db = seededDb();
    const recs = getRecommendations(
      engine,
      db,
      parsedTopSession(),
      PUUID,
    ) as RecLike[];

    expect(recs.length).toBeGreaterThan(0);
    const scores = recs.map((r) => r.total_score);
    expect(scores).toEqual([...scores].sort((a, b) => b - a));
    // Yüksek mastery + top meta + olumlu feedback → Garen aday listesinde.
    expect(recs.some((r) => r.champion_id === 86)).toBe(true);
    // Rakibin kilitlediği şampiyon önerilemez.
    expect(recs.some((r) => r.champion_id === 122)).toBe(false);
  });

  it("enriches recs with seed builds, pro presence and the DraftBrain upgrade", () => {
    const db = seededDb();
    // Position-default curated build for Garen (opponent_archetype NULL).
    db.prepare(
      `INSERT INTO builds
         (champion_id, position, patch_version, item_ids, rune_ids, win_rate,
          pick_rate, source, skill_order, cached_at)
       VALUES (86, 'top', '14.1', '[3078,3742,3065,3026,6333]', '[8010,8000]',
               0.53, 0.1, 'seed', 'Q→E→W', 0)`,
    ).run();
    // Leaguepedia pro-presence row (synthetic "pro" position — never blended).
    db.prepare(
      `INSERT INTO champion_rates
         (champion_id, position, win_rate, pick_rate, ban_rate, sample_size,
          patch, source, confidence, cached_at)
       VALUES (86, 'pro', 0, 0.30, 0.25, 40, '14.1', 'leaguepedia', 'medium', 0)`,
    ).run();

    interface EnrichedRec extends RecLike {
      build_source: string;
      core_items: number[];
      keystone: number;
      skill_order?: string;
      pro_presence?: number;
      model_version: string;
      missing_signals?: string[];
      score_breakdown?: unknown[];
    }
    const recs = getRecommendations(
      engine,
      db,
      parsedTopSession(),
      PUUID,
    ) as EnrichedRec[];

    const garen = recs.find((r) => r.champion_id === 86);
    expect(garen).toBeDefined();
    expect(garen!.build_source).toBe("seed");
    expect(garen!.core_items).toEqual([3078, 3742, 3065, 3026]); // 4'e kırpılır
    expect(garen!.keystone).toBe(8010);
    expect(garen!.skill_order).toBe("Q→E→W");
    expect(garen!.pro_presence).toBeCloseTo(0.55, 5);
    expect(garen!.missing_signals ?? []).not.toContain("build");
    // DraftBrain upgrade her öneride çalışır (pack yok → local rules fallback).
    for (const rec of recs) {
      expect(rec.model_version).toBe("draft-brain-rules-v2");
      expect(rec.score_breakdown?.length ?? 0).toBeGreaterThan(0);
    }
    // Builds satırı olmayan aday dürüstçe "general" heuristiğe düşer.
    const other = recs.find((r) => r.champion_id !== 86);
    if (other) {
      expect(other.build_source).toBe("general");
      expect(other.missing_signals ?? []).toContain("build");
    }
  });

  it("accepts a RAW LCU session via core parse_session (actions branch)", () => {
    const db = seededDb();
    const raw = {
      localPlayerCellId: 0,
      myTeam: [
        { cellId: 0, championId: 0, championPickIntent: 0, assignedPosition: "top" },
      ],
      theirTeam: [
        { cellId: 5, championId: 122, championPickIntent: 0, assignedPosition: "top" },
      ],
      bans: { myTeamBans: [], theirTeamBans: [] },
      actions: [],
      timer: { phase: "BAN_PICK", adjustedTimeLeftInPhase: 27000 },
      gameConfig: { queueId: 420 },
    };
    const recs = getRecommendations(engine, db, raw, PUUID) as RecLike[];
    expect(recs.length).toBeGreaterThan(0);
  });

  it("throws the host-parity error for an invalid raw session", () => {
    expect(() => parseSessionArg(engine, { actions: [] })).toThrow(
      /Geçersiz session JSON/,
    );
  });
});

describe("analysis cluster (champ_select.rs parity)", () => {
  it("analyzes one champion and returns null for id 0", () => {
    const db = seededDb();
    const rec = getChampionAnalysis(
      engine,
      db,
      parsedTopSession(),
      86,
      PUUID,
    ) as RecLike | null;
    expect(rec?.champion_id).toBe(86);
    expect(getChampionAnalysis(engine, db, parsedTopSession(), 0, PUUID)).toBeNull();
  });

  it("produces ban suggestions, team comp and game plan from the session", () => {
    const db = seededDb();
    const bans = getBanSuggestions(engine, db, parsedTopSession(), PUUID);
    expect(Array.isArray(bans)).toBe(true);

    const board = getTeamComp(engine, db, parsedTopSession()) as {
      ally: object;
      enemy: object;
    };
    expect(board.ally).toBeTypeOf("object");
    expect(board.enemy).toBeTypeOf("object");

    const plan = getGamePlan(engine, db, parsedTopSession()) as {
      timeline: unknown[];
    };
    expect(Array.isArray(plan.timeline)).toBe(true);
  });

  it("player/postgame commands read and write the DB honestly", () => {
    const db = seededDb();

    // matches: 2 maç → player_stats + performance report.
    const insertMatch = db.prepare(
      `INSERT INTO matches (match_id, puuid, champion_id, position, win, kills,
        deaths, assists, duration_secs, queue_id, played_at, cs)
       VALUES (?, ?, ?, 'top', ?, 5, 2, 4, 1800, 420, ?, 160)`,
    );
    insertMatch.run("TR1_1", PUUID, 86, 1, 1000);
    insertMatch.run("TR1_2", PUUID, 86, 0, 2000);

    const stats = getPlayerStats(db, PUUID) as { champion_id: number; games: number }[];
    expect(stats[0]).toMatchObject({ champion_id: 86, games: 2 });

    const report = getPerformanceReport(engine, db, PUUID) as Record<string, unknown>;
    expect(report).toBeTypeOf("object");

    expect(getActiveSummonerPuuid(db)).toBe(PUUID);

    // mastery_snapshots: iki snapshot → gained > 0.
    const snap = db.prepare(
      `INSERT INTO mastery_snapshots (puuid, champion_id, mastery_points, mastery_level, snapshot_at)
       VALUES (?, 86, ?, 7, ?)`,
    );
    const now = Math.floor(Date.now() / 1000);
    snap.run(PUUID, 1000, now - 3600);
    snap.run(PUUID, 2500, now - 60);
    const progress = getMasteryProgress(db, PUUID, 7);
    expect(progress[0]).toMatchObject({
      champion_id: 86,
      champion_key: "Garen",
      points_gained: 1500,
    });

    const ack = submitRecommendationFeedback(db, {
      champion_id: 86,
      champion_key: "Garen",
      feedback: "helpful",
    });
    expect(ack.stored).toBe(true);
    expect(ack.synced).toBe(false);
    const fb = db
      .prepare("SELECT model_version FROM recommendation_feedback WHERE id = ?")
      .get(ack.feedback_id) as unknown as { model_version: string };
    expect(fb.model_version).toBe("draft-brain-rules-v2");
  });

  it("pool / feedback insight commands run end-to-end", () => {
    const db = seededDb();

    const plan = getChampionPoolPlan(engine, db, "top", PUUID) as Record<string, unknown>;
    expect(plan).toBeTypeOf("object");

    const suggestions = getPoolSuggestions(engine, db, "top", PUUID) as {
      champion_id: number;
    }[];
    expect(Array.isArray(suggestions)).toBe(true);
    expect(suggestions.every((s) => s.champion_id !== 86)).toBe(true);

    // seededDb 2 helpful feedback satırı ekler (synced_at NULL).
    const obs = getFeedbackObservability(engine, db) as {
      counters: { pending_sync: number };
    };
    expect(obs.counters.pending_sync).toBe(2);
  });

  it("simulates candidate picks and compares a fork (draft_simulator.rs parity)", () => {
    const db = seededDb();

    // 122 rakipte kilitli → unavailable; 0 ve dup atılır; yalnız 86 simüle edilir.
    const results = getDraftSimulation(engine, db, parsedTopSession(), [
      86, 0, 86, 122,
    ]) as { champion_id?: number }[];
    expect(results).toHaveLength(1);

    expect(getDraftSimulation(engine, db, parsedTopSession(), [])).toEqual([]);

    // Fork: geçerli çift → obje; aynı id → null; rakipte kilitli opsiyon → null.
    const fork = getDraftFork(engine, db, parsedTopSession(), 86, 54);
    expect(fork).toBeTypeOf("object");
    expect(fork).not.toBeNull();
    expect(getDraftFork(engine, db, parsedTopSession(), 86, 86)).toBeNull();
    expect(getDraftFork(engine, db, parsedTopSession(), 86, 122)).toBeNull();
  });

  it("serves the in-game plan and macro state from a live payload (overlay.rs parity)", async () => {
    const db = seededDb();
    const allgamedata = {
      activePlayer: { riotIdGameName: "Me" },
      allPlayers: [
        {
          riotIdGameName: "Me",
          championName: "Garen",
          rawChampionName: "game_character_displayname_Garen",
          position: "TOP",
          team: "ORDER",
          level: 9,
          scores: { kills: 2, deaths: 1, assists: 1, creepScore: 70 },
        },
        { riotIdGameName: "Foe", championName: "Darius", position: "TOP", team: "CHAOS" },
      ],
      gameData: { gameTime: 600.0 },
      events: { Events: [{ EventName: "DragonKill", EventTime: 480.0 }] },
    };
    const live = async () => allgamedata;
    const offline = async () => null;

    const plan = (await getIngamePlan(engine, db, live)) as {
      champion_key: string;
      opponent_key?: string;
      cs_per_min?: number;
    };
    expect(plan.champion_key).toBe("Garen");
    expect(plan.opponent_key).toBe("Darius");
    expect(plan.cs_per_min).toBe(7); // 70 CS / 10 dk

    const macro = (await getMacroState(engine, live)) as {
      live: boolean;
      state: { objectives: { objective: string; next_spawn_secs: number }[] };
    };
    expect(macro.live).toBe(true);
    const dragon = macro.state.objectives.find((o) => o.objective === "dragon");
    expect(dragon?.next_spawn_secs).toBe(780); // 480 + 300

    // Canlı oyun yok → sessiz null / {live:false} (hata DEĞİL).
    expect(await getIngamePlan(engine, db, offline)).toBeNull();
    expect(await getMacroState(engine, offline)).toEqual({ live: false, state: null });
  });

  it("serves KB archetypes and champion detail without a DB", () => {
    const archetypes = getChampionArchetypes(engine) as {
      champion_id: number;
      archetype: string;
    }[];
    expect(archetypes.length).toBeGreaterThan(100);

    const detail = getChampionDetail(engine, 86) as { champion_key: string };
    expect(detail.champion_key).toBe("Garen");
    expect(getChampionDetail(engine, 999_999)).toBeNull();
  });
});
