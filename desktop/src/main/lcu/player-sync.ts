// Pure parsers for LCU match-history / mastery / current-summoner responses +
// the champion-pool counter — TS port of src-tauri/src/lcu/player_sync.rs and
// src-tauri/src/lcu/champ_pool.rs (`compute_champion_pool`). Side-effect-free so
// they unit-test without a live League client.
//
// ⚠ The LCU match-history endpoint is NOT part of the official public Riot API;
// its shape may drift between client versions. Parsers never throw on unexpected
// shapes — unknown entries are skipped, never fabricated.

export interface ParsedLcuMatch {
  match_id: string; // "LCU_{gameId}" — Riot Match-V5 id'leriyle çakışmasın
  champion_id: number;
  position: string | null;
  win: boolean;
  kills: number;
  deaths: number;
  assists: number;
  duration_secs: number;
  queue_id: number;
  played_at: number; // unix saniye
  cs: number; // lane minions + neutral monsters
  // LCU match history timeline taşımaz → cs_at_10/deaths_pre_14 yok; yalnız
  // visionScore stats'ta mevcut (yoksa dürüst null).
  vision_score: number | null;
}

export interface ParsedLcuMastery {
  champion_id: number;
  level: number;
  points: number;
  last_play_time: number | null;
}

type Json = Record<string, unknown>;

const asObj = (v: unknown): Json => (v && typeof v === "object" ? (v as Json) : {});
const asArr = (v: unknown): unknown[] => (Array.isArray(v) ? v : []);
const num = (v: unknown, fallback = 0): number =>
  typeof v === "number" && Number.isFinite(v) ? v : fallback;

/** puuid'nin participantId'si (participantIdentities üzerinden). */
function localParticipantId(game: Json, puuid: string): number | null {
  for (const id of asArr(game.participantIdentities)) {
    const entry = asObj(id);
    const player = asObj(entry.player);
    if (player.puuid === puuid) {
      const pid = entry.participantId;
      return typeof pid === "number" ? pid : null;
    }
  }
  return null;
}

function parseSingleGame(gameRaw: unknown, puuid: string): ParsedLcuMatch | null {
  const game = asObj(gameRaw);
  const gameId = game.gameId;
  if (typeof gameId !== "number") return null;

  const participantId = localParticipantId(game, puuid);
  if (participantId === null) return null;

  const participant = asArr(game.participants)
    .map(asObj)
    .find((p) => p.participantId === participantId);
  if (!participant) return null;

  const championId = participant.championId;
  if (typeof championId !== "number") return null;

  const stats = asObj(participant.stats);
  const lane = asObj(participant.timeline).lane;
  const position =
    typeof lane === "string" && lane !== "" && lane !== "NONE" ? lane : null;

  return {
    match_id: `LCU_${gameId}`,
    champion_id: championId,
    position,
    win: stats.win === true,
    kills: num(stats.kills),
    deaths: num(stats.deaths),
    assists: num(stats.assists),
    duration_secs: num(game.gameDuration),
    queue_id: num(game.queueId),
    played_at: Math.floor(num(game.gameCreation) / 1000),
    cs: num(stats.totalMinionsKilled) + num(stats.neutralMinionsKilled),
    vision_score: typeof stats.visionScore === "number" ? stats.visionScore : null,
  };
}

/** `/lol-match-history/v1/products/lol/{puuid}/matches` → yalnız puuid'nin maçları. */
export function parseLcuMatchHistory(
  json: unknown,
  puuid: string,
): ParsedLcuMatch[] {
  const games = asArr(asObj(asObj(json).games).games);
  const result: ParsedLcuMatch[] = [];
  for (const game of games) {
    const parsed = parseSingleGame(game, puuid);
    if (parsed) result.push(parsed);
  }
  return result;
}

/** `/lol-champion-mastery/v1/local-player/champion-mastery` → tüm geçerli girdiler. */
export function parseLcuMastery(json: unknown): ParsedLcuMastery[] {
  if (!Array.isArray(json)) return [];
  const result: ParsedLcuMastery[] = [];
  for (const entry of json.map(asObj)) {
    const championId = entry.championId;
    if (typeof championId !== "number") continue;
    result.push({
      champion_id: championId,
      level: num(entry.championLevel),
      points: num(entry.championPoints),
      last_play_time:
        typeof entry.lastPlayTime === "number" ? entry.lastPlayTime : null,
    });
  }
  return result;
}

/**
 * current-summoner yanıtından (game_name, tag_line). Öncelik:
 * gameName+tagLine → displayName "Name#TAG" → displayName olduğu gibi.
 */
export function parseLcuSummonerName(summoner: unknown): {
  gameName: string;
  tagLine: string;
} {
  const s = asObj(summoner);
  const gameName = typeof s.gameName === "string" ? s.gameName.trim() : "";
  const tagLine = typeof s.tagLine === "string" ? s.tagLine.trim() : "";
  if (gameName) return { gameName, tagLine };

  const display =
    typeof s.displayName === "string" && s.displayName.trim()
      ? s.displayName.trim()
      : "Summoner";
  const idx = display.lastIndexOf("#");
  if (idx > 0) {
    return { gameName: display.slice(0, idx), tagLine: display.slice(idx + 1) };
  }
  return { gameName: display, tagLine: "" };
}

/**
 * LCU match-history → `(champion_id, count)` çiftleri, çok oynanandan aza
 * (champ_pool.rs `compute_champion_pool` paritesi). Eşleşmeyen puuid'li
 * oyunlar sessizce atlanır; championId 0 sayılmaz.
 */
export function computeChampionPool(
  historyJson: unknown,
  puuid: string,
): [number, number][] {
  const counts = new Map<number, number>();
  for (const gameRaw of asArr(asObj(asObj(historyJson).games).games)) {
    const game = asObj(gameRaw);
    const participantId = localParticipantId(game, puuid);
    if (participantId === null) continue;
    const participant = asArr(game.participants)
      .map(asObj)
      .find((p) => p.participantId === participantId);
    const championId = participant?.championId;
    if (typeof championId === "number" && championId !== 0) {
      counts.set(championId, (counts.get(championId) ?? 0) + 1);
    }
  }
  return [...counts.entries()].sort((a, b) => b[1] - a[1]);
}
