// FAZ 4 / Sprint 1 — getCoachNarrative host testleri. GERÇEK wasm core ile
// (engine.coachNarrative); DB gerekmez. Narrator mantığı core'da test edili;
// burada host yolu: rec + FAZ3 sinyal mapping + dış aday audit-gate/fallback.

import { beforeAll, describe, expect, it } from "vitest";

import { getCoachNarrative } from "../src/main/commands/coach-narrative";
import { Engine } from "../src/main/engine";

let engine: Engine;
beforeAll(() => {
  engine = Engine.load();
});

/** Core Recommendation'ın deserialize için gereken minimal + narrator alanları. */
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
        {
          ally_champion_id: 59,
          ally_champion_key: "Jarvan",
          combo_text: "wombo",
          ability_ref: "x",
          combo_type: "wombo",
        },
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

describe("getCoachNarrative (FAZ 4 / Sprint 1)", () => {
  it("weaves grounded facts + FAZ3 signals into a deterministic note", () => {
    const n = getCoachNarrative(engine, {
      recommendation: baseRec(),
      win_prob: { probability: 0.58, sample_size: 40 },
      combo_history: { games: 5, wins: 3 },
    });
    expect(n.source).toBe("deterministic");
    expect(n.external_rejected).toBe(false);
    expect(n.text).toContain("Lee Sin");
    expect(n.text).toContain("Elise"); // lane grounding
    expect(n.text).toContain("Jarvan"); // combo grounding
    expect(n.text).toContain("5 maç"); // combo history (3C)
    expect(n.text).toContain("~%58"); // win-prob (3A), probability*100
    expect(n.text).toContain("Goredrinker"); // build grounding
  });

  it("accepts a clean external candidate (audit pass)", () => {
    const candidate = "Lee Sin ile erken tempo kurup Jarvan'la pencere ararsan iyi olur.";
    const n = getCoachNarrative(engine, { recommendation: baseRec(), candidate });
    expect(n.source).toBe("external");
    expect(n.external_rejected).toBe(false);
    expect(n.text).toBe(candidate);
  });

  it("rejects an over-promising candidate and falls back to deterministic", () => {
    const candidate = "Lee Sin ile bu maçı kesinlikle kazanırsın, garanti.";
    const n = getCoachNarrative(engine, { recommendation: baseRec(), candidate });
    expect(n.source).toBe("deterministic");
    expect(n.external_rejected).toBe(true);
    expect(n.text).toContain("Lee Sin");
  });

  it("omits FAZ3 lines when signals are absent", () => {
    const n = getCoachNarrative(engine, { recommendation: baseRec() });
    expect(n.source).toBe("deterministic");
    expect(n.text).not.toContain("~%"); // win-prob yok
    expect(n.text).not.toContain("geçmişin"); // combo history yok
    expect(n.text).toContain("Lee Sin");
  });
});
