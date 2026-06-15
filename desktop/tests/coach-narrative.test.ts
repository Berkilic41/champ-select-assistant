// FAZ 4 — getCoachNarrative host testleri. GERÇEK wasm core (engine.coachNarrative)
// + migrated DB (settings). Narrator mantığı core'da test edili; burada host yolu:
// rec→core, FAZ3 mapping, dış aday audit-gate/fallback VE opsiyonel LLM provider
// (yapılandırılmış endpoint, mock fetch ile).

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { afterEach, beforeAll, describe, expect, it } from "vitest";

import { getCoachNarrative } from "../src/main/commands/coach-narrative";
import type { FetchFn } from "../src/main/commands/llm-narrator";
import { DEFAULT_SETTINGS, saveSettings } from "../src/main/commands/settings";
import { openDatabase, runMigrations } from "../src/main/db";
import { Engine } from "../src/main/engine";

const MIGRATIONS_DIR = join(__dirname, "..", "resources", "migrations");

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
  dir = mkdtempSync(join(tmpdir(), "csa-coach-"));
  const db = openDatabase(join(dir, "app.db"));
  runMigrations(db, MIGRATIONS_DIR);
  openDb = db;
  return db;
}

/** Yapılandırılmış endpoint'i döndüren mock fetch. */
function mockFetch(content: string): FetchFn {
  return async () => ({
    ok: true,
    json: async () => ({ choices: [{ message: { content } }] }),
  });
}

function baseRec(): Record<string, unknown> {
  return {
    champion_id: 64,
    champion_key: "LeeSin",
    champion_name: "Lee Sin",
    total_score: 0.8,
    comfort_score: 0.7,
    matchup_score: 0.78,
    team_counter_score: 0.6,
    synergy_score: 0.65,
    meta_score: 0.7,
    role_fit_score: 0.9,
    risk_score: 0.2,
    reason: "test",
    core_items: [],
    situational_items: [],
    primary_rune_tree: 8000,
    keystone: 8100,
    tier: "a",
    confidence: "high",
    games_on_champ: 30,
    wins_on_champ: 18,
    enemy_team_summary: "AP",
    lane_opponent_name: "Elise",
    core_item_name: "Goredrinker",
    risk_summary: "erken ölürsen tempo düşer",
    draft_plan: {
      combo_with: [
        { ally_champion_id: 59, ally_champion_key: "Jarvan", combo_text: "wombo", ability_ref: "x", combo_type: "wombo" },
      ],
      win_condition: "x",
      team_role: "x",
      damage_profile: "x",
      blind_pick_safety: 0.5,
      execution_difficulty: 3,
      threats: [],
      fills_team_need: [],
    },
  };
}

describe("getCoachNarrative (FAZ 4)", () => {
  it("weaves grounded facts + FAZ3 signals into a deterministic note (no LLM)", async () => {
    const db = migratedDb();
    const n = await getCoachNarrative(engine, db, {
      recommendation: baseRec(),
      win_prob: { probability: 0.58, sample_size: 40 },
      combo_history: { games: 5, wins: 3 },
    });
    expect(n.source).toBe("deterministic");
    expect(n.text).toContain("Lee Sin");
    expect(n.text).toContain("Elise");
    expect(n.text).toContain("Jarvan");
    expect(n.text).toContain("5 maç");
    expect(n.text).toContain("~%58");
  });

  it("accepts a clean explicit candidate (audit pass)", async () => {
    const db = migratedDb();
    const candidate = "Lee Sin ile erken tempo kurup Jarvan'la pencere ararsan iyi olur.";
    const n = await getCoachNarrative(engine, db, { recommendation: baseRec(), candidate });
    expect(n.source).toBe("external");
    expect(n.text).toBe(candidate);
  });

  it("rejects an over-promising explicit candidate → deterministic fallback", async () => {
    const db = migratedDb();
    const candidate = "Lee Sin ile bu maçı kesinlikle kazanırsın, garanti.";
    const n = await getCoachNarrative(engine, db, { recommendation: baseRec(), candidate });
    expect(n.source).toBe("deterministic");
    expect(n.external_rejected).toBe(true);
  });

  it("uses an LLM endpoint candidate when configured (audit pass)", async () => {
    const db = migratedDb();
    saveSettings(db, {
      ...DEFAULT_SETTINGS,
      coach_llm_endpoint: "http://localhost:11434/v1/chat/completions",
      coach_llm_model: "llama3",
    });
    const llmText = "Lee Sin ile lane'i kontrollü oyna; Jarvan'la pencere ararsan tempo bulursun.";
    const n = await getCoachNarrative(
      engine,
      db,
      { recommendation: baseRec() },
      mockFetch(llmText),
    );
    expect(n.source).toBe("external");
    expect(n.text).toBe(llmText);
  });

  it("rejects an over-promising LLM candidate → deterministic fallback", async () => {
    const db = migratedDb();
    saveSettings(db, {
      ...DEFAULT_SETTINGS,
      coach_llm_endpoint: "http://localhost:11434/v1/chat/completions",
    });
    const n = await getCoachNarrative(
      engine,
      db,
      { recommendation: baseRec() },
      mockFetch("Lee Sin kesinlikle kazandırır, garanti zafer."),
    );
    expect(n.source).toBe("deterministic");
    expect(n.external_rejected).toBe(true);
  });

  it("falls back to deterministic when the LLM endpoint errors", async () => {
    const db = migratedDb();
    saveSettings(db, {
      ...DEFAULT_SETTINGS,
      coach_llm_endpoint: "http://localhost:11434/v1/chat/completions",
    });
    const throwing: FetchFn = async () => {
      throw new Error("connection refused");
    };
    const n = await getCoachNarrative(engine, db, { recommendation: baseRec() }, throwing);
    expect(n.source).toBe("deterministic");
    expect(n.external_rejected).toBe(false); // aday hiç üretilmedi (LLM null)
    expect(n.text).toContain("Lee Sin");
  });
});
