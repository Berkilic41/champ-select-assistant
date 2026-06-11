import { readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import { Engine } from "../src/main/engine";

// Real WASM, real KB, shared synthetic fixture — the same one the Rust unit
// tests and core/tests/node/parity.mjs assert against.
const FIXTURE_PATH = join(
  __dirname,
  "..",
  "..",
  "core",
  "tests",
  "fixtures",
  "json_api_recommendations_input.json",
);

describe("Engine (core.wasm bridge)", () => {
  it("loads the WASM module and passes the smoke check", () => {
    expect(() => Engine.load()).not.toThrow();
  });

  it("computes ranked recommendations from the shared fixture", () => {
    const engine = Engine.load();
    const input = JSON.parse(readFileSync(FIXTURE_PATH, "utf8"));
    const recs = engine.recommendations<
      { champion_id: number; total_score: number }[]
    >(input);
    expect(recs.length).toBeGreaterThan(0);
    const scores = recs.map((r) => r.total_score);
    expect(scores).toEqual([...scores].sort((a, b) => b - a));
  });

  it("surfaces engine errors as exceptions", () => {
    const engine = Engine.load();
    expect(() => engine.draftVerdict({})).toThrow(/invalid draft verdict input/);
  });
});
