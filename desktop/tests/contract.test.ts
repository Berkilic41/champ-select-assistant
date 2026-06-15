// V4 — sözleşme sağlamlaştırması. Two cheap drift shields:
//
// 1. DraftBrainQualityReport: the generated TS type (hand-maintained since the
//    Rust struct was removed) and the host's emitted interface (quality.ts) must
//    stay identical. The bidirectional type assignments below fail desktop
//    `typecheck` (tsconfig includes tests/) on any field add/remove/rename/retype;
//    the runtime test pins the emitted key-set + the number-not-bigint contract.
//
// 2. draft-brain rules version: the same string lives in core (draft_brain.rs, the
//    source of truth) and is mirrored in two host modules (quality.ts, outcomes.ts).
//    If they drift, recorded labels / quality notes silently disagree with the
//    engine. This reads the core literal and asserts both host copies match it.

import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { afterEach, describe, expect, it } from "vitest";

import {
  getDraftBrainQualityReport,
  type DraftBrainQualityReport as HostReport,
} from "../src/main/commands/quality";
import { openDatabase, runMigrations } from "../src/main/db";
import type { DraftBrainQualityReport as GeneratedReport } from "../../src/types/generated/DraftBrainQualityReport";

// Compile-time drift shield (both directions): structural identity of the two
// interfaces. These only need to COMPILE — a field drift breaks `typecheck`.
const hostSatisfiesGenerated: GeneratedReport = {} as HostReport;
const generatedSatisfiesHost: HostReport = {} as GeneratedReport;

const MIGRATIONS_DIR = join(__dirname, "..", "resources", "migrations");

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
  dir = mkdtempSync(join(tmpdir(), "csa-contract-"));
  const db = openDatabase(join(dir, "app.db"));
  runMigrations(db, MIGRATIONS_DIR);
  openDb = db;
  return db;
}

describe("DraftBrainQualityReport contract", () => {
  it("keeps the host interface structurally identical to the generated type", () => {
    // The module-level casts above carry the real (compile-time) assertion.
    expect(hostSatisfiesGenerated).toBeDefined();
    expect(generatedSatisfiesHost).toBeDefined();
  });

  it("emits exactly the generated field set with number (not bigint) counts", () => {
    const report = getDraftBrainQualityReport(migratedDb());
    expect(Object.keys(report).sort()).toEqual([
      "cloud_configured",
      "data_pack_confidence",
      "data_pack_fresh",
      "data_pack_generated_at",
      "data_pack_version",
      "feedback_total",
      "feedback_unsynced",
      "local_rules_version",
      "model_pack_version",
      "notes",
    ]);
    // feedback_* serialize as plain JS numbers over IPC (NOT bigint).
    expect(typeof report.feedback_total).toBe("number");
    expect(typeof report.feedback_unsynced).toBe("number");
    expect(Array.isArray(report.notes)).toBe(true);
    expect(report.model_pack_version).toBeNull(); // fresh DB → no learned pack
  });
});

describe("draft-brain rules version cross-language sync", () => {
  it("host quality.ts + outcomes.ts mirror the core draft_brain.rs constant", () => {
    const coreSrc = readFileSync(
      resolve(__dirname, "..", "..", "core", "src", "draft_brain.rs"),
      "utf8",
    );
    const coreVersion = coreSrc.match(
      /DRAFT_BRAIN_RULES_VERSION:\s*&str\s*=\s*"([^"]+)"/,
    )?.[1];
    expect(coreVersion, "core'da DRAFT_BRAIN_RULES_VERSION literal'i bulunamadı").toBeTruthy();

    // host quality.ts — runtime value through the real report.
    expect(getDraftBrainQualityReport(migratedDb()).local_rules_version).toBe(coreVersion);

    // host outcomes.ts — source literal (written into every label's model_version).
    const outcomesSrc = readFileSync(
      resolve(__dirname, "..", "src", "main", "commands", "outcomes.ts"),
      "utf8",
    );
    expect(outcomesSrc).toContain(`"${coreVersion}"`);
  });
});
