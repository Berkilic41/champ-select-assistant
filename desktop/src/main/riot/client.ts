// Riot API client — port of src-tauri/src/riot/client.rs + endpoints/*.
//
// Behaviour parity:
//   * The key is re-read from the environment (+ nearest `.env`, which FILLS gaps
//     only) on EVERY client build — dev keys rotate daily, the app must pick a new
//     one up without a restart. The real process environment WINS over the file so
//     a stray/hostile `.env` in cwd can never silently redirect API traffic.
//   * `PROXY_URL` set → requests go through `{base}/{host}{path}` WITHOUT the
//     X-Riot-Token header (the proxy holds the key server-side).
//   * Rate limit 20 req/s (sequential 50ms spacing — the Rust governor's
//     steady-state behaviour for this single-consumer client).
//   * 429 → honor Retry-After (+ ≤250ms jitter), one retry. 5xx → one jittered
//     backoff retry. Errors carry the status + URL ("Riot API error ...").
//
// The API key is NEVER logged and never persisted by this module.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";

const RATE_LIMIT_INTERVAL_MS = 50; // 20 req/s

/** Parse a minimal KEY=VALUE .env file (no export/quote handling beyond trim). */
export function parseEnvFile(content: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const line of content.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const eq = trimmed.indexOf("=");
    if (eq <= 0) continue;
    const key = trimmed.slice(0, eq).trim();
    let value = trimmed.slice(eq + 1).trim();
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }
    out[key] = value;
  }
  return out;
}

/** Nearest `.env` content walking up from `start` (8 levels, Rust parity). */
function readNearestEnvFile(start: string): string | null {
  let dir = start;
  for (let i = 0; i < 8; i++) {
    try {
      return readFileSync(join(dir, ".env"), "utf8");
    } catch {
      /* yok — yukarı çık */
    }
    const parent = dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return null;
}

/** Runtime env: process.env wins; nearest .env only FILLS empty/missing keys. */
export function runtimeEnv(
  startDirs: string[] = [process.cwd(), __dirname],
): Record<string, string | undefined> {
  const merged: Record<string, string | undefined> = { ...process.env };
  for (const start of startDirs) {
    const content = readNearestEnvFile(start);
    if (content === null) continue;
    // Security (defence-in-depth): the real process environment WINS. A `.env`
    // dropped into cwd by a stray/hostile actor can only fill a gap — it can
    // never override a RIOT_API_KEY / PROXY_URL already exported, so it cannot
    // silently redirect API traffic. Dev rotation still works: devs keep the key
    // in `.env` only, not also as a shell export.
    for (const [k, v] of Object.entries(parseEnvFile(content))) {
      if (!nonEmpty(merged[k])) merged[k] = v;
    }
    break; // İLK bulunan .env kullanılır
  }
  return merged;
}

function nonEmpty(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

/** Platform region ("tr1") → Match-V5 routing region ("europe"). */
export function routingForRegion(region: string): string {
  switch (region.toLowerCase()) {
    case "euw1":
    case "eune1":
    case "tr1":
    case "ru":
      return "europe";
    case "na1":
    case "la1":
    case "la2":
      return "americas";
    case "kr":
    case "jp1":
      return "asia";
    default:
      return "europe";
  }
}

export interface SummonerInfo {
  puuid: string;
  game_name: string;
  tag_line: string;
  region: string;
}

export interface SyncResult {
  synced: number;
  skipped: number;
  errors: number;
}

/** Test seam: fetch a URL with headers, return status + parsed JSON body. */
export type RiotFetch = (
  url: string,
  headers: Record<string, string>,
) => Promise<{ status: number; retryAfterSecs?: number; json: () => Promise<unknown> }>;

const realFetch: RiotFetch = async (url, headers) => {
  const res = await fetch(url, { headers });
  const retryAfter = Number(res.headers.get("Retry-After"));
  return {
    status: res.status,
    retryAfterSecs: Number.isFinite(retryAfter) ? retryAfter : undefined,
    json: () => res.json(),
  };
};

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const jitterMs = (max: number) => Math.floor(Math.random() * Math.max(max, 1));

export class RiotClient {
  private lastRequestAt = 0;
  private queue: Promise<unknown> = Promise.resolve();

  constructor(
    private readonly apiKey: string,
    private readonly proxyBase: string | null = null,
    private readonly fetchImpl: RiotFetch = realFetch,
    /** Test seam: sleeps disabled in unit tests. */
    private readonly sleepImpl: (ms: number) => Promise<unknown> = sleep,
  ) {}

  private effectiveUrl(url: string): string {
    if (!this.proxyBase) return url;
    const stripped = url.replace(/^https:\/\//, "");
    return `${this.proxyBase.replace(/\/+$/, "")}/${stripped}`;
  }

  private headers(): Record<string, string> {
    return this.proxyBase ? {} : { "X-Riot-Token": this.apiKey };
  }

  /** Serialized + spaced (20 req/s) GET with the Rust client's retry shape. */
  get<T = unknown>(url: string): Promise<T> {
    const run = this.queue.then(() => this.getInner<T>(url));
    // Hata zinciri kuyruğu kilitlemesin — sıradaki istek yine de çalışır.
    this.queue = run.catch(() => undefined);
    return run;
  }

  private async rateLimit(): Promise<void> {
    const wait = this.lastRequestAt + RATE_LIMIT_INTERVAL_MS - Date.now();
    if (wait > 0) await this.sleepImpl(wait);
    this.lastRequestAt = Date.now();
  }

  private async getInner<T>(url: string): Promise<T> {
    await this.rateLimit();
    const effective = this.effectiveUrl(url);
    const resp = await this.fetchImpl(effective, this.headers());

    if (resp.status === 429) {
      const waitSecs = resp.retryAfterSecs ?? 5;
      await this.sleepImpl(waitSecs * 1000 + jitterMs(250));
      await this.rateLimit();
      const retry = await this.fetchImpl(effective, this.headers());
      if (retry.status < 200 || retry.status >= 300) {
        throw new Error(`Riot API error ${retry.status} (after retry): ${url}`);
      }
      return (await retry.json()) as T;
    }

    if (resp.status >= 500) {
      await this.sleepImpl(250 + jitterMs(250));
      await this.rateLimit();
      const retry = await this.fetchImpl(effective, this.headers());
      if (retry.status < 200 || retry.status >= 300) {
        throw new Error(`Riot API error ${retry.status} (after 5xx retry): ${url}`);
      }
      return (await retry.json()) as T;
    }

    if (resp.status < 200 || resp.status >= 300) {
      throw new Error(`Riot API error ${resp.status}: ${url}`);
    }
    return (await resp.json()) as T;
  }
}

/**
 * Fresh client from the current runtime env (Rust `runtime_client_from_env`).
 * `null` = no key/proxy configured — callers surface the honest
 * "Riot API key yapılandırılmamış" error.
 */
export function runtimeClientFromEnv(
  env: Record<string, string | undefined> = runtimeEnv(),
  fetchImpl?: RiotFetch,
): RiotClient | null {
  const proxy = nonEmpty(env.PROXY_URL);
  if (proxy) return new RiotClient("", proxy, fetchImpl);
  const key = nonEmpty(env.RIOT_API_KEY);
  return key ? new RiotClient(key, null, fetchImpl) : null;
}

// ── Endpoints (src-tauri/src/riot/endpoints/*) ────────────────────────────────

export async function getByRiotId(
  client: RiotClient,
  gameName: string,
  tagLine: string,
  region: string,
): Promise<SummonerInfo> {
  const routing = routingForRegion(region);
  const url = `https://${routing}.api.riotgames.com/riot/account/v1/accounts/by-riot-id/${encodeURIComponent(gameName)}/${encodeURIComponent(tagLine)}`;
  const dto = (await client.get(url)) as {
    puuid: string;
    gameName: string;
    tagLine: string;
  };
  return {
    puuid: dto.puuid,
    game_name: dto.gameName,
    tag_line: dto.tagLine,
    region,
  };
}

/** Match-V5 id listesi (type/queue opsiyonel — Rust build_list_ids_url paritesi). */
export function buildListIdsUrl(
  puuid: string,
  routing: string,
  matchType: string | null,
  queue: number | null,
  count: number,
): string {
  const typeParam = matchType ? `&type=${matchType}` : "";
  const queueParam = queue !== null ? `&queue=${queue}` : "";
  return `https://${routing}.api.riotgames.com/lol/match/v5/matches/by-puuid/${puuid}/ids?count=${count}${typeParam}${queueParam}`;
}

export function listMatchIds(
  client: RiotClient,
  puuid: string,
  routing: string,
  matchType: string | null,
  queue: number | null,
  count: number,
): Promise<string[]> {
  return client.get(buildListIdsUrl(puuid, routing, matchType, queue, count));
}

export function getMatchDetail(
  client: RiotClient,
  matchId: string,
  routing: string,
): Promise<Record<string, unknown>> {
  return client.get(
    `https://${routing}.api.riotgames.com/lol/match/v5/matches/${matchId}`,
  );
}

/** Match-V5 timeline (dakikalık frame'ler + event'ler) — WS4 farm@10 /
 *  erken-ölüm türevleri için. Detail ile aynı rate-limit havuzundan geçer. */
export function getMatchTimeline(
  client: RiotClient,
  matchId: string,
  routing: string,
): Promise<Record<string, unknown>> {
  return client.get(
    `https://${routing}.api.riotgames.com/lol/match/v5/matches/${matchId}/timeline`,
  );
}

export function masteryTopByPuuid(
  client: RiotClient,
  puuid: string,
  platform: string,
  count: number,
): Promise<Record<string, unknown>[]> {
  return client.get(
    `https://${platform}.api.riotgames.com/lol/champion-mastery/v4/champion-masteries/by-puuid/${puuid}/top?count=${count}`,
  );
}
