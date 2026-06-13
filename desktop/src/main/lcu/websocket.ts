// LCU WebSocket watcher — port of src-tauri/src/lcu/websocket.rs onto `ws`.
//
// Parity notes:
// - Exponential backoff with deterministic jitter (2s → 4s → 8s → 16s → 30s cap),
//   same formula as the Rust side so reconnect behaviour matches.
// - Auth (401) failure short-circuits: no infinite reconnect on bad creds.
// - WAMP: send [5, topic] to subscribe; events arrive as [8, topic, { data }].
// - SECURITY UPGRADE over the Rust version: the Rust WS used an accept-all cert
//   verifier; here we pin Riot's root CA (same as the HTTP client).

import type { ClientRequestArgs } from "node:http";

import WebSocket from "ws";

import { riotRootCa } from "./client";
import type { Lockfile } from "./lockfile";

export const CHAMP_SELECT_TOPIC = "OnJsonApiEvent_lol-champ-select_v1_session";
/** Gameflow phase ("ChampSelect" / "InProgress" / "Lobby" / "None" …) — drives
 *  the in-game overlay switch and the champ-select→lobby navigation fail-safe. */
export const GAMEFLOW_PHASE_TOPIC = "OnJsonApiEvent_lol-gameflow_v1_gameflow-phase";

/** Deterministic jitter (no rand dep): up to ~1s, same formula as Rust. */
export function jitterMs(attempt: number): number {
  return (attempt * 137) % 1000;
}

/** 2s, 4s, 8s, 16s, 30s cap (+ jitter) — mirrors `backoff_delay` in Rust. */
export function backoffDelayMs(consecutiveErrors: number): number {
  const baseMs = 2000 * 2 ** Math.min(consecutiveErrors, 4);
  return Math.min(baseMs, 30_000) + jitterMs(consecutiveErrors);
}

export interface WampEvent {
  topic: string;
  /** Event payload (`arr[2].data`); null when the LCU sends an empty update. */
  data: unknown;
}

/** Parse one WS text frame; returns null for anything that isn't a WAMP EVENT. */
export function parseWampEvent(text: string): WampEvent | null {
  let arr: unknown;
  try {
    arr = JSON.parse(text);
  } catch {
    return null;
  }
  if (!Array.isArray(arr) || arr[0] !== 8) return null;
  const payload = arr[2] as { data?: unknown } | undefined;
  return {
    topic: typeof arr[1] === "string" ? arr[1] : "",
    data: payload && "data" in payload ? (payload.data ?? null) : null,
  };
}

export interface WatcherOptions {
  topics?: string[];
  /** Called for every WAMP event on a subscribed topic. */
  onEvent: (event: WampEvent) => void;
  onStatus?: (status: "connected" | "disconnected" | "auth-failed") => void;
  connectTimeoutMs?: number;
  /** Test seam: sleep + WebSocket factory. */
  sleep?: (ms: number) => Promise<void>;
  makeSocket?: (url: string, opts: ClientRequestArgs & { ca: string }) => WebSocket;
}

const defaultSleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/**
 * Reconnecting champ-select watcher. `start()` runs until `stop()`; auth
 * failures (401) end the loop immediately, everything else backs off + retries.
 */
export class LcuEventWatcher {
  private stopped = false;
  private socket: WebSocket | undefined;

  constructor(
    private readonly lockfile: Lockfile,
    private readonly opts: WatcherOptions,
  ) {}

  stop(): void {
    this.stopped = true;
    this.socket?.close();
  }

  async start(): Promise<void> {
    const sleep = this.opts.sleep ?? defaultSleep;
    let consecutiveErrors = 0;

    while (!this.stopped) {
      try {
        const authFailed = await this.runOnce();
        if (authFailed) {
          this.opts.onStatus?.("auth-failed");
          return;
        }
        if (this.stopped) return;
        // Normal close (LCU shut down) — treat like an error: back off, retry.
        consecutiveErrors += 1;
      } catch {
        if (this.stopped) return;
        consecutiveErrors += 1;
      }
      this.opts.onStatus?.("disconnected");
      await sleep(backoffDelayMs(consecutiveErrors));
    }
  }

  /** One connect→listen cycle. Resolves `true` on 401 (do not reconnect). */
  private runOnce(): Promise<boolean> {
    const url = `wss://127.0.0.1:${this.lockfile.port}/`;
    const auth = Buffer.from(`riot:${this.lockfile.password}`).toString("base64");
    const makeSocket =
      this.opts.makeSocket ??
      ((u, o) => new WebSocket(u, { ...o, handshakeTimeout: this.opts.connectTimeoutMs ?? 5000 }));

    return new Promise<boolean>((resolve, reject) => {
      const ws = makeSocket(url, {
        ca: riotRootCa(),
        headers: { Authorization: `Basic ${auth}` },
      });
      this.socket = ws;

      ws.on("unexpected-response", (_req, res) => {
        ws.terminate();
        if (res.statusCode === 401) resolve(true);
        else reject(new Error(`LCU WS upgrade reddedildi: HTTP ${res.statusCode}`));
      });

      ws.on("open", () => {
        this.opts.onStatus?.("connected");
        const topics = this.opts.topics ?? [CHAMP_SELECT_TOPIC];
        for (const topic of topics) ws.send(JSON.stringify([5, topic]));
      });

      ws.on("message", (raw) => {
        const event = parseWampEvent(String(raw));
        if (event) this.opts.onEvent(event);
      });

      ws.on("error", (err) => reject(err));
      ws.on("close", () => resolve(false));
    });
  }
}
