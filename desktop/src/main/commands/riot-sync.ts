// Riot API sync commands — port of src-tauri/src/commands/riot.rs
// (sync_riot_player / sync_match_history / sync_masteries). Orchestration only:
// fetch via the rate-limited RiotClient, upsert via repos-write. Key absent →
// the same honest "Riot API key yapılandırılmamış" error as the Rust host.

import type { DatabaseSync } from "node:sqlite";

import {
  getByRiotId,
  getMatchDetail,
  getMatchTimeline,
  listMatchIds,
  masteryTopByPuuid,
  routingForRegion,
  runtimeClientFromEnv,
  type RiotClient,
  type SummonerInfo,
  type SyncResult,
} from "../riot/client";
import {
  ensureChampion,
  insertMatch,
  recordMasterySnapshot,
  summonerRegion,
  upsertMastery,
  upsertSummoner,
  type MatchInsert,
} from "../repos-write";

function requireRiot(client?: RiotClient | null): RiotClient {
  const resolved = client ?? runtimeClientFromEnv();
  if (!resolved) throw new Error("Riot API key yapılandırılmamış");
  return resolved;
}

export async function syncRiotPlayer(
  db: DatabaseSync,
  gameName: string,
  tagLine: string,
  region: string,
  client?: RiotClient | null,
): Promise<SummonerInfo> {
  const riot = requireRiot(client);
  const info = await getByRiotId(riot, gameName, tagLine, region);
  upsertSummoner(db, info);
  return info;
}

interface MatchParticipant {
  puuid?: string;
  championId?: number;
  teamPosition?: string;
  win?: boolean;
  kills?: number;
  deaths?: number;
  assists?: number;
  totalMinionsKilled?: number;
  neutralMinionsKilled?: number;
  visionScore?: number;
}

const CS_AT_MS = 10 * 60_000; // farm@10 frame eşiği
const EARLY_DEATH_MS = 14 * 60_000; // laning fazı sınırı

/** Timeline'dan (puuid'nin participant'ı için) farm@10 + erken ölüm sayısı.
 *  Eksik/bozuk timeline veya 10 dk'dan kısa maç → dürüst null'lar. Pure —
 *  testler sentetik timeline ile çağırır. */
export function timelineStats(
  timeline: Record<string, unknown>,
  puuid: string,
): { cs_at_10: number | null; deaths_pre_14: number | null } {
  const none = { cs_at_10: null, deaths_pre_14: null };
  const meta = (timeline.metadata ?? {}) as Record<string, unknown>;
  const puuids = Array.isArray(meta.participants) ? (meta.participants as string[]) : [];
  const idx = puuids.indexOf(puuid);
  if (idx < 0) return none;
  const pid = idx + 1; // participantId 1-10

  const info = (timeline.info ?? {}) as Record<string, unknown>;
  const frames = Array.isArray(info.frames)
    ? (info.frames as Record<string, unknown>[])
    : [];
  if (frames.length === 0) return none;

  let csAt10: number | null = null;
  let deathsPre14 = 0;
  for (const frame of frames) {
    const ts = Number(frame.timestamp ?? 0);

    if (csAt10 === null && ts >= CS_AT_MS) {
      const pf = ((frame.participantFrames ?? {}) as Record<string, unknown>)[String(pid)] as
        | Record<string, unknown>
        | undefined;
      if (pf) {
        csAt10 = Number(pf.minionsKilled ?? 0) + Number(pf.jungleMinionsKilled ?? 0);
      }
    }

    for (const ev of Array.isArray(frame.events)
      ? (frame.events as Record<string, unknown>[])
      : []) {
      if (
        ev.type === "CHAMPION_KILL" &&
        Number(ev.victimId ?? 0) === pid &&
        Number(ev.timestamp ?? 0) < EARLY_DEATH_MS
      ) {
        deathsPre14 += 1;
      }
    }
  }

  return { cs_at_10: csAt10, deaths_pre_14: deathsPre14 };
}

export async function syncMatchHistory(
  db: DatabaseSync,
  puuid: string,
  count: number,
  client?: RiotClient | null,
): Promise<SyncResult> {
  const riot = requireRiot(client);
  const routing = routingForRegion(summonerRegion(db, puuid));

  const ids = await listMatchIds(
    riot,
    puuid,
    routing,
    "ranked",
    420,
    Math.min(count, 20),
  );

  // Faz 1: tüm detayları çek (Rust paritesi — DB işi sona toplanır).
  let errors = 0;
  let skipped = 0;
  const parsed: MatchInsert[] = [];
  for (const id of ids) {
    let detail: Record<string, unknown>;
    try {
      detail = await getMatchDetail(riot, id, routing);
    } catch {
      errors += 1;
      continue;
    }
    const info = (detail.info ?? {}) as Record<string, unknown>;
    const participants = Array.isArray(info.participants)
      ? (info.participants as MatchParticipant[])
      : [];
    const p = participants.find((x) => x.puuid === puuid);
    if (!p) {
      skipped += 1;
      continue;
    }
    const metadata = (detail.metadata ?? {}) as Record<string, unknown>;

    // Timeline best-effort (+1 istek/maç, aynı rate-limit havuzu): hata/eksikte
    // alanlar null kalır — rapor null'u atlar, sync başarısı etkilenmez.
    let tl: { cs_at_10: number | null; deaths_pre_14: number | null } = {
      cs_at_10: null,
      deaths_pre_14: null,
    };
    try {
      tl = timelineStats(await getMatchTimeline(riot, id, routing), puuid);
    } catch {
      /* timeline best-effort */
    }

    parsed.push({
      match_id: typeof metadata.matchId === "string" ? metadata.matchId : id,
      champion_id: Number(p.championId ?? 0),
      position: typeof p.teamPosition === "string" ? p.teamPosition : null,
      win: Boolean(p.win),
      kills: Number(p.kills ?? 0),
      deaths: Number(p.deaths ?? 0),
      assists: Number(p.assists ?? 0),
      duration_secs: Number(info.gameDuration ?? 0),
      queue_id: Number(info.queueId ?? 0),
      played_at: Math.floor(Number(info.gameStartTimestamp ?? 0) / 1000),
      // CS = lane minions + neutral monsters (standart Match-V5 alanları).
      cs: Number(p.totalMinionsKilled ?? 0) + Number(p.neutralMinionsKilled ?? 0),
      cs_at_10: tl.cs_at_10,
      deaths_pre_14: tl.deaths_pre_14,
      vision_score: typeof p.visionScore === "number" ? p.visionScore : null,
    });
  }

  // Faz 2: tüm insert'ler.
  let synced = 0;
  for (const m of parsed) {
    ensureChampion(db, m.champion_id);
    if (insertMatch(db, puuid, m)) synced += 1;
    else skipped += 1;
  }

  return { synced, skipped, errors };
}

export async function syncMasteries(
  db: DatabaseSync,
  puuid: string,
  client?: RiotClient | null,
): Promise<SyncResult> {
  const riot = requireRiot(client);
  const platform = summonerRegion(db, puuid);
  const entries = await masteryTopByPuuid(riot, puuid, platform, 20);

  let synced = 0;
  let errors = 0;
  const now = Math.floor(Date.now() / 1000);
  for (const entry of entries) {
    const championId = Number(entry.championId ?? 0);
    const level = Number(entry.championLevel ?? 0);
    const points = Number(entry.championPoints ?? 0);
    const lastPlay =
      typeof entry.lastPlayTime === "number" ? entry.lastPlayTime : null;

    ensureChampion(db, championId);
    try {
      upsertMastery(db, puuid, championId, level, points, lastPlay);
      synced += 1;
    } catch {
      errors += 1;
    }
    // İlerleme takibi: puan değişince snapshot (training mode) — hata yutulur.
    try {
      recordMasterySnapshot(db, puuid, championId, level, points, now);
    } catch {
      /* snapshot kritik değil */
    }
  }

  return { synced, skipped: 0, errors };
}
