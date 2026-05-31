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

async function riotGet<T>(host: string, path: string, apiKey: string): Promise<T> {
  const resp = await fetch(`https://${host}${path}`, {
    headers: { "X-Riot-Token": apiKey, Accept: "application/json" },
  });
  if (!resp.ok) {
    throw new Error(`Riot ${resp.status} ${host}${path}`);
  }
  return (await resp.json()) as T;
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
): Promise<string[]> {
  const host = platformHost(platform);
  const list = await riotGet<LeagueList>(
    host,
    "/lol/league/v4/challengerleagues/by-queue/RANKED_SOLO_5x5",
    apiKey,
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
): Promise<string[]> {
  return riotGet<string[]>(
    regionalHost(platform),
    `/lol/match/v5/matches/by-puuid/${puuid}/ids?queue=420&type=ranked&count=${count}`,
    apiKey,
  );
}

export interface MatchParticipant {
  championId: number;
  teamPosition: string; // TOP | JUNGLE | MIDDLE | BOTTOM | UTILITY | ""
  win: boolean;
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
): Promise<MatchDto> {
  return riotGet<MatchDto>(
    regionalHost(platform),
    `/lol/match/v5/matches/${matchId}`,
    apiKey,
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
