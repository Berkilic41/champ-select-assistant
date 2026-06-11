// Live Client Data API fetcher — the network half of
// src-tauri/src/riot/live_client.rs's `LiveClientApi`. All parsing and every
// decision run in core (csa_core::live_client) via the WASM JSON API; this file
// only fetches the raw `allgamedata` payload from the local game client.
//
// TLS: the local game client (port 2999) serves Riot's self-signed chain; we pin
// the same Riot root CA already shipped for the LCU (resources/riotgames.pem —
// Riot's docs publish it for BOTH the LCU and the Live Client Data API) instead
// of disabling verification. `rejectUnauthorized: false` is forbidden in this
// codebase even though the Rust host used accept-all here — if the pin ever
// fails against a real game, that surfaces in the live smoke test, not silently.
//
// Any failure (connection refused = no live game, timeout, TLS, bad JSON)
// resolves to `null` — the caller's quiet "no live game" signal, mirroring the
// Rust host's `Err(_) => no game` handling. Never rejects, never logs as error.

import https from "node:https";

import { riotRootCa } from "./lcu/client";

const BASE_URL = "https://127.0.0.1:2999";

/** Returns the raw `allgamedata` JSON, or `null` when no live game is reachable. */
export type FetchAllGameData = () => Promise<unknown | null>;

export function createLiveClientFetcher(
  ca: string = riotRootCa(),
): FetchAllGameData {
  const agent = new https.Agent({ ca });
  return () =>
    new Promise((resolve) => {
      const req = https.get(
        `${BASE_URL}/liveclientdata/allgamedata`,
        { agent, timeout: 2000, headers: { Accept: "application/json" } },
        (res) => {
          const chunks: Buffer[] = [];
          res.on("data", (c: Buffer) => chunks.push(c));
          res.on("end", () => {
            const status = res.statusCode ?? 0;
            if (status < 200 || status >= 300) return resolve(null);
            try {
              resolve(JSON.parse(Buffer.concat(chunks).toString("utf8")));
            } catch {
              resolve(null);
            }
          });
        },
      );
      // destroy() makes the request emit 'error', which resolves null below.
      req.on("timeout", () => req.destroy());
      req.on("error", () => resolve(null));
    });
}
