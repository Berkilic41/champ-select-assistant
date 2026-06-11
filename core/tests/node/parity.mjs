// WASM ↔ native parity smoke for the csa-core JSON API.
//
// Run AFTER `wasm-pack build --target nodejs` (from core/):
//   node tests/node/parity.mjs
//
// Mirrors the native unit tests in src/json_api.rs (same fixture, same
// assertions), proving the WASM artifact exposes the same surface and produces
// structurally identical output in a JS host.

import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(import.meta.url);
const core = require(join(here, "..", "..", "pkg", "csa_core.js"));

let failures = 0;
function check(name, fn) {
  try {
    fn();
    console.log(`ok   ${name}`);
  } catch (err) {
    failures++;
    console.error(`FAIL ${name}: ${err.message}`);
  }
}

// ── smoke ─────────────────────────────────────────────────────────────────────
check("core_smoke", () => {
  const s = core.core_smoke();
  if (s !== "csa-core wasm alive") throw new Error(`unexpected smoke: ${s}`);
});

// ── recommendations_json (shared fixture with src/json_api.rs tests) ─────────
const fixture = readFileSync(
  join(here, "..", "fixtures", "json_api_recommendations_input.json"),
  "utf8",
);

check("recommendations_json: ranked output", () => {
  const out = JSON.parse(core.recommendations_json(fixture));
  if (!Array.isArray(out)) throw new Error("output must be an array");
  if (out.length === 0) throw new Error("fixture should yield recommendations");
  for (const rec of out) {
    if (typeof rec.champion_id !== "number") throw new Error("missing champion_id");
    if (typeof rec.total_score !== "number") throw new Error("missing total_score");
  }
  const scores = out.map((r) => r.total_score);
  const sorted = [...scores].sort((a, b) => b - a);
  if (JSON.stringify(scores) !== JSON.stringify(sorted)) {
    throw new Error("recommendations must be sorted by total_score desc");
  }
});

check("recommendations_json: malformed input throws", () => {
  let threw = false;
  try {
    core.recommendations_json("{not json");
  } catch (err) {
    threw = true;
    if (!String(err).includes("invalid recommendations input")) {
      throw new Error(`unexpected error: ${err}`);
    }
  }
  if (!threw) throw new Error("expected a throw");
});

// ── draft_verdict_json (mirrors the native draft_verdict_roundtrip test) ─────
check("draft_verdict_json: roundtrip", () => {
  const input = {
    plan: {
      team_identity: "Teamfight / deathball",
      identity_note: "5v5 kazanır",
      timeline: [
        { label: "Erken (0-14dk)", stance: "even", advice: "", strength: 0.5 },
        { label: "Orta (14-25dk)", stance: "advantage", advice: "", strength: 0.6 },
        { label: "Geç (25dk+)", stance: "advantage", advice: "", strength: 0.65 },
      ],
      win_conditions: ["Objektif etrafında 5v5 al"],
      objectives: ["dragon"],
      enemy_threat: "Split push",
      alt_plan: null,
      partial: false,
    },
    ally: {
      tanks: 1, fighters: 1, mages: 1, marksmen: 1, assassins: 0,
      supports: 1, ap_share: 0.45, ad_share: 0.55,
      has_engage: true, has_frontline: true, has_hard_cc: true,
      has_peel: true, gaps: [], summary: "dengeli",
    },
    enemy: {
      tanks: 0, fighters: 2, mages: 1, marksmen: 1, assassins: 1,
      supports: 0, ap_share: 0.35, ad_share: 0.65,
      has_engage: false, has_frontline: false, has_hard_cc: false,
      has_peel: false, gaps: ["Frontline yok"], summary: "kırılgan",
    },
    lane_matchup: 0.55,
  };
  const verdict = JSON.parse(core.draft_verdict_json(JSON.stringify(input)));
  if (typeof verdict !== "object" || verdict === null) {
    throw new Error("verdict must be an object");
  }
});

// ── ban_suggestions_json ──────────────────────────────────────────────────────
check("ban_suggestions_json: returns an array", () => {
  const input = JSON.parse(fixture);
  const out = JSON.parse(
    core.ban_suggestions_json(
      JSON.stringify({
        session: input.session,
        all_champions: input.all_champions,
        meta_rates: input.meta_rates,
        my_pool: input.mastery,
        role_map: input.role_map,
      }),
    ),
  );
  if (!Array.isArray(out)) throw new Error("output must be an array");
});

// ── pool_coach_json / performance_report_json / macro_state_json ─────────────
check("pool_coach_json: returns a plan object", () => {
  const out = JSON.parse(
    core.pool_coach_json(
      JSON.stringify({
        role: "top",
        pool: [{
          champion_id: 86, champion_key: "Garen", archetype: "juggernaut",
          blind_safety: 0.8, execution_difficulty: 1, power_late: 0.6,
          engage: false, peel: false, comfort: 0.9, games: 40,
          meta_strength: 0.55,
        }],
        candidates: [],
      }),
    ),
  );
  if (typeof out !== "object" || out === null) throw new Error("plan must be an object");
});

check("performance_report_json: returns a report object", () => {
  const out = JSON.parse(
    core.performance_report_json(
      JSON.stringify([{
        champion_id: 86, champion_key: "Garen", position: "top",
        win: true, kills: 7, deaths: 3, assists: 5,
        played_at: 1700000000, duration_secs: 1800, cs: 180,
      }]),
    ),
  );
  if (typeof out !== "object" || out === null) throw new Error("report must be an object");
});

check("macro_state_json: returns a state object", () => {
  const out = JSON.parse(
    core.macro_state_json(
      JSON.stringify({
        game_time_secs: 600,
        events: [{ objective: "dragon", killed_at_secs: 480 }],
      }),
    ),
  );
  if (typeof out !== "object" || out === null) throw new Error("state must be an object");
});

// ── parse_session_json (mirrors parse_session_handles_raw_lcu_payload) ───────
check("parse_session_json: parses a raw LCU session", () => {
  const raw = {
    localPlayerCellId: 2,
    myTeam: [
      { cellId: 2, championId: 99, championPickIntent: 0, assignedPosition: "middle" },
    ],
    theirTeam: [],
    bans: { myTeamBans: [], theirTeamBans: [] },
    actions: [],
    timer: { phase: "BAN_PICK", adjustedTimeLeftInPhase: 27000 },
    gameConfig: { queueId: 420 },
  };
  const state = JSON.parse(core.parse_session_json(JSON.stringify(raw)));
  if (state.my_cell_id !== 2) throw new Error("my_cell_id mismatch");
  if (state.queue_id !== 420) throw new Error("queue_id mismatch");
  if (state.local_player.champion_id !== 99) throw new Error("local_player mismatch");
});

check("parse_session_json: invalid payload throws host-parity message", () => {
  let threw = false;
  try {
    core.parse_session_json("{}");
  } catch (err) {
    threw = true;
    if (!String(err).includes("Geçersiz session JSON")) {
      throw new Error(`unexpected error: ${err}`);
    }
  }
  if (!threw) throw new Error("expected a throw");
});

// ── blended_meta_rates_json (mirrors the native blend + role-share test) ──────
check("blended_meta_rates_json: blends and filters off-role noise", () => {
  const out = JSON.parse(
    core.blended_meta_rates_json(
      JSON.stringify({
        rows: [
          { champion_id: 51, position: "bottom", win_rate: 0.52, ban_rate: 0.08, sample_size: 5000 },
          { champion_id: 51, position: "bottom", win_rate: 0.53, ban_rate: 0.0, sample_size: 9000 },
          { champion_id: 2, position: "bottom", win_rate: 0.55, ban_rate: 0.0, sample_size: 2621 },
        ],
        position_samples: [
          { champion_id: 51, position: "bottom", sample_size: 14000 },
          { champion_id: 2, position: "top", sample_size: 199646 },
          { champion_id: 2, position: "bottom", sample_size: 2621 },
        ],
      }),
    ),
  );
  if (!Array.isArray(out) || out.length !== 1) {
    throw new Error("off-role row must be filtered, one entry expected");
  }
  if (out[0].champion_id !== 51 || out[0].sample_size !== 14000) {
    throw new Error("agreeing sources must sum evidence");
  }
});

// ── feedback_signals_json (mirrors feedback_signals_aggregate_raw_rows) ───────
check("feedback_signals_json: aggregates raw verdict rows", () => {
  const out = JSON.parse(
    core.feedback_signals_json(
      JSON.stringify([
        { champion_id: 86, verdict: "helpful" },
        { champion_id: 86, verdict: "helpful" },
        { champion_id: 86, verdict: "not_helpful" },
        { champion_id: 99, verdict: "skipped" },
      ]),
    ),
  );
  if (!Array.isArray(out) || out.length !== 1) {
    throw new Error("skipped-only champions must emit no signal");
  }
  if (out[0].champion_id !== 86 || out[0].positive !== 2 || out[0].negative !== 1) {
    throw new Error("aggregate counts mismatch");
  }
});

// ── aram_weights_json (single source of truth: ScoringWeights::aram) ─────────
check("aram_weights_json: returns the core brawl preset", () => {
  const w = JSON.parse(core.aram_weights_json());
  if (w.matchup !== 0 || w.role_fit !== 0) {
    throw new Error("brawl preset must zero matchup/role_fit");
  }
  if (typeof w.comfort !== "number" || w.comfort <= w.meta) {
    throw new Error("brawl preset must uplift comfort");
  }
});

if (failures > 0) {
  console.error(`\n${failures} parity check(s) FAILED`);
  process.exit(1);
}
console.log("\nAll WASM parity checks passed.");
