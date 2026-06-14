// Feedback flush — port of src-tauri/src/commands/feedback_flush.rs
// (`sync_recommendation_feedback`). The PURE policy (backoff "due", the privacy
// hash gate, the backend-dedup idempotency key, and result→state resolution)
// runs in core via the WASM JSON API; this file only does SQL + the POST loop.
//
// Correctness guarantees (Rust parity):
//   * Network failure never corrupts the queue — a row is marked synced ONLY on
//     success; failure just bumps retry bookkeeping.
//   * PII-free — rows without a (≥16 char) session hash are skipped, never sent.
//   * Idempotent — every POST carries the core-computed dedup key.

import type { DatabaseSync } from "node:sqlite";

import type { Engine } from "../engine";
import { runtimeEnv } from "../riot/client";
import { getSettings } from "./settings";

export interface FeedbackFlushSummary {
  offline: boolean;
  attempted: number;
  synced: number;
  failed: number;
  skipped_no_hash: number;
}

interface QueueRow {
  id: number;
  champion_id: number;
  champion_key: string;
  feedback: string;
  session_hash: string | null;
  model_version: string;
  score: number;
  payload_json: string;
  created_at: number;
  retry_count: number;
  next_retry_at: number | null;
}

interface FlushRowPlan {
  id: number;
  action: "send" | "wait" | "skip_no_hash";
  user_hash?: string;
  idempotency_key?: string;
}

interface FlushStateOut {
  synced_at: number | null;
  retry_count: number;
  last_error: string | null;
  next_retry_at: number | null;
}

/** Test seam: POST one feedback body; resolve {ok} or {ok:false, error}. */
export type PostFeedback = (
  url: string,
  token: string | undefined,
  body: unknown,
) => Promise<{ ok: boolean; error?: string }>;

const realPost: PostFeedback = async (url, token, body) => {
  try {
    const res = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
      },
      body: JSON.stringify(body),
    });
    return res.ok ? { ok: true } : { ok: false, error: `HTTP ${res.status}` };
  } catch (err) {
    return { ok: false, error: (err as Error).message };
  }
};

/** Vadesi gelen, sync'lenmemiş feedback satırlarını buluta gönder. */
export async function syncRecommendationFeedback(
  engine: Engine,
  db: DatabaseSync,
  env: Record<string, string | undefined> = runtimeEnv(),
  post: PostFeedback = realPost,
): Promise<FeedbackFlushSummary> {
  // Privacy gate (FAZ 2): anonymized feedback only leaves the device with the
  // user's explicit opt-in (Settings → Gizlilik, OFF by default). Without consent
  // the feedback backend is treated as unreachable — queued rows stay local and
  // are never uploaded. This is the authoritative check: it runs before any
  // network work regardless of caller, so consent can never be bypassed.
  if (!getSettings(db).share_anonymous_feedback) {
    return { offline: true, attempted: 0, synced: 0, failed: 0, skipped_no_hash: 0 };
  }
  const base = env.DRAFT_BRAIN_API_BASE?.trim();
  if (!base) {
    return { offline: true, attempted: 0, synced: 0, failed: 0, skipped_no_hash: 0 };
  }
  const token = env.DRAFT_BRAIN_API_TOKEN?.trim() || undefined;
  const now = Math.floor(Date.now() / 1000);
  const url = `${base.replace(/\/+$/, "")}/v1/recommendation-feedback`;

  const rows = db
    .prepare(
      `SELECT rowid AS id, champion_id, champion_key, feedback, session_hash,
              model_version, score, payload_json, created_at, retry_count, next_retry_at
       FROM recommendation_feedback WHERE synced_at IS NULL`,
    )
    .all() as unknown as QueueRow[];

  // Karar core'da: hangi satır gönderilir / bekler / atlanır + dedup anahtarı.
  const plans = engine.feedbackFlushPlan<FlushRowPlan[]>({
    rows: rows.map((r) => ({
      id: r.id,
      champion_id: r.champion_id,
      feedback: r.feedback,
      session_hash: r.session_hash,
      created_at: r.created_at,
      retry_count: r.retry_count ?? 0,
      next_retry_at: r.next_retry_at,
    })),
    now_secs: now,
  });
  const planById = new Map(plans.map((p) => [p.id, p]));

  const summary: FeedbackFlushSummary = {
    offline: false,
    attempted: 0,
    synced: 0,
    failed: 0,
    skipped_no_hash: 0,
  };
  const update = db.prepare(
    `UPDATE recommendation_feedback
     SET synced_at = ?, retry_count = ?, last_error = ?, next_retry_at = ?
     WHERE rowid = ?`,
  );

  for (const row of rows) {
    const plan = planById.get(row.id);
    if (!plan || plan.action === "wait") continue;
    if (plan.action === "skip_no_hash") {
      summary.skipped_no_hash += 1;
      continue;
    }
    summary.attempted += 1;

    let payload: unknown = {};
    try {
      payload = JSON.parse(row.payload_json);
    } catch {
      payload = {};
    }
    const result = await post(url, token, {
      user_hash: plan.user_hash,
      champion_id: row.champion_id,
      champion_key: row.champion_key,
      feedback: row.feedback,
      model_version: row.model_version,
      score: row.score,
      payload,
      idempotency_key: plan.idempotency_key,
    });
    if (result.ok) summary.synced += 1;
    else summary.failed += 1;

    const st = engine.feedbackFlushResolve<FlushStateOut>({
      retry_count: row.retry_count ?? 0,
      next_retry_at: row.next_retry_at,
      ok: result.ok,
      error: result.error ?? null,
      now_secs: now,
    });
    update.run(st.synced_at, st.retry_count, st.last_error, st.next_retry_at, row.id);
  }

  return summary;
}
