// Arka plan data-pipeline scheduler'ı — port of src-tauri/src/commands/
// data_quality.rs `start_data_pipeline_scheduler` + `run_scheduler_tick` +
// `record_tick_ramp`. Tick başına: refresh planı core'da kararlaştırılır
// (pipeline_refresh_plan_json), kaynaklar koşulur, fetch-log'lar yazılır,
// başarı varsa cache promotion denenir ve before→after coverage-ramp ölçümü
// bellekte saklanır (get_data_trajectory'nin "unknown"dan çıkması bu ölçüme bağlı).
//
// Kurallar (Rust paritesi):
//   * Manuel sync_data_pipeline ile AYNI refresh kilidi (withRefreshLock).
//   * Champ-select sırasında ağ YASAK — plan her kaynağı skip_champ_select yapar,
//     ramp "no_budget" (deferred) kaydedilir; sahte ölçüm yok.
//   * Ramp kaydı best-effort: hatası tick'i asla düşürmez (işi yukarıdaki refresh).

import type { DatabaseSync } from "node:sqlite";

import {
  cachedPackRow,
  defaultJsonFetch,
  isChampSelectActive,
  promoteDataPack,
  recordFetchLog,
  syncMerakiRates,
  withRefreshLock,
} from "./commands/data-pipeline";
import type { LcuService } from "./commands/lcu";
import { syncDdragonChampions, type DdragonCaches, type FetchJson } from "./ddragon";
import type { Engine } from "./engine";
import { syncMatchV5Ingestion } from "./match-v5";
import { runtimeClientFromEnv, runtimeEnv, type RiotClient } from "./riot/client";
import {
  defaultEdgeFetch,
  defaultLeaguepediaFetch,
  defaultUggFetch,
  syncEdgeRates,
  syncLeaguepedia,
  syncUgg,
} from "./sources";

export const SCHEDULER_INITIAL_DELAY_MS = 30_000;
export const SCHEDULER_INTERVAL_MS = 3 * 60_000;
/** read_fetch_logs penceresi (30 gün) — Rust build_pipeline_scheduler_status. */
const FETCH_LOG_WINDOW_SECS = 30 * 24 * 60 * 60;
/** recent_request_timestamps penceresi (RATE_WINDOW_SECS = 1 saat). */
const RATE_WINDOW_SECS = 60 * 60;
/** MATCH_DISCOVERY_CRAWL_BUDGET — match-v5.ts'teki sabitle aynı (Rust verbatim). */
const MATCH_DISCOVERY_CRAWL_BUDGET = 15;
/** Bu host'ta devre dışı kaynaklar (tümü taşındı → boş; yenisi gelirse buraya). */
const DISABLED_SOURCES: string[] = [];

const nowSecs = () => Math.floor(Date.now() / 1000);

/** Rust `LastCoverageRamp` — scheduler'ın son ramp hükmü (restart'ta sıfırlanır). */
export interface LastCoverageRamp {
  ramp_state: string;
  data_growing: boolean;
  measured_at: number;
}

interface RampSnapshot {
  taken_at: number;
  champion_rate_rows: number;
  matchup_rows: number;
  build_rows: number;
  discovered_matches: number;
  fetched_matches: number;
  processed_matches: number;
  failed_matches: number;
  crawled_players: number;
}

function count(db: DatabaseSync, sql: string): number {
  const row = db.prepare(sql).get() as unknown as Record<string, unknown>;
  return Math.max(0, Number(Object.values(row ?? {})[0] ?? 0));
}

/** Rust `ramp_snapshot` SQL'leri birebir. */
function rampSnapshot(db: DatabaseSync, takenAt: number): RampSnapshot {
  return {
    taken_at: takenAt,
    champion_rate_rows: count(db, "SELECT COUNT(*) FROM champion_rates"),
    matchup_rows: count(db, "SELECT COUNT(*) FROM champion_matchups"),
    build_rows: count(db, "SELECT COUNT(*) FROM builds"),
    discovered_matches: count(
      db,
      "SELECT COUNT(*) FROM match_v5_fetch_history WHERE status = 'discovered'",
    ),
    fetched_matches: count(
      db,
      "SELECT COUNT(*) FROM match_v5_fetch_history WHERE status IN ('fetched', 'parsed')",
    ),
    processed_matches: count(
      db,
      "SELECT COUNT(*) FROM match_v5_fetch_history WHERE status = 'processed'",
    ),
    failed_matches: count(
      db,
      "SELECT COUNT(*) FROM match_v5_fetch_history WHERE status = 'failed'",
    ),
    crawled_players: count(db, "SELECT COUNT(*) FROM match_discovery_players"),
  };
}

/** Rust `read_fetch_logs`: pencere içi tüm girdiler (özet core'da çıkarılır). */
function readFetchLogs(
  db: DatabaseSync,
  since: number,
): { source: string; status: string; at: number }[] {
  return (
    db
      .prepare(
        `SELECT source, status, finished_at AS at FROM source_fetch_log
         WHERE finished_at >= ? ORDER BY finished_at ASC`,
      )
      .all(since) as unknown as { source: string; status: string; at: number }[]
  ).map((r) => ({ source: r.source, status: r.status, at: Number(r.at) }));
}

/** Rust `recent_request_timestamps`: rate penceresindeki refresh denemeleri. */
function recentRequestTimestamps(db: DatabaseSync, since: number): number[] {
  return (
    db
      .prepare(
        `SELECT finished_at FROM source_fetch_log
         WHERE finished_at >= ?
           AND decision = 'refresh'
           AND status IN ('success', 'failed', 'rate_limited')`,
      )
      .all(since) as unknown as { finished_at: number }[]
  ).map((r) => Number(r.finished_at));
}

export interface SchedulerDeps {
  engine: Engine;
  db: DatabaseSync;
  lcu: LcuService;
  caches: DdragonCaches;
  ddragonFetch?: FetchJson;
  merakiFetch?: FetchJson;
  uggFetch?: FetchJson;
  leaguepediaFetch?: FetchJson;
  edgeFetch?: FetchJson;
  /** undefined = env'den çöz (EDGE_BASE_URL); null = edge yok (testler ağa çıkmaz). */
  edgeBaseUrl?: string | null;
  /** undefined = her tick'te env'den çöz; null = key yok (testler ağa çıkmaz). */
  riotClient?: RiotClient | null;
  initialDelayMs?: number;
  intervalMs?: number;
}

interface RefreshPlanStatus {
  champ_select_active: boolean;
  plan: {
    decisions: { source: string; decision: string; reason: string }[];
  };
}

export class PipelineScheduler {
  private timer: NodeJS.Timeout | undefined;
  private running = false;
  private lastRamp: LastCoverageRamp | undefined;

  constructor(private readonly deps: SchedulerDeps) {}

  /** Son arka plan ramp hükmü (get_data_trajectory'ye akar); henüz yoksa null. */
  lastCoverageRamp(): LastCoverageRamp | null {
    return this.lastRamp ?? null;
  }

  /** Idempotent: ilk tick `initialDelayMs` sonra, sonra her `intervalMs`'te bir
   *  (tick bitiminden itibaren — Rust loop+sleep paritesi, üst üste binme yok). */
  start(): void {
    if (this.running) return;
    this.running = true;
    const initial = this.deps.initialDelayMs ?? SCHEDULER_INITIAL_DELAY_MS;
    this.schedule(initial);
    console.log("Data pipeline scheduler started");
  }

  stop(): void {
    this.running = false;
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = undefined;
    }
  }

  private schedule(delayMs: number): void {
    if (!this.running) return;
    this.timer = setTimeout(() => {
      void this.tick()
        .catch((err) =>
          console.warn("Data pipeline scheduler tick failed:", (err as Error).message),
        )
        .finally(() =>
          this.schedule(this.deps.intervalMs ?? SCHEDULER_INTERVAL_MS),
        );
    }, delayMs);
  }

  /** Tek scheduler tick'i (Rust `run_scheduler_tick`). Test edilebilir; manuel
   *  sync ile aynı kilidi paylaşır. */
  tick(): Promise<void> {
    return withRefreshLock(() => this.tickInner());
  }

  private async tickInner(): Promise<void> {
    const { engine, db, lcu, caches } = this.deps;

    // Promotion kıyası REFRESH ÖNCESİ son-iyi cache'e karşı (Rust current_good).
    const currentPack = cachedPackRow(db);
    // Best-effort ramp baseline (salt-okunur sayımlar).
    let rampBefore: RampSnapshot | undefined;
    try {
      rampBefore = rampSnapshot(db, nowSecs());
    } catch {
      rampBefore = undefined;
    }

    const now = nowSecs();
    const champSelectActive = await isChampSelectActive(lcu);
    const riot =
      this.deps.riotClient === undefined
        ? runtimeClientFromEnv()
        : this.deps.riotClient;
    const hasSummoner =
      db.prepare("SELECT 1 FROM summoners LIMIT 1").get() !== undefined;
    const edgeBase =
      this.deps.edgeBaseUrl === undefined
        ? (runtimeEnv().EDGE_BASE_URL ?? "").trim() || null
        : this.deps.edgeBaseUrl;

    const status = engine.pipelineRefreshPlan<RefreshPlanStatus>({
      now_secs: now,
      champ_select_active: champSelectActive,
      riot_enabled: riot !== null && hasSummoner,
      edge_enabled: edgeBase !== null,
      matchup_count: count(db, "SELECT COUNT(*) FROM champion_matchups"),
      logs: readFetchLogs(db, now - FETCH_LOG_WINDOW_SECS),
      request_timestamps: recentRequestTimestamps(db, now - RATE_WINDOW_SECS),
      disabled_sources: DISABLED_SOURCES,
    });

    let anySuccess = false;
    for (const decision of status.plan.decisions) {
      const started = nowSecs();
      if (decision.decision !== "refresh") {
        recordFetchLog(
          db,
          decision.source,
          decision.decision === "skip_rate_limited" ? "rate_limited" : "skipped",
          decision.decision,
          decision.reason,
          started,
          nowSecs(),
        );
        continue;
      }
      try {
        const message = await this.runScheduledSource(decision.source, riot, edgeBase);
        anySuccess = true;
        recordFetchLog(
          db,
          decision.source,
          "success",
          decision.decision,
          message,
          started,
          nowSecs(),
        );
      } catch (err) {
        recordFetchLog(
          db,
          decision.source,
          "failed",
          decision.decision,
          (err as Error).message,
          started,
          nowSecs(),
        );
      }
    }

    if (anySuccess) {
      promoteDataPack(engine, db, caches, currentPack);
    }

    // Bu tick'in ramp hareketini kaydet (trajectory yüzeyi). Hata tick'i düşürmez.
    if (rampBefore) {
      try {
        const measuredAt = nowSecs();
        const report = engine.coverageRamp<{
          ramp_state: string;
          data_growing: boolean;
        }>({
          before: rampBefore,
          after: rampSnapshot(db, measuredAt),
          champ_select_active: champSelectActive,
          crawl_budget: champSelectActive ? 0 : MATCH_DISCOVERY_CRAWL_BUDGET,
        });
        this.lastRamp = {
          ramp_state: report.ramp_state,
          data_growing: report.data_growing,
          measured_at: measuredAt,
        };
      } catch (err) {
        console.warn("coverage-ramp ölçümü kaydedilemedi:", (err as Error).message);
      }
    }
  }

  /** Rust `run_scheduled_source` — kaynak başına refresh koşucusu. */
  private async runScheduledSource(
    source: string,
    riot: RiotClient | null,
    edgeBase: string | null,
  ): Promise<string> {
    const { engine, db, caches } = this.deps;
    switch (source) {
      case "ddragon": {
        const n = await syncDdragonChampions(db, caches, this.deps.ddragonFetch);
        return `${n} champions`;
      }
      case "meraki": {
        const n = await syncMerakiRates(db, this.deps.merakiFetch ?? defaultJsonFetch);
        return `${n} rates`;
      }
      case "u_gg": {
        const o = await syncUgg(db, caches, this.deps.uggFetch ?? defaultUggFetch);
        return `${o.rates} rates, ${o.builds} builds, ${o.matchups} matchups`;
      }
      case "leaguepedia": {
        const n = await syncLeaguepedia(
          db,
          caches,
          this.deps.leaguepediaFetch ?? defaultLeaguepediaFetch,
        );
        return `${n} pro champions`;
      }
      case "cloud_edge": {
        if (!edgeBase) throw new Error("EDGE_BASE_URL yapılandırılmamış");
        const n = await syncEdgeRates(db, edgeBase, this.deps.edgeFetch ?? defaultEdgeFetch);
        return `${n.rates} rates, ${n.matchups} matchups, ${n.builds} builds`;
      }
      case "match_v5": {
        const o = await syncMatchV5Ingestion(engine, db, caches, riot);
        return (
          `${o.fetched_matches} matches, ${o.rates} rates, ` +
          `${o.matchups} matchups, ${o.builds} builds, ${o.detail_errors} errors`
        );
      }
      default:
        throw new Error(`Bilinmeyen pipeline source: ${source}`);
    }
  }
}
