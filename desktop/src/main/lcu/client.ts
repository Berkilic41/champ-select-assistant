// LCU HTTPS client — port of src-tauri/src/lcu/client.rs onto Node `https`.
//
// TLS: the League Client uses a self-signed certificate issued by Riot's
// "LoL Game Engineering" root CA. We PIN that root (resources/riotgames.pem,
// fetched from https://static.developer.riotgames.com/docs/lol/riotgames.pem)
// instead of disabling verification — `rejectUnauthorized: false` is forbidden
// in this codebase (MITM).

import { readFileSync } from "node:fs";
import https from "node:https";
import { join } from "node:path";

import { resourcesDir } from "../paths";
import type { Lockfile } from "./lockfile";

/** Riot's LCU root CA (PEM). Resolved relative to the package resources dir. */
export function riotRootCa(
  pemPath: string = join(resourcesDir(), "riotgames.pem"),
): string {
  return readFileSync(pemPath, "utf8");
}

/** Basic-auth header value for the LCU (`riot:<password>`). */
export function lcuAuthHeader(lockfile: Pick<Lockfile, "password">): string {
  return "Basic " + Buffer.from(`riot:${lockfile.password}`).toString("base64");
}

/** Base URL of the local LCU HTTP API. */
export function lcuBaseUrl(lockfile: Pick<Lockfile, "port" | "protocol">): string {
  return `${lockfile.protocol}://127.0.0.1:${lockfile.port}`;
}

/** WebSocket URL of the LCU event bus (WAMP). */
export function lcuWsUrl(lockfile: Pick<Lockfile, "port">): string {
  return `wss://127.0.0.1:${lockfile.port}`;
}

export interface LcuResponse {
  status: number;
  body: string;
}

/**
 * Minimal LCU HTTP client. One pinned-CA agent per client; `request` resolves
 * with status+body (JSON parsing is the caller's concern, mirroring the Rust
 * client's `Response`-returning surface).
 */
export class LcuClient {
  readonly baseUrl: string;
  private readonly authHeader: string;
  private readonly agent: https.Agent;

  constructor(lockfile: Lockfile, ca: string = riotRootCa()) {
    this.baseUrl = lcuBaseUrl(lockfile);
    this.authHeader = lcuAuthHeader(lockfile);
    this.agent = new https.Agent({ ca, keepAlive: true });
  }

  request(
    method: "GET" | "POST" | "PUT" | "DELETE" | "PATCH",
    path: string,
    body?: unknown,
    timeoutMs = 5000,
  ): Promise<LcuResponse> {
    const url = new URL(path, this.baseUrl);
    const payload = body === undefined ? undefined : JSON.stringify(body);
    return new Promise((resolve, reject) => {
      const req = https.request(
        url,
        {
          method,
          agent: this.agent,
          timeout: timeoutMs,
          headers: {
            Authorization: this.authHeader,
            Accept: "application/json",
            ...(payload !== undefined
              ? { "Content-Type": "application/json" }
              : {}),
          },
        },
        (res) => {
          const chunks: Buffer[] = [];
          res.on("data", (c: Buffer) => chunks.push(c));
          res.on("end", () =>
            resolve({
              status: res.statusCode ?? 0,
              body: Buffer.concat(chunks).toString("utf8"),
            }),
          );
        },
      );
      req.on("timeout", () => req.destroy(new Error(`LCU isteği zaman aşımı (${timeoutMs}ms): ${path}`)));
      req.on("error", reject);
      if (payload !== undefined) req.write(payload);
      req.end();
    });
  }

  /** GET + parse JSON; throws on non-2xx (status included for honest errors). */
  async getJson<T = unknown>(path: string): Promise<T> {
    const res = await this.request("GET", path);
    if (res.status < 200 || res.status >= 300) {
      throw new Error(`LCU ${path} → HTTP ${res.status}: ${res.body.slice(0, 200)}`);
    }
    return JSON.parse(res.body) as T;
  }
}
