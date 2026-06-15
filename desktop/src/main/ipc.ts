// IPC surface — the Electron equivalent of src-tauri's `commands/`. The renderer
// calls everything through ONE dispatcher channel ("cmd", Tauri-invoke parity);
// handlers register into the command registry one at a time as the ~70 Tauri
// commands migrate. Unmigrated commands fail loudly + honestly.

import type { DatabaseSync } from "node:sqlite";

import { BrowserWindow, ipcMain, type IpcMainInvokeEvent } from "electron";

import {
  completeOnboarding,
  getSettings,
  isOnboardingComplete,
  saveSettings,
  type AppSettings,
} from "./commands/settings";
import {
  getBanSuggestions,
  getChampionAnalysis,
  getChampionArchetypes,
  getChampionDetail,
  getComboBoard,
  getCounterItems,
  getCounterPicks,
  getDraftFork,
  getDraftSimulation,
  getDraftVerdict,
  getGamePlan,
  getLaneMatchup,
  getTeamComp,
} from "./commands/analysis";
import { getIngamePlan, getMacroState } from "./commands/overlay";
import { syncDataPipeline } from "./commands/data-pipeline";
import {
  getDataSourceRegistry,
  getDataTrajectory,
  getPipelineQualityReport,
} from "./commands/data-quality";
import { syncRecommendationFeedback } from "./commands/feedback-flush";
import {
  generateGameReviews,
  getGameReviews,
  getMatchNote,
  getTrendReport,
  setMatchNote,
} from "./commands/game-review";
import {
  getChampionPreferences,
  getMetaTrend,
  setChampionPreference,
} from "./commands/preferences";
import { getSessionCoach, getWeeklySummary } from "./commands/session-coach";
import { rankedStats } from "./repos";
import { syncLcuPlayerData } from "./commands/player-data";
import { getDraftBrainQualityReport } from "./commands/quality";
import {
  syncMasteries,
  syncMatchHistory,
  syncRiotPlayer,
} from "./commands/riot-sync";
import { getChampions } from "./commands/champions";
import {
  getChampionPoolPlan,
  getFeedbackAnalytics,
  getFeedbackObservability,
  getPoolSuggestions,
} from "./commands/insights";
import type { LcuService } from "./commands/lcu";
import {
  getActiveSummonerPuuid,
  getMasteries,
  getMasteryProgress,
  getPerformanceReport,
  getPlayerStats,
  submitRecommendationFeedback,
  type RecommendationFeedbackInput,
} from "./commands/player";
import { getRecommendations } from "./commands/recommendations";
import { getWinProbEstimates } from "./commands/win-prob";
import { trainLocalModelPack } from "./commands/model-training";
import { getComboOutcomes } from "./commands/combo-outcomes";
import { getCoachNarrative, type CoachNarrativeInput } from "./commands/coach-narrative";
import type { RecsCache } from "./commands/outcomes";
import {
  getDdragonVersion,
  syncCdragonMeta,
  syncDdragonChampions,
  type DdragonCaches,
} from "./ddragon";
import {
  hideWindow,
  setAlwaysOnTop,
  setOverlayMode,
  setWindowOpacity,
  setWindowSize,
  showWindow,
} from "./commands/window";
import type { Engine } from "./engine";
import type { FetchAllGameData } from "./live-client";
import type { PipelineScheduler } from "./scheduler";

export interface AppStatus {
  engine: "ready" | "failed";
  db: { migrated: number; error?: string };
  lcu: "connected" | "disconnected" | "auth-failed" | "searching";
}

export type CommandHandler = (
  args: Record<string, unknown> | undefined,
) => Promise<unknown> | unknown;

export interface IpcContext {
  engine: Engine | undefined;
  db: DatabaseSync | undefined;
  lcu: LcuService | undefined;
  /** DDragon item/rune/version bellek-içi cache'i (AppState paritesi). */
  caches: DdragonCaches;
  /** 2B: get_recommendations'ın son sonucu — pick anında öneri→sonuç linkage
   *  için OutcomeTracker okur. Yoksa (test ctx'i) öneri cache'lemesi atlanır. */
  recsCache?: RecsCache;
  /** Live Client Data (port 2999) fetch'i — null = canlı oyun yok (sessiz). */
  fetchAllGameData: FetchAllGameData;
  /** Arka plan pipeline scheduler'ı (ramp ölçümü trajectory'ye akar);
   *  boot başarısızsa undefined olabilir. */
  scheduler?: PipelineScheduler;
  getMainWindow: () => BrowserWindow | undefined;
  status: () => AppStatus;
}

function requireDb(ctx: IpcContext): DatabaseSync {
  if (!ctx.db) throw new Error("veritabanı açılamadı (boot hatası)");
  return ctx.db;
}

function requireWindow(ctx: IpcContext): BrowserWindow {
  const win = ctx.getMainWindow();
  if (!win) throw new Error("ana pencere yok");
  return win;
}

function requireLcu(ctx: IpcContext): LcuService {
  if (!ctx.lcu) throw new Error("LCU servisi başlatılamadı");
  return ctx.lcu;
}

function requireEngine(ctx: IpcContext): Engine {
  if (!ctx.engine) throw new Error("csa-core motoru yüklenemedi");
  return ctx.engine;
}

/** Tauri komut adı → Node handler. Göç ettikçe genişler. */
export function buildCommandRegistry(
  ctx: IpcContext,
): Map<string, CommandHandler> {
  const commands = new Map<string, CommandHandler>();

  commands.set("get_app_status", () => ctx.status());

  // settings.rs
  commands.set("get_settings", () => getSettings(requireDb(ctx)));
  commands.set("save_settings", (args) =>
    saveSettings(requireDb(ctx), args?.settings as AppSettings),
  );
  commands.set("is_onboarding_complete", () =>
    isOnboardingComplete(requireDb(ctx)),
  );
  commands.set("complete_onboarding", () => completeOnboarding(requireDb(ctx)));

  // window.rs
  commands.set("set_always_on_top", (args) =>
    setAlwaysOnTop(requireWindow(ctx), Boolean(args?.enabled)),
  );
  commands.set("set_window_size", (args) =>
    setWindowSize(requireWindow(ctx), String(args?.preset ?? "standard")),
  );
  commands.set("hide_window", () => hideWindow(requireWindow(ctx)));
  commands.set("show_window", () => showWindow(requireWindow(ctx)));
  commands.set("set_overlay_mode", (args) =>
    setOverlayMode(requireWindow(ctx), Boolean(args?.enabled)),
  );
  commands.set("set_window_opacity", (args) =>
    setWindowOpacity(requireWindow(ctx), Number(args?.opacity ?? 1)),
  );

  // lcu.rs (durum dilimi)
  commands.set("connect_lcu", () => requireLcu(ctx).connect());
  commands.set("get_lcu_status", () => requireLcu(ctx).status());
  commands.set("start_ws_listener", () => requireLcu(ctx).startWsListener());

  // champ_select.rs:110 hover_champion — LCU pick action PATCH.
  commands.set("hover_champion", (args) =>
    requireLcu(ctx).hoverChampion(Number(args?.championId ?? 0)),
  );

  // ddragon.rs
  commands.set("get_champions", () => getChampions(requireDb(ctx)));
  commands.set("get_ddragon_version", () => getDdragonVersion(ctx.caches));
  commands.set("sync_ddragon_champions", () =>
    syncDdragonChampions(requireDb(ctx), ctx.caches),
  );
  commands.set("sync_cdragon_meta", () => syncCdragonMeta(requireDb(ctx)));

  // champ_select.rs (öneri dilimi) — get_draft_brain_recommendations Rust'ta da
  // get_recommendations'a delege eden bir alias (champ_select.rs:584).
  const recommendations = (args: Record<string, unknown> | undefined) => {
    const puuid = String(args?.puuid ?? "");
    const recs = getRecommendations(
      requireEngine(ctx),
      requireDb(ctx),
      args?.sessionJson,
      puuid,
      ctx.caches,
    );
    // 2B: bu seansın öneri listesini cache'le; oyuncu commit edip oyun başlayınca
    // OutcomeTracker pick'in rank/score'unu buradan okuyup pending etiket yazar.
    if (puuid) ctx.recsCache?.set(puuid, recs);
    return recs;
  };
  commands.set("get_recommendations", recommendations);
  commands.set("get_draft_brain_recommendations", recommendations);

  // FAZ 3 / 3A: öneri skorları → kalibre kazanma olasılığı (ayrı komut; hot
  // recommendation yolunu DEĞİŞTİRMEZ). Min-sample altında available=false.
  commands.set("get_win_prob_estimates", (args) =>
    getWinProbEstimates(
      requireEngine(ctx),
      requireDb(ctx),
      Array.isArray(args?.scores) ? (args.scores as number[]) : [],
    ),
  );

  // FAZ 3 / 3B: yerel etiketlerden öğrenilen ModelPack'i (yeniden) eğit + persist.
  // Gate altı → null (rules fallback). recommendations otomatik tüketir.
  commands.set("train_local_model_pack", () =>
    trainLocalModelPack(requireEngine(ctx), requireDb(ctx)),
  );

  // FAZ 3 / 3C: oyuncunun co-pick combo geçmişi (display-augment; scoring'e
  // dokunmaz). Renderer combo ipucunda "geçmişin: nM %W" gösterir.
  commands.set("get_combo_outcomes", () => getComboOutcomes(requireDb(ctx)));

  // FAZ 4 / Sprint 1: grounded koçluk notu (pluggable seam). candidate yoksa
  // deterministik; gelecekte LLM provider candidate sağlar, core audit'ler+fallback.
  commands.set("get_coach_narrative", (args) =>
    getCoachNarrative(
      requireEngine(ctx),
      requireDb(ctx),
      (args ?? {}) as unknown as CoachNarrativeInput,
    ),
  );

  // champ_select.rs (analiz kümesi) — tüm karar mantığı core WASM'da.
  commands.set("get_champion_analysis", (args) =>
    getChampionAnalysis(
      requireEngine(ctx),
      requireDb(ctx),
      args?.sessionJson,
      Number(args?.championId ?? 0),
      String(args?.puuid ?? ""),
      ctx.caches,
    ),
  );
  commands.set("get_counter_picks", (args) =>
    getCounterPicks(
      requireEngine(ctx),
      requireDb(ctx),
      args?.sessionJson,
      String(args?.puuid ?? ""),
      ctx.caches,
    ),
  );
  commands.set("get_draft_verdict", (args) =>
    getDraftVerdict(
      requireEngine(ctx),
      requireDb(ctx),
      args?.sessionJson,
      String(args?.puuid ?? ""),
      ctx.caches,
    ),
  );
  commands.set("get_ban_suggestions", (args) =>
    getBanSuggestions(
      requireEngine(ctx),
      requireDb(ctx),
      args?.sessionJson,
      String(args?.puuid ?? ""),
    ),
  );
  commands.set("get_team_comp", (args) =>
    getTeamComp(requireEngine(ctx), requireDb(ctx), args?.sessionJson),
  );
  commands.set("get_game_plan", (args) =>
    getGamePlan(requireEngine(ctx), requireDb(ctx), args?.sessionJson),
  );
  commands.set("get_combo_board", (args) =>
    getComboBoard(requireEngine(ctx), requireDb(ctx), args?.sessionJson),
  );
  commands.set("get_lane_matchup", (args) =>
    getLaneMatchup(requireEngine(ctx), requireDb(ctx), args?.sessionJson),
  );
  commands.set("get_counter_items", (args) =>
    getCounterItems(
      requireEngine(ctx),
      requireDb(ctx),
      args?.sessionJson,
      ctx.caches,
    ),
  );
  commands.set("get_champion_archetypes", () =>
    getChampionArchetypes(requireEngine(ctx)),
  );
  commands.set("get_champion_detail", (args) =>
    getChampionDetail(requireEngine(ctx), Number(args?.championId ?? 0)),
  );

  // commands/draft_simulator.rs — aday filtreleme + sim/fork kararları core'da.
  commands.set("get_draft_simulation", (args) =>
    getDraftSimulation(
      requireEngine(ctx),
      requireDb(ctx),
      args?.sessionJson,
      (args?.candidateIds as number[]) ?? [],
    ),
  );
  commands.set("get_draft_fork", (args) =>
    getDraftFork(
      requireEngine(ctx),
      requireDb(ctx),
      args?.sessionJson,
      Number(args?.optionAId ?? 0),
      Number(args?.optionBId ?? 0),
    ),
  );

  // commands/overlay.rs — "canlı oyun yok" hata DEĞİL (sessiz polling).
  commands.set("get_ingame_plan", () =>
    getIngamePlan(requireEngine(ctx), requireDb(ctx), ctx.fetchAllGameData),
  );
  commands.set("get_macro_state", () =>
    getMacroState(requireEngine(ctx), ctx.fetchAllGameData),
  );

  // commands/riot.rs — Riot API sync üçlüsü (key .env'den her çağrıda tazelenir).
  commands.set("sync_riot_player", (args) =>
    syncRiotPlayer(
      requireDb(ctx),
      String(args?.gameName ?? ""),
      String(args?.tagLine ?? ""),
      String(args?.region ?? ""),
    ),
  );
  commands.set("sync_match_history", (args) =>
    syncMatchHistory(
      requireDb(ctx),
      String(args?.puuid ?? ""),
      Number(args?.count ?? 20),
    ),
  );
  commands.set("sync_masteries", (args) =>
    syncMasteries(requireDb(ctx), String(args?.puuid ?? "")),
  );

  // commands/lcu.rs sync_lcu_player_data.
  commands.set("sync_lcu_player_data", (args) =>
    syncLcuPlayerData(
      requireDb(ctx),
      requireLcu(ctx),
      typeof args?.region === "string" ? args.region : undefined,
    ),
  );

  // commands/feedback_flush.rs + draft_brain.rs quality raporu.
  commands.set("sync_recommendation_feedback", () =>
    syncRecommendationFeedback(requireEngine(ctx), requireDb(ctx)),
  );
  commands.set("get_draft_brain_quality_report", () =>
    getDraftBrainQualityReport(requireDb(ctx)),
  );

  // data_quality.rs okuma üçlüsü — tüm türetimler core WASM'da, ağ YOK.
  commands.set("get_data_source_registry", () =>
    getDataSourceRegistry(requireEngine(ctx), requireDb(ctx)),
  );
  commands.set("get_pipeline_quality_report", () =>
    getPipelineQualityReport(requireEngine(ctx), requireDb(ctx), ctx.caches),
  );
  commands.set("get_data_trajectory", () =>
    getDataTrajectory(
      requireEngine(ctx),
      requireDb(ctx),
      ctx.caches,
      ctx.scheduler?.lastCoverageRamp() ?? null,
    ),
  );

  // data_quality.rs sync_data_pipeline — manuel refresh; Match-V5 adımı henüz
  // taşınmadı, özet errors'ta dürüstçe raporlanır.
  commands.set("sync_data_pipeline", () =>
    syncDataPipeline(
      requireEngine(ctx),
      requireDb(ctx),
      requireLcu(ctx),
      ctx.caches,
    ),
  );

  // riot.rs / postgame.rs / draft_brain.rs (DB dilimleri)
  commands.set("get_player_stats", (args) =>
    getPlayerStats(requireDb(ctx), String(args?.puuid ?? "")),
  );
  commands.set("get_masteries", (args) =>
    getMasteries(requireDb(ctx), String(args?.puuid ?? "")),
  );
  commands.set("get_active_summoner_puuid", () =>
    getActiveSummonerPuuid(requireDb(ctx)),
  );
  commands.set("get_mastery_progress", (args) =>
    getMasteryProgress(
      requireDb(ctx),
      String(args?.puuid ?? ""),
      Number(args?.days ?? 7),
    ),
  );
  commands.set("get_performance_report", (args) =>
    getPerformanceReport(
      requireEngine(ctx),
      requireDb(ctx),
      String(args?.puuid ?? ""),
    ),
  );
  commands.set("submit_recommendation_feedback", (args) =>
    submitRecommendationFeedback(
      requireDb(ctx),
      args?.feedback as RecommendationFeedbackInput,
    ),
  );

  // pool_coach.rs / champ_select.rs:1283 / scouting.rs / data_quality.rs (insight dilimi)
  commands.set("get_pool_suggestions", (args) =>
    getPoolSuggestions(
      requireEngine(ctx),
      requireDb(ctx),
      String(args?.role ?? ""),
      String(args?.puuid ?? ""),
    ),
  );
  commands.set("get_champion_pool_plan", (args) =>
    getChampionPoolPlan(
      requireEngine(ctx),
      requireDb(ctx),
      String(args?.role ?? ""),
      String(args?.puuid ?? ""),
    ),
  );
  // Koç döngüsü (C1+C2): karne üretimi + okuma.
  commands.set("generate_game_reviews", (args) =>
    generateGameReviews(requireEngine(ctx), requireDb(ctx), String(args?.puuid ?? "")),
  );
  commands.set("get_game_reviews", (args) =>
    getGameReviews(
      requireDb(ctx),
      String(args?.puuid ?? ""),
      typeof args?.limit === "number" ? args.limit : 3,
    ),
  );
  commands.set("get_trend_report", (args) =>
    getTrendReport(requireEngine(ctx), requireDb(ctx), String(args?.puuid ?? "")),
  );
  // D3 tercihler + D4 meta trend.
  commands.set("get_champion_preferences", (args) =>
    getChampionPreferences(requireDb(ctx), String(args?.puuid ?? "")),
  );
  commands.set("set_champion_preference", (args) =>
    setChampionPreference(
      requireDb(ctx),
      String(args?.puuid ?? ""),
      Number(args?.championId ?? 0),
      args?.preference === "never" || args?.preference === "learning"
        ? args.preference
        : null,
    ),
  );
  commands.set("get_meta_trend", (args) =>
    getMetaTrend(
      requireDb(ctx),
      Number(args?.championId ?? 0),
      String(args?.position ?? ""),
    ),
  );
  // D2: rank bağlamı (LCU keyless çekildi, sync'te tazelenir).
  commands.set("get_ranked_stats", (args) =>
    rankedStats(requireDb(ctx), String(args?.puuid ?? "")),
  );
  // Faz F: seans koçluğu + haftalık özet.
  commands.set("get_session_coach", (args) =>
    getSessionCoach(requireEngine(ctx), requireDb(ctx), String(args?.puuid ?? "")),
  );
  commands.set("get_weekly_summary", (args) =>
    getWeeklySummary(requireDb(ctx), String(args?.puuid ?? "")),
  );
  commands.set("get_match_note", (args) =>
    getMatchNote(requireDb(ctx), String(args?.matchId ?? "")),
  );
  commands.set("set_match_note", (args) =>
    setMatchNote(
      requireDb(ctx),
      String(args?.puuid ?? ""),
      String(args?.matchId ?? ""),
      String(args?.note ?? ""),
      Array.isArray(args?.tags) ? (args.tags as string[]) : [],
    ),
  );
  commands.set("get_feedback_observability", () =>
    getFeedbackObservability(requireEngine(ctx), requireDb(ctx)),
  );
  commands.set("get_feedback_analytics", (args) =>
    getFeedbackAnalytics(
      requireEngine(ctx),
      requireDb(ctx),
      args?.windowDays === undefined ? undefined : Number(args.windowDays),
    ),
  );

  return commands;
}

/**
 * Sender allowlist (defence-in-depth). Every IPC handler must only honour calls
 * that originate from OUR renderer: the packaged `file://` page or the dev-server
 * origin (HMR). A compromised/iframed remote frame attempting to reach `ipcMain`
 * is rejected loudly before any handler runs. Mirrors window.ts:will-navigate.
 */
function assertTrustedSender(event: IpcMainInvokeEvent): void {
  const url = event.senderFrame?.url ?? "";
  if (url.startsWith("file://")) return;
  const devUrl = process.env.ELECTRON_RENDERER_URL;
  if (devUrl && url.startsWith(devUrl)) return;
  throw new Error("yetkisiz IPC göndericisi");
}

export function registerIpcHandlers(ctx: IpcContext): void {
  const commands = buildCommandRegistry(ctx);

  ipcMain.handle("cmd", (event, cmd: unknown, args: unknown) => {
    assertTrustedSender(event);
    if (typeof cmd !== "string") throw new Error("geçersiz komut çağrısı");
    const handler = commands.get(cmd);
    if (!handler) {
      // Dürüst hata: UI sessizce boş veri görmesin (silent-fallback yasak).
      throw new Error(`komut henüz Electron host'a taşınmadı: ${cmd}`);
    }
    return handler(args as Record<string, unknown> | undefined);
  });

  ipcMain.handle("core:smoke", (event) => {
    assertTrustedSender(event);
    if (!ctx.engine) throw new Error("csa-core motoru yüklenemedi");
    return "csa-core wasm alive";
  });

  ipcMain.handle("app:status", (event): AppStatus => {
    assertTrustedSender(event);
    return ctx.status();
  });
}
