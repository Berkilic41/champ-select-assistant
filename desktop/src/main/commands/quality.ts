// DraftBrain quality report — port of src-tauri/src/commands/draft_brain.rs
// (`get_draft_brain_quality_report` + its helpers). Local DB counters + pack
// metadata + honest Turkish notes; never hits the network. Note strings are
// copied VERBATIM from the Rust host so the UI renders identically.

import type { DatabaseSync } from "node:sqlite";

import { runtimeEnv } from "../riot/client";

const DEFAULT_PACK_TTL_SECS = 24 * 60 * 60;

// core draft_brain.rs:9 sabitiyle EL İLE eşli (player.ts model_version gibi) —
// sürüm core'da değişirse burayı da güncelle.
const LOCAL_RULES_VERSION = "draft-brain-rules-v2";

export interface DraftBrainQualityReport {
  feedback_total: number;
  feedback_unsynced: number;
  model_pack_version: string | null;
  data_pack_version: string | null;
  data_pack_confidence: string | null;
  data_pack_generated_at: number | null;
  data_pack_fresh: boolean | null;
  local_rules_version: string;
  cloud_configured: boolean;
  notes: string[];
}

function count(db: DatabaseSync, sql: string): number {
  const row = db.prepare(sql).get() as unknown as Record<string, unknown>;
  return Number(Object.values(row ?? {})[0] ?? 0);
}

function packRow(
  db: DatabaseSync,
  kind: string,
): { version: string; payload_json: string } | undefined {
  return db
    .prepare("SELECT version, payload_json FROM draft_brain_packs WHERE kind = ?")
    .get(kind) as unknown as { version: string; payload_json: string } | undefined;
}

function packMetadata(payload: string | undefined): {
  confidence: string | null;
  generated_at: number | null;
} {
  if (!payload) return { confidence: null, generated_at: null };
  let parsed: Record<string, unknown> = {};
  try {
    parsed = JSON.parse(payload) as Record<string, unknown>;
  } catch {
    /* bozuk payload → metadata yok */
  }
  const confidence =
    typeof parsed.confidence === "string" && parsed.confidence.trim()
      ? parsed.confidence
      : null;
  const generatedAt =
    typeof parsed.generated_at === "number" && parsed.generated_at >= 0
      ? Math.floor(parsed.generated_at)
      : null;
  return { confidence, generated_at: generatedAt };
}

export function getDraftBrainQualityReport(
  db: DatabaseSync,
  env: Record<string, string | undefined> = runtimeEnv(),
  nowSecs: number = Math.floor(Date.now() / 1000),
): DraftBrainQualityReport {
  const cloudConfigured = Boolean(env.DRAFT_BRAIN_API_BASE?.trim());

  const feedbackTotal = count(db, "SELECT COUNT(*) FROM recommendation_feedback");
  const feedbackUnsynced = count(
    db,
    "SELECT COUNT(*) FROM recommendation_feedback WHERE synced_at IS NULL",
  );

  const modelPack = packRow(db, "model_pack");
  const dataPack = packRow(db, "data_pack");
  const meta = packMetadata(dataPack?.payload_json);
  const dataPackFresh =
    meta.generated_at === null
      ? null
      : nowSecs - meta.generated_at <= DEFAULT_PACK_TTL_SECS;

  const notes: string[] = [];
  if (!cloudConfigured) {
    notes.push("DRAFT_BRAIN_API_BASE yok; local rules/data fallback aktif");
  }
  if (!modelPack) {
    notes.push(
      "Model pack cache boş; runtime local rules kullanır, sync_model_pack önerilir",
    );
  }
  if (!dataPack) {
    notes.push(
      "Data pack cache boş; runtime local seed kullanır, sync_data_pack önerilir",
    );
  }
  if (dataPack && meta.confidence === null) {
    notes.push(
      "Data pack confidence yok; eski backend veya eksik pack metadata olabilir",
    );
  }
  if (dataPackFresh === false) {
    notes.push("Data pack generated_at 24 saatten eski; sync_data_pack önerilir");
  }
  if (meta.confidence === "low") {
    notes.push("Data pack confidence low; local/seed fallback sinyali baskın");
  }
  if (feedbackUnsynced > 0) {
    notes.push(`${feedbackUnsynced} feedback cloud sync bekliyor`);
  }

  return {
    feedback_total: feedbackTotal,
    feedback_unsynced: feedbackUnsynced,
    model_pack_version: modelPack?.version ?? null,
    data_pack_version: dataPack?.version ?? null,
    data_pack_confidence: meta.confidence,
    data_pack_generated_at: meta.generated_at,
    data_pack_fresh: dataPackFresh,
    local_rules_version: LOCAL_RULES_VERSION,
    cloud_configured: cloudConfigured,
    notes,
  };
}
