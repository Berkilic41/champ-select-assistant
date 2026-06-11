// Data-quality read trio — port of the read-only commands in
// src-tauri/src/commands/data_quality.rs (get_data_source_registry /
// get_pipeline_quality_report / get_data_trajectory). The host only runs the
// same SQL the Rust host ran (counts, cached pack row, per-source GROUP BYs,
// dominant patch, fetch-log last-success); every derivation runs in core via
// the WASM JSON API. Never hits the network.
//
// Trajectory'nin ramp girdisi arka plan scheduler'ından gelir
// (PipelineScheduler.lastCoverageRamp) — ilk tick öncesi null → core dürüstçe
// "unknown" sınıflar (Rust'ta da ilk tick öncesi aynı).

import type { DatabaseSync } from "node:sqlite";

import type { DdragonCaches } from "../ddragon";
import type { Engine } from "../engine";
import type { LastCoverageRamp } from "../scheduler";
import { runtimeClientFromEnv } from "../riot/client";

function count(db: DatabaseSync, sql: string): number {
  const row = db.prepare(sql).get() as unknown as Record<string, unknown>;
  return Math.max(0, Number(Object.values(row ?? {})[0] ?? 0));
}

/** gather_coverage'ın SQL yarısı — sayımlar core'a ham gider. */
function coverageCounts(db: DatabaseSync): Record<string, number> {
  return {
    total_champions: count(db, "SELECT COUNT(*) FROM champions"),
    champion_rates_count: count(db, "SELECT COUNT(*) FROM champion_rates"),
    matchup_count: count(db, "SELECT COUNT(*) FROM champion_matchups"),
    build_count: count(db, "SELECT COUNT(*) FROM builds"),
    build_champions: count(db, "SELECT COUNT(DISTINCT champion_id) FROM builds"),
    meta_role_champions: count(
      db,
      "SELECT COUNT(DISTINCT champion_id) FROM champion_rates",
    ),
  };
}

function dataPackCache(
  db: DatabaseSync,
): { source: string | null; expires_at: number | null } | null {
  const row = db
    .prepare(
      "SELECT source, expires_at FROM draft_brain_packs WHERE kind = 'data_pack'",
    )
    .get() as unknown as
    | { source: string | null; expires_at: number | null }
    | undefined;
  if (!row) return null;
  return {
    source: row.source ?? null,
    expires_at: row.expires_at === null ? null : Number(row.expires_at),
  };
}

/** pipeline_sources'ın SQL yarısı (risk türetimi core'da). */
function pipelineSourceRows(
  db: DatabaseSync,
): { source: string; updated_at: number }[] {
  const queries = [
    "SELECT 'rates:' || source AS source, MAX(cached_at) AS updated_at FROM champion_rates GROUP BY source",
    "SELECT 'builds:' || source AS source, MAX(cached_at) AS updated_at FROM builds GROUP BY source",
    "SELECT 'matchups:' || source AS source, MAX(cached_at) AS updated_at FROM champion_matchups GROUP BY source",
    "SELECT 'pack:' || COALESCE(source, 'unknown') AS source, fetched_at AS updated_at FROM draft_brain_packs WHERE kind = 'data_pack'",
  ];
  const rows: { source: string; updated_at: number }[] = [];
  for (const sql of queries) {
    for (const raw of db.prepare(sql).all() as unknown as {
      source: string;
      updated_at: number | null;
    }[]) {
      if (typeof raw.source === "string" && raw.source) {
        rows.push({ source: raw.source, updated_at: Number(raw.updated_at ?? 0) });
      }
    }
  }
  return rows;
}

/** Rates/builds/matchups genelinde baskın patch (optional_string semantiği). */
function dominantDataPatch(db: DatabaseSync): string | null {
  const row = db
    .prepare(
      `SELECT patch FROM (
           SELECT patch AS patch FROM champion_rates WHERE patch != 'unknown'
           UNION ALL
           SELECT patch_version AS patch FROM builds WHERE patch_version != 'unknown'
           UNION ALL
           SELECT patch_version AS patch FROM champion_matchups WHERE patch_version != 'unknown'
       )
       GROUP BY patch
       ORDER BY COUNT(*) DESC
       LIMIT 1`,
    )
    .get() as unknown as { patch?: string | null } | undefined;
  const patch = row?.patch;
  return typeof patch === "string" && patch && patch !== "unknown" ? patch : null;
}

export function getDataSourceRegistry(engine: Engine, db: DatabaseSync): unknown {
  return engine.dataSourceRegistry({
    counts: coverageCounts(db),
    data_pack: dataPackCache(db),
    now_secs: Math.floor(Date.now() / 1000),
  });
}

export function getPipelineQualityReport(
  engine: Engine,
  db: DatabaseSync,
  caches: DdragonCaches,
): unknown {
  return engine.pipelineQualityReport({
    counts: coverageCounts(db),
    data_pack: dataPackCache(db),
    now_secs: Math.floor(Date.now() / 1000),
    current_patch: caches.version ?? "",
    data_patch: dominantDataPatch(db),
    source_rows: pipelineSourceRows(db),
    pack_exists: dataPackCache(db) !== null,
  });
}

export function getDataTrajectory(
  engine: Engine,
  db: DatabaseSync,
  caches: DdragonCaches,
  ramp: LastCoverageRamp | null = null,
): unknown {
  const quality = getPipelineQualityReport(engine, db, caches) as {
    status: string;
  };
  const now = Math.floor(Date.now() / 1000);
  const since = now - 30 * 24 * 60 * 60;
  const lastV5 = db
    .prepare(
      `SELECT MAX(finished_at) AS at FROM source_fetch_log
       WHERE finished_at >= ? AND source = 'match_v5' AND status = 'success'`,
    )
    .get(since) as unknown as { at: number | null } | undefined;
  const hasSummoner =
    db.prepare("SELECT 1 FROM summoners LIMIT 1").get() !== undefined;

  return engine.dataTrajectory({
    quality_status: quality.status,
    // İlk scheduler tick'i öncesi null → core dürüstçe "unknown" sınıflar.
    ramp,
    riot_key_present: runtimeClientFromEnv() !== null,
    has_summoner: hasSummoner,
    last_match_v5_success_at:
      typeof lastV5?.at === "number" ? lastV5.at : null,
    now_secs: now,
  });
}
