// FAZ 4 / LLM provider — llm-narrator birim testleri (prompt + fetch edge cases).
// DB/core gerekmez; fetch mock'lanır. Hata yolları → null (deterministik fallback).

import { describe, expect, it } from "vitest";

import {
  buildCoachUserPrompt,
  fetchLlmCandidate,
  type FetchFn,
} from "../src/main/commands/llm-narrator";

const rec = {
  champion_name: "Lee Sin",
  headline: "matchup penceren iyi",
  lane_opponent_name: "Elise",
  core_item_name: "Goredrinker",
  risk_summary: "erken ölürsen tempo düşer",
  draft_plan: { combo_with: [{ ally_champion_key: "Jarvan" }] },
};

describe("buildCoachUserPrompt", () => {
  it("includes the grounded facts + FAZ3 signals", () => {
    const prompt = buildCoachUserPrompt(rec, {
      win_prob: { probability: 0.58, sample_size: 40 },
      combo_history: { games: 5, wins: 3 },
    });
    expect(prompt).toContain("Lee Sin");
    expect(prompt).toContain("Elise");
    expect(prompt).toContain("Jarvan");
    expect(prompt).toContain("Goredrinker");
    expect(prompt).toContain("~%58");
    expect(prompt).toContain("5 maç");
  });

  it("omits absent facts", () => {
    const prompt = buildCoachUserPrompt({ champion_name: "Yasuo" }, {});
    expect(prompt).toContain("Yasuo");
    expect(prompt).not.toContain("Lane rakibi");
    expect(prompt).not.toContain("~%");
  });
});

describe("fetchLlmCandidate", () => {
  const ok = (content: unknown): FetchFn => async () => ({
    ok: true,
    json: async () => ({ choices: [{ message: { content } }] }),
  });

  it("returns the assistant content on a clean response", async () => {
    const out = await fetchLlmCandidate("http://x/v1/chat", "m", rec, {}, ok("Lee Sin notu."));
    expect(out).toBe("Lee Sin notu.");
  });

  it("returns null for an empty endpoint (disabled)", async () => {
    const out = await fetchLlmCandidate("  ", "m", rec, {}, ok("x"));
    expect(out).toBeNull();
  });

  it("returns null on a non-ok response", async () => {
    const notOk: FetchFn = async () => ({ ok: false, json: async () => ({}) });
    expect(await fetchLlmCandidate("http://x", "m", rec, {}, notOk)).toBeNull();
  });

  it("returns null on empty/blank content", async () => {
    expect(await fetchLlmCandidate("http://x", "m", rec, {}, ok("   "))).toBeNull();
    expect(await fetchLlmCandidate("http://x", "m", rec, {}, ok(123))).toBeNull();
  });

  it("returns null when fetch throws (offline / refused)", async () => {
    const boom: FetchFn = async () => {
      throw new Error("ECONNREFUSED");
    };
    expect(await fetchLlmCandidate("http://x", "m", rec, {}, boom)).toBeNull();
  });
});
