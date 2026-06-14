// Originally ts-rs generated; the Rust source struct was removed in the Electron
// migration, so this is now the hand-maintained shared contract for the report
// the Node host emits (desktop/src/main/commands/quality.ts). Keep it in sync
// with that interface. `feedback_*` are SQL COUNT(*) values serialized as plain
// JS numbers over IPC (NOT bigint) — the counts never approach 2^53.

export type DraftBrainQualityReport = { feedback_total: number, feedback_unsynced: number, model_pack_version: string | null, data_pack_version: string | null, data_pack_confidence: string | null, data_pack_generated_at: number | null, data_pack_fresh: boolean | null, local_rules_version: string, cloud_configured: boolean, notes: Array<string>, };
