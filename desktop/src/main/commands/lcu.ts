// Port of src-tauri/src/commands/lcu.rs (connect_lcu / get_lcu_status /
// start_ws_listener slice). Failure shape parity: connect_lcu RESOLVES with
// `{connected:false, error}` instead of rejecting — the renderer renders the
// honest disconnected state from the payload.

import { LcuClient, type LcuResponse } from "../lcu/client";
import { findLockfile, type Lockfile } from "../lcu/lockfile";

/** Minimal LCU HTTP yüzeyi — testler yalnız getJson stub'layabilir. */
export interface LcuHttp {
  getJson<T = unknown>(path: string): Promise<T>;
  request?(
    method: "GET" | "POST" | "PUT" | "DELETE" | "PATCH",
    path: string,
    body?: unknown,
  ): Promise<LcuResponse>;
}

/**
 * find_own_pick_action_id paritesi (champ_select.rs:94): grup dizisi içinde
 * yerel oyuncunun tamamlanmamış pick action'ı. Rust'taki `?` davranışı korunur:
 * grup dizi değilse ya da action alanları eksikse TÜM sonuç null.
 */
export function findOwnPickActionId(
  session: Record<string, unknown>,
  localCellId: number,
): number | null {
  const groups = session?.actions;
  if (!Array.isArray(groups)) return null;
  for (const group of groups) {
    if (!Array.isArray(group)) return null;
    for (const action of group as Record<string, unknown>[]) {
      const cell = Number(action?.actorCellId);
      const type = action?.type;
      const completed = action?.completed;
      if (!Number.isFinite(cell) || typeof type !== "string") return null;
      if (cell === localCellId && type === "pick" && !(completed ?? true)) {
        const id = Number(action?.id);
        return Number.isFinite(id) ? id : null;
      }
    }
  }
  return null;
}
import {
  CHAMP_SELECT_TOPIC,
  GAMEFLOW_PHASE_TOPIC,
  LcuEventWatcher,
  type WampEvent,
} from "../lcu/websocket";

export interface LcuStatus {
  connected: boolean;
  summoner_name: string | null;
  region: string | null;
  port: number | null;
  error: string | null;
}

const DISCONNECTED: LcuStatus = {
  connected: false,
  summoner_name: null,
  region: null,
  port: null,
  error: null,
};

export interface LcuServiceOptions {
  /** Renderer'a event yayını (webContents.send). */
  emit: (channel: string, payload: unknown) => void;
  onStatus?: (status: "connected" | "disconnected" | "auth-failed") => void;
  /**
   * Ham LCU session'ı ChampSelectState'e çevirir (core WASM `parse_session_json`).
   * Tauri websocket.rs paritesi: renderer HAM session değil, parse edilmiş
   * snake_case state alır. Geçersiz payload → null → event YAYINLANMAZ.
   */
  parseSession?: (raw: unknown) => unknown | null;
  /**
   * 2B: parse edilmiş ChampSelectState (snake_case) ya da null (seans bitti) —
   * OutcomeTracker commit edilen pick'i buradan izler. emit'e EK olarak çağrılır.
   */
  onChampSelectSession?: (state: unknown | null) => void;
  /**
   * 2B: gameflow faz string'i ("ChampSelect"/"InProgress"/"EndOfGame"…) — pick
   * yazma (InProgress) + oyun-sonu resolve (EndOfGame) tetikleyici. emit'e EK.
   */
  onGameflowPhase?: (phase: string) => void;
  /** Test seam. */
  findLockfileFn?: () => Lockfile;
  makeClient?: (lockfile: Lockfile) => LcuHttp;
}

/** Holds the live LCU connection + champ-select WS watcher (AppState parity). */
export class LcuService {
  private lockfile: Lockfile | undefined;
  private watcher: LcuEventWatcher | undefined;
  private client: LcuHttp | undefined;
  private lastStatus: LcuStatus = { ...DISCONNECTED };

  constructor(private readonly opts: LcuServiceOptions) {}

  /** connect_lcu: lockfile bul → current-summoner sorgula → watcher'ı tazele. */
  async connect(): Promise<LcuStatus> {
    let lockfile: Lockfile;
    try {
      lockfile = (this.opts.findLockfileFn ?? findLockfile)();
    } catch (err) {
      this.lastStatus = { ...DISCONNECTED, error: (err as Error).message };
      return this.lastStatus;
    }

    let summoner: Record<string, unknown>;
    const client: LcuHttp =
      this.opts.makeClient?.(lockfile) ?? new LcuClient(lockfile);
    try {
      summoner = await client.getJson("/lol-summoner/v1/current-summoner");
    } catch (err) {
      this.lastStatus = {
        ...DISCONNECTED,
        port: lockfile.port,
        error: `LCU isteği başarısız: ${(err as Error).message}`,
      };
      return this.lastStatus;
    }

    const gameName =
      (typeof summoner.gameName === "string" && summoner.gameName) ||
      (typeof summoner.displayName === "string" && summoner.displayName) ||
      "Summoner";
    const tagLine = typeof summoner.tagLine === "string" ? summoner.tagLine : "";
    const summonerName = tagLine ? `${gameName}#${tagLine}` : gameName;

    this.lockfile = lockfile;
    this.client = client;
    this.lastStatus = {
      connected: true,
      summoner_name: summonerName,
      region: null, // Rust paritesi: region Riot API fazında dolacak.
      port: lockfile.port,
      error: null,
    };
    this.startWsListener();
    // Seed the current gameflow phase — the WS only pushes future transitions, so
    // without this the renderer wouldn't reflect an already-live match on connect.
    void client
      .getJson<unknown>("/lol-gameflow/v1/gameflow-phase")
      .then((p) => {
        const phase = typeof p === "string" ? p : "";
        this.opts.emit("gameflow-phase", phase);
        this.opts.onGameflowPhase?.(phase);
      })
      .catch(() => undefined);
    return this.lastStatus;
  }

  /** hover_champion paritesi (champ_select.rs:110): aktif pick action'ı PATCH'le. */
  async hoverChampion(championId: number): Promise<void> {
    const client = this.client;
    if (!client) throw new Error("LCU bağlantısı yok"); // AppError::NotConnected
    const session = await client.getJson<Record<string, unknown>>(
      "/lol-champ-select/v1/session",
    );
    const localCellId = Number(session?.localPlayerCellId ?? 0);
    const actionId = findOwnPickActionId(session, localCellId);
    if (actionId === null) throw new Error("Aktif pick action bulunamadı");
    if (!client.request) throw new Error("LCU client PATCH desteklemiyor");
    const res = await client.request(
      "PATCH",
      `/lol-champ-select/v1/session/actions/${actionId}`,
      { championId },
    );
    if (res.status < 200 || res.status >= 300) {
      throw new Error(`Hover PATCH reddedildi: HTTP ${res.status}`);
    }
  }

  /** start_ws_listener: champ-select watcher'ı (yeniden) başlat — idempotent. */
  startWsListener(): void {
    if (!this.lockfile) return;
    this.watcher?.stop();
    this.watcher = new LcuEventWatcher(this.lockfile, {
      topics: [CHAMP_SELECT_TOPIC, GAMEFLOW_PHASE_TOPIC],
      onStatus: (s) => this.opts.onStatus?.(s),
      onEvent: (event: WampEvent) => {
        // Gameflow phase drives navigation: the renderer switches to the in-game
        // overlay on a live match and uses the phase as a fail-safe to leave
        // champ-select if the null event below never arrives (dodge / WS drop).
        // data is the phase string ("ChampSelect" / "InProgress" / "Lobby" / "None").
        if (event.topic === GAMEFLOW_PHASE_TOPIC) {
          const phase = typeof event.data === "string" ? event.data : "";
          this.opts.emit("gameflow-phase", phase);
          this.opts.onGameflowPhase?.(phase);
          return;
        }
        // Tauri websocket.rs paritesi (228-237): null → null yayınla (champ select
        // bitti sinyali); ham session → parse edip STATE yayınla; parse edilemeyen
        // payload → hiç yayınlama.
        if (event.data === null) {
          this.opts.emit("champ-select-session", null);
          this.opts.onChampSelectSession?.(null);
          return;
        }
        if (!this.opts.parseSession) {
          this.opts.emit("champ-select-session", event.data);
          return; // ham (camelCase) shape — OutcomeTracker'a besleme (yanlış-şekil).
        }
        let state: unknown | null = null;
        try {
          state = this.opts.parseSession(event.data);
        } catch {
          state = null; // core "Geçersiz session JSON" fırlatır — yayınlama.
        }
        if (state !== null) {
          this.opts.emit("champ-select-session", state);
          this.opts.onChampSelectSession?.(state);
        }
      },
    });
    void this.watcher.start();
  }

  /** get_lcu_status: son bilinen durum (yeniden sorgulamaz — Rust paritesi). */
  status(): LcuStatus {
    return { ...this.lastStatus };
  }

  /** Bağlı client (varsa) — get_enemy_champion_pools "bağlı değilse boş" yolu. */
  currentClient(): LcuHttp | undefined {
    return this.client;
  }

  /**
   * Bağlı client'ı ver; yoksa lockfile'dan TAZE oluştur (sync_lcu_player_data
   * auto-create paritesi). League çalışmıyorsa lockfile hatası fırlar.
   */
  ensureClient(): LcuHttp {
    if (this.client) return this.client;
    const lockfile = (this.opts.findLockfileFn ?? findLockfile)();
    return this.opts.makeClient?.(lockfile) ?? new LcuClient(lockfile);
  }

  stop(): void {
    this.watcher?.stop();
  }
}
