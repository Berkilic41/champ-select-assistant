// Minimal Riot API client for server-side aggregation.
// The API key lives only here (Worker secret), never in the desktop client.

/** Platform routing value (e.g. "tr1") → regional routing host used by Match-V5. */
const PLATFORM_TO_REGIONAL: Record<string, string> = {
  na1: "americas", br1: "americas", la1: "americas", la2: "americas",
  euw1: "europe", eune1: "europe", tr1: "europe", ru: "europe",
  kr: "asia", jp1: "asia",
  oc1: "sea", ph2: "sea", sg2: "sea", th2: "sea", tw2: "sea", vn2: "sea",
};

export function regionalHost(platform: string): string {
  const region = PLATFORM_TO_REGIONAL[platform.toLowerCase()] ?? "europe";
  return `${region}.api.riotgames.com`;
}

export function platformHost(platform: string): string {
  return `${platform.toLowerCase()}.api.riotgames.com`;
}

/** Test seams: inject a fake fetch/sleep so backoff is unit-testable without a
 *  real network or real wall-clock delays. Defaults hit the live Riot API. */
export interface RiotOpts {
  fetchImpl?: typeof fetch;
  sleepImpl?: (ms: number) => Promise<void>;
  /** Total attempts on a 429 / 5xx before giving up (default 4). */
  maxAttempts?: number;
}

const defaultSleep = (ms: number): Promise<void> =>
  new Promise((r) => setTimeout(r, ms));

/** Backoff for a retryable status: honour Retry-After (seconds) when present,
 *  else exponential 0.5s·2^n capped at 8s. Pure → directly unit-tested. */
export function backoffMs(retryAfterHeader: string | null, attempt: number): number {
  const retryAfter = Number(retryAfterHeader);
  if (Number.isFinite(retryAfter) && retryAfter > 0) {
    return Math.min(retryAfter * 1000, 30_000);
  }
  return Math.min(500 * 2 ** attempt, 8_000);
}

const isRetryable = (status: number): boolean => status === 429 || status >= 500;

async function riotGet<T>(
  host: string,
  path: string,
  apiKey: string,
  opts: RiotOpts = {},
): Promise<T> {
  const fetchImpl = opts.fetchImpl ?? fetch;
  const sleepImpl = opts.sleepImpl ?? defaultSleep;
  const maxAttempts = opts.maxAttempts ?? 4;

  for (let attempt = 0; ; attempt++) {
    const resp = await fetchImpl(`https://${host}${path}`, {
      headers: { "X-Riot-Token": apiKey, Accept: "application/json" },
    });
    if (resp.ok) return (await resp.json()) as T;

    // Retry 429 (rate limit) + 5xx (transient) with backoff; surface 4xx at once.
    if (isRetryable(resp.status) && attempt < maxAttempts - 1) {
      const jitter = Math.floor(Math.random() * 250);
      await sleepImpl(backoffMs(resp.headers.get("Retry-After"), attempt) + jitter);
      continue;
    }
    throw new Error(`Riot ${resp.status} ${host}${path}`);
  }
}

interface LeagueEntry {
  puuid?: string;
  summonerId?: string;
}
interface LeagueList {
  entries: LeagueEntry[];
}

/** Challenger Solo/Duo PUUIDs for a platform (resolves via Summoner-V4 if needed). */
export async function challengerPuuids(
  platform: string,
  apiKey: string,
  limit: number,
  opts: RiotOpts = {},
): Promise<string[]> {
  const host = platformHost(platform);
  const list = await riotGet<LeagueList>(
    host,
    "/lol/league/v4/challengerleagues/by-queue/RANKED_SOLO_5x5",
    apiKey,
    opts,
  );
  const entries = list.entries.slice(0, limit);
  const puuids: string[] = [];
  for (const e of entries) {
    if (e.puuid) {
      puuids.push(e.puuid);
    } else if (e.summonerId) {
      try {
        const s = await riotGet<{ puuid: string }>(
          host,
          `/lol/summoner/v4/summoners/${e.summonerId}`,
          apiKey,
          opts,
        );
        puuids.push(s.puuid);
      } catch {
        /* skip unresolved */
      }
    }
  }
  return puuids;
}

/** Recent ranked Solo/Duo (queue 420) match ids for a PUUID. */
export async function recentRankedMatchIds(
  platform: string,
  puuid: string,
  apiKey: string,
  count: number,
  opts: RiotOpts = {},
): Promise<string[]> {
  return riotGet<string[]>(
    regionalHost(platform),
    `/lol/match/v5/matches/by-puuid/${puuid}/ids?queue=420&type=ranked&count=${count}`,
    apiKey,
    opts,
  );
}

export interface MatchParticipant {
  championId: number;
  teamId: number; // 100 (blue) | 200 (red) — used to pair lane opponents
  teamPosition: string; // TOP | JUNGLE | MIDDLE | BOTTOM | UTILITY | ""
  win: boolean;
  // Build aggregation (Phase 3) — final item slots (item6 = trinket, excluded),
  // summoner spells and the rune page. All optional: older cached fixtures may
  // omit them, and ingest treats missing as "no build data".
  item0?: number;
  item1?: number;
  item2?: number;
  item3?: number;
  item4?: number;
  item5?: number;
  summoner1Id?: number;
  summoner2Id?: number;
  perks?: {
    styles?: {
      style?: number;
      selections?: { perk?: number }[];
    }[];
  };
}
export interface MatchInfo {
  gameVersion: string; // e.g. "16.11.581.0000"
  participants: MatchParticipant[];
  teams: { bans: { championId: number }[] }[];
}
export interface MatchDto {
  info: MatchInfo;
}

export async function matchDetail(
  platform: string,
  matchId: string,
  apiKey: string,
  opts: RiotOpts = {},
): Promise<MatchDto> {
  return riotGet<MatchDto>(
    regionalHost(platform),
    `/lol/match/v5/matches/${matchId}`,
    apiKey,
    opts,
  );
}

/** "16.11.581.0000" → "16.11" (the patch bucket we aggregate under). */
export function patchFromGameVersion(gameVersion: string): string {
  const parts = gameVersion.split(".");
  return parts.length >= 2 ? `${parts[0]}.${parts[1]}` : gameVersion;
}

export const LCU_POSITIONS: Record<string, string> = {
  TOP: "top", JUNGLE: "jungle", MIDDLE: "middle", BOTTOM: "bottom", UTILITY: "utility",
};
