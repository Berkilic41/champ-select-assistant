// Aggregate veri kaynakları — port of src-tauri/src/meta/ugg.rs +
// meta/leaguepedia.rs + commands/data_quality.rs sync_ugg_inner /
// sync_leaguepedia_inner. Scheduler'ın u_gg / leaguepedia koşucuları.
//
// u.gg: stats2.u.gg açık CDN'inden şampiyon başına overview (rate+build) +
// matchups JSON'u; region 12 (world) / rank 8 (overall). Düşük örneklem
// gürültüsü atılır (MIN_ROLE_MATCHES / MATCHUP_GAMES_FLOOR — Rust verbatim).
//
// Leaguepedia: tek kibar Cargo isteği (maxlag + tanımlayıcı UA) →
// PicksAndBansS7 draft satırları → şampiyon bazında pro presence (pick%+ban%),
// sentetik "pro" pozisyonu + "leaguepedia" kaynağıyla yazılır (ranked rol
// blend'ine ASLA karışmaz — rol sorguları "pro" istemez).

import type { DatabaseSync } from "node:sqlite";

import type { DdragonCaches, FetchJson } from "./ddragon";
import { normalizePatch, upsertCanonicalRows } from "./match-v5";

// ── u.gg sabitleri (ugg.rs verbatim) ──────────────────────────────────────────

const UGG_STATS_BASE = "https://stats2.u.gg/lol/1.5";
/** u.gg path'ine gömülü iç veri-format sürümü (dönerse 404 → dürüst hata). */
const UGG_DATA_VERSION = "1.5.0";
/** A2 spike bulgusu (2026-06-12): stats2 CDN'i YALNIZ ranked_solo_5x5 sunuyor —
 *  normal_aram / aram / ranked_flex_sr / normal_blind_5x5 / quickplay → 403.
 *  ARAM canlı metası bu CDN'den alınamaz; ARAM koçluğu KB + Match-V5'e dayanır. */
const UGG_QUEUE = "ranked_solo_5x5";
/** Region 12 = world (tüm bölgelerin toplamı). */
const UGG_REGION_WORLD = "12";
/** Rank 8 = overall (en büyük örneklem → en sağlam win-rate). */
const UGG_RANK_OVERALL = "8";
/** CDN'e kibarlık: eşzamanlı şampiyon fetch sınırı. */
const UGG_CONCURRENCY = 6;
/** Off-role gürültüsünü at (ör. bir avuç maçlık Lee Sin "support"). */
const MIN_ROLE_MATCHES = 200;
/** Düşük örneklemli matchup çiftlerini at. */
const MATCHUP_GAMES_FLOOR = 500;
const UGG_UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120";

/** u.gg rol id → LCU pozisyonu. */
const UGG_ROLES: [string, string][] = [
  ["1", "jungle"],
  ["2", "utility"],
  ["3", "bottom"],
  ["4", "top"],
  ["5", "middle"],
];

function confidenceFromMatches(n: number): string {
  if (n < 200) return "low";
  if (n <= 2000) return "medium";
  return "high";
}

/** DDragon patch'i ("16.11" / "16.11.1") → u.gg path patch'i ("16_11"). */
export function toUggPatch(ddragonPatch: string): string {
  const [major = "", minor = ""] = ddragonPatch.split(".");
  return `${major}_${minor}`;
}

/** JSON array değerinden sayısal id listesi (sayı olmayanlar atlanır). */
function idList(v: unknown): number[] {
  if (!Array.isArray(v)) return [];
  return v.filter((x): x is number => typeof x === "number" && Number.isFinite(x));
}

function at(node: unknown, key: string | number): unknown {
  if (node === null || node === undefined) return undefined;
  if (typeof key === "number") {
    return Array.isArray(node) ? node[key] : undefined;
  }
  return typeof node === "object" && !Array.isArray(node)
    ? (node as Record<string, unknown>)[key]
    : undefined;
}

function asU64(v: unknown): number | undefined {
  return typeof v === "number" && Number.isFinite(v) && v >= 0 ? v : undefined;
}

export interface UggParseResult {
  rates: Record<string, unknown>[];
  builds: Record<string, unknown>[];
}

/** Bir şampiyonun `overview` JSON'u → canonical rate + build satırları.
 *
 *  Blok şeması (canlı payload'da doğrulandı, 2026-06-12 A1 spike'ı):
 *    d[0] = [matches, wins, primary_style, sub_style, perks[6]]
 *    d[1] = [matches, wins, [spell1, spell2]]
 *    d[3] = [matches, wins, core_items[]]
 *    d[4] = [matches, wins, seq[18], "QWE"]   ← skill max önceliği
 *    d[6] = [wins, matches]                    ← rol toplamı
 *    d[8] = [matches, wins, ["5008","5008","5001"]] ← stat shard'lar
 *
 *  rune_ids seed formatıyla AYNI kodlanır: [keystone, primary_tree_id]
 *  (eski davranış 6 perk'i olduğu gibi yazıyordu — tüketici rune_ids[1]'i
 *  ağaç id'si sandığından u_gg build'lerinde yanlış ağaç görünüyordu). */
export function parseUggOverview(
  v: unknown,
  championId: number,
  patch: string,
  regionTag: string,
): UggParseResult {
  const rates: Record<string, unknown>[] = [];
  const builds: Record<string, unknown>[] = [];
  const rankNode = at(at(v, UGG_REGION_WORLD), UGG_RANK_OVERALL);
  if (!rankNode) return { rates, builds };

  for (const [roleKey, position] of UGG_ROLES) {
    // role node = [dataArray, timestamp]; dataArray 13 elemanlı blok.
    const d = at(at(rankNode, roleKey), 0);
    if (!d) continue;

    const wins = asU64(at(at(d, 6), 0));
    const matches = asU64(at(at(d, 6), 1));
    if (wins === undefined || matches === undefined) continue;
    if (matches < MIN_ROLE_MATCHES || matches === 0) continue;

    const winRate = wins / matches;
    const confidence = confidenceFromMatches(matches);
    rates.push({
      region: regionTag,
      patch,
      champion_id: championId,
      position,
      win_rate: winRate,
      // u.gg overview'da decode edilebilir pick/ban yok — dürüst 0
      // (pick/ban'ı Meraki/Match-V5 taşır).
      pick_rate: 0,
      ban_rate: 0,
      sample_size: matches,
      source: "u_gg",
      confidence,
    });

    const itemIds = idList(at(at(d, 3), 2));
    const perks = idList(at(at(d, 0), 4));
    const primaryStyle = asU64(at(at(d, 0), 2));
    const subStyle = asU64(at(at(d, 0), 3));
    // Seed format paritesi: [keystone, primary_tree]; stil çözülemezse perk
    // listesi olduğu gibi kalır (eski davranış — veri uydurma yok).
    const runeIds =
      perks.length > 0 && primaryStyle !== undefined
        ? [perks[0], primaryStyle]
        : perks;
    // [secondary_tree, rune1, rune2] — yalnız tam 6-perk sayfası varsa.
    const secondaryRunes =
      subStyle !== undefined && perks.length >= 6
        ? [subStyle, perks[4], perks[5]]
        : [];
    const statShards = (() => {
      const raw = at(at(d, 8), 2);
      if (!Array.isArray(raw)) return [] as number[];
      return raw
        .map((x) => Number(x))
        .filter((n) => Number.isFinite(n) && n > 0);
    })();
    // "QWE" max-öncelik dizgisi → seed formatı "Q→W→E"; beklenmedik şekil →
    // null (uydurma yok).
    const orderRaw = at(at(d, 4), 3);
    const skillOrder =
      typeof orderRaw === "string" && /^[QWER]{3}$/.test(orderRaw)
        ? orderRaw.split("").join("→")
        : null;
    const summonerSpells = idList(at(at(d, 1), 2));
    if (itemIds.length > 0 || runeIds.length > 0) {
      builds.push({
        region: regionTag,
        patch,
        champion_id: championId,
        position,
        item_ids: itemIds,
        rune_ids: runeIds,
        secondary_runes: secondaryRunes,
        stat_shards: statShards,
        skill_order: skillOrder,
        summoner_spells: summonerSpells,
        games: matches,
        win_rate: winRate,
        pick_rate: 0,
        sample_size: matches,
        source: "u_gg",
        confidence,
      });
    }
  }
  return { rates, builds };
}

/** Bir şampiyonun `matchups` JSON'u → canonical matchup satırları
 *  (ugg.rs parse_matchups; satır = [opponentId, wins, games, …deltas]). */
export function parseUggMatchups(
  v: unknown,
  championId: number,
  patch: string,
  regionTag: string,
): Record<string, unknown>[] {
  const out: Record<string, unknown>[] = [];
  const rankNode = at(at(v, UGG_REGION_WORLD), UGG_RANK_OVERALL);
  if (!rankNode) return out;

  for (const [roleKey, position] of UGG_ROLES) {
    const rows = at(at(rankNode, roleKey), 0);
    if (!Array.isArray(rows)) continue;
    for (const row of rows) {
      const opponentId = asU64(at(row, 0));
      const wins = asU64(at(row, 1));
      const games = asU64(at(row, 2));
      if (opponentId === undefined || wins === undefined || games === undefined) {
        continue;
      }
      if (games < MATCHUP_GAMES_FLOOR || games === 0 || opponentId === 0) continue;
      out.push({
        region: regionTag,
        patch,
        champion_id: championId,
        opponent_id: opponentId,
        position,
        games,
        wins,
        win_rate: wins / games,
        sample_size: games,
        source: "u_gg",
        confidence: confidenceFromMatches(games),
      });
    }
  }
  return out;
}

function uggOverviewUrl(uggPatch: string, championId: number): string {
  return `${UGG_STATS_BASE}/overview/${uggPatch}/${UGG_QUEUE}/${championId}/${UGG_DATA_VERSION}.json`;
}

function uggMatchupsUrl(uggPatch: string, championId: number): string {
  return `${UGG_STATS_BASE}/matchups/${uggPatch}/${UGG_QUEUE}/${championId}/${UGG_DATA_VERSION}.json`;
}

/** Varsayılan u.gg fetch'i (15s timeout + tarayıcı-vari UA — Rust paritesi). */
export const defaultUggFetch: FetchJson = async (url) => {
  const res = await fetch(url, {
    signal: AbortSignal.timeout(15_000),
    headers: { "User-Agent": UGG_UA },
  });
  if (!res.ok) throw new Error(`HTTP ${res.status}: ${url}`);
  return res.json();
};

/** u.gg canlı patch'in 1-2 gerisinde kalabilir; seed şampiyonla mevcut patch
 *  ve iki öncesini yokla, veri sunan ilkini kullan (resolve_working_patch). */
async function resolveWorkingUggPatch(
  fetchJson: FetchJson,
  patch: string,
  seedChampionId: number,
): Promise<string> {
  const [majorRaw = "0", minorRaw = "0"] = patch.split(".");
  const major = Number.parseInt(majorRaw, 10) || 0;
  const minor = Number.parseInt(minorRaw, 10) || 0;
  for (let back = 0; back < 3; back++) {
    if (minor - back < 0) break;
    const candidate = `${major}_${minor - back}`;
    try {
      await fetchJson(uggOverviewUrl(candidate, seedChampionId));
      return candidate;
    } catch {
      /* sıradaki adayı dene */
    }
  }
  throw new Error(
    `u.gg: '${toUggPatch(patch)}' patch'i ve 2 öncesi için veri yok (CDN güncel değil veya engelli)`,
  );
}

export interface UggSyncOutcome {
  rates: number;
  builds: number;
  matchups: number;
}

/** sync_ugg_inner paritesi: roster'daki her şampiyon için overview (zorunlu) +
 *  matchups (best-effort), 6'lı eşzamanlılıkla; hiçbiri gelmezse dürüst hata. */
export async function syncUgg(
  db: DatabaseSync,
  caches: DdragonCaches,
  fetchJson: FetchJson = defaultUggFetch,
): Promise<UggSyncOutcome> {
  const champions = (
    db
      .prepare("SELECT champion_id FROM champions ORDER BY champion_id")
      .all() as unknown as { champion_id: number }[]
  ).map((c) => Number(c.champion_id));
  if (champions.length === 0) throw new Error("u.gg: boş şampiyon listesi");

  const patch = normalizePatch(caches.version ?? "");
  const regionTag = "all";
  const uggPatch = await resolveWorkingUggPatch(fetchJson, patch, champions[0]);
  // u.gg canlı patch'in 1-2 gerisinde olabilir; satırları GERÇEK kaynak patch'iyle
  // (uggPatch) etiketle — canlı patch ile değil — ki bayat veri 'güncel patch' diye
  // yazılıp patch_fresh'i yanlış true yapmasın (staleness maskelenmesin).
  const sourcePatch = uggPatch.replace("_", ".");

  const rates: Record<string, unknown>[] = [];
  const builds: Record<string, unknown>[] = [];
  const matchups: Record<string, unknown>[] = [];
  let ok = 0;

  // CONCURRENCY'li havuz: sıradaki id'yi çeken N işçi.
  let next = 0;
  const worker = async () => {
    while (next < champions.length) {
      const id = champions[next++];
      let overview: unknown;
      try {
        overview = await fetchJson(uggOverviewUrl(uggPatch, id));
      } catch {
        continue; // bu patch'te şampiyon yok → atla, diğerleri taşır
      }
      ok += 1;
      const parsed = parseUggOverview(overview, id, sourcePatch, regionTag);
      rates.push(...parsed.rates);
      builds.push(...parsed.builds);
      try {
        const mu = await fetchJson(uggMatchupsUrl(uggPatch, id));
        matchups.push(...parseUggMatchups(mu, id, sourcePatch, regionTag));
      } catch {
        /* matchups best-effort */
      }
    }
  };
  await Promise.all(
    Array.from({ length: Math.min(UGG_CONCURRENCY, champions.length) }, worker),
  );

  if (ok === 0) throw new Error("u.gg: hiçbir şampiyon verisi alınamadı");
  upsertCanonicalRows(db, { rates, matchups, builds });
  return { rates: rates.length, builds: builds.length, matchups: matchups.length };
}

// ── Leaguepedia (leaguepedia.rs + sync_leaguepedia_inner) ─────────────────────

const CARGO_URL = "https://lol.fandom.com/api.php";
const LEAGUEPEDIA_UA = "champ-select-assistant/0.9 (personal LoL draft tool)";
/** İyi bir MediaWiki vatandaşı ol — küçük sayfa, sunucu-lag duyarlı. */
const PAGE_LIMIT = 200;

const PICK_KEYS = [
  "Team1Pick1", "Team1Pick2", "Team1Pick3", "Team1Pick4", "Team1Pick5",
  "Team2Pick1", "Team2Pick2", "Team2Pick3", "Team2Pick4", "Team2Pick5",
];
const BAN_KEYS = [
  "Team1Ban1", "Team1Ban2", "Team1Ban3", "Team1Ban4", "Team1Ban5",
  "Team2Ban1", "Team2Ban2", "Team2Ban3", "Team2Ban4", "Team2Ban5",
];

/** Şampiyon görünen adını eşleşme anahtarına indirger: lowercase + yalnız
 *  alfasayısal ("Lee Sin"/"Nunu & Willump"/"Kai'Sa" özel durumsuz hizalanır). */
export function normalizeChampionName(name: string): string {
  return name.replace(/[^\p{L}\p{N}]/gu, "").toLowerCase();
}

/** PicksAndBansS7 cargoquery yanıtı → oyun başına (picks, bans) ad listeleri. */
export function parseDraftRows(v: unknown): [string[], string[]][] {
  const rows = at(v, "cargoquery");
  if (!Array.isArray(rows)) return [];
  const out: [string[], string[]][] = [];
  for (const row of rows) {
    const title = at(row, "title");
    if (!title) continue;
    const collect = (keys: string[]) =>
      keys
        .map((k) => at(title, k))
        .filter((s): s is string => typeof s === "string")
        .map((s) => s.trim())
        .filter((s) => s.length > 0);
    out.push([collect(PICK_KEYS), collect(BAN_KEYS)]);
  }
  return out;
}

export interface ProPresence {
  champion_id: number;
  pick_games: number;
  ban_games: number;
  total_games: number;
  pick_rate: number;
  ban_rate: number;
  presence: number;
}

/** Oyun başına pick/ban → şampiyon bazında pro presence (aggregate_presence
 *  birebir: oyun içinde tekrar SAYILMAZ; eşleşmeyen adlar atlanır — fab yok). */
export function aggregatePresence(
  games: [string[], string[]][],
  nameToId: Map<string, number>,
): ProPresence[] {
  const total = games.length;
  if (total === 0) return [];
  const resolve = (name: string) => nameToId.get(normalizeChampionName(name));

  const pick = new Map<number, number>();
  const ban = new Map<number, number>();
  for (const [picks, bans] of games) {
    const seen = new Set<number>();
    for (const name of picks) {
      const id = resolve(name);
      if (id !== undefined && !seen.has(id)) {
        seen.add(id);
        pick.set(id, (pick.get(id) ?? 0) + 1);
      }
    }
    seen.clear();
    for (const name of bans) {
      const id = resolve(name);
      if (id !== undefined && !seen.has(id)) {
        seen.add(id);
        ban.set(id, (ban.get(id) ?? 0) + 1);
      }
    }
  }

  const ids = [...new Set([...pick.keys(), ...ban.keys()])].sort((a, b) => a - b);
  return ids.map((championId) => {
    const pickGames = pick.get(championId) ?? 0;
    const banGames = ban.get(championId) ?? 0;
    return {
      champion_id: championId,
      pick_games: pickGames,
      ban_games: banGames,
      total_games: total,
      pick_rate: pickGames / total,
      ban_rate: banGames / total,
      presence: pickGames / total + banGames / total,
    };
  });
}

/** Kibar Cargo isteği URL'i (maxlag + alan listesi + DateTime_UTC DESC). */
export function leaguepediaUrl(offset = 0): string {
  const params = new URLSearchParams({
    action: "cargoquery",
    format: "json",
    maxlag: "5",
    tables: "PicksAndBansS7",
    fields: [...PICK_KEYS, ...BAN_KEYS].join(","),
    order_by: "DateTime_UTC DESC",
    limit: String(PAGE_LIMIT),
    offset: String(offset),
  });
  return `${CARGO_URL}?${params.toString()}`;
}

/** Varsayılan Leaguepedia fetch'i (20s timeout + tanımlayıcı UA — MediaWiki
 *  görgü kuralı). */
export const defaultLeaguepediaFetch: FetchJson = async (url) => {
  const res = await fetch(url, {
    signal: AbortSignal.timeout(20_000),
    headers: { "User-Agent": LEAGUEPEDIA_UA },
  });
  if (!res.ok) throw new Error(`HTTP ${res.status}: ${url}`);
  return res.json();
};

/** sync_leaguepedia_inner paritesi: tek kibar Cargo isteği → pro presence →
 *  sentetik "pro" pozisyon/region + "leaguepedia" kaynağıyla rate upsert'i.
 *  Yazılan şampiyon sayısını döner; API `error.code` (ratelimited/maxlag)
 *  dürüst hata olur — çağıran son-iyi cache'le devam eder. */
export async function syncLeaguepedia(
  db: DatabaseSync,
  caches: DdragonCaches,
  fetchJson: FetchJson = defaultLeaguepediaFetch,
): Promise<number> {
  const nameToId = new Map<string, number>();
  for (const row of db
    .prepare("SELECT champion_id, name FROM champions")
    .all() as unknown as { champion_id: number; name: string }[]) {
    nameToId.set(normalizeChampionName(String(row.name)), Number(row.champion_id));
  }
  const patch = normalizePatch(caches.version ?? "");

  const value = await fetchJson(leaguepediaUrl(0));
  const errCode = at(at(value, "error"), "code");
  if (typeof errCode === "string") {
    throw new Error(`Leaguepedia API error: ${errCode}`);
  }
  const games = parseDraftRows(value);
  const presence = aggregatePresence(games, nameToId);

  const rates = presence.map((p) => ({
    region: "pro",
    patch,
    champion_id: p.champion_id,
    position: "pro",
    win_rate: 0,
    pick_rate: p.pick_rate,
    ban_rate: p.ban_rate,
    sample_size: p.total_games,
    source: "leaguepedia",
    confidence: p.total_games >= 100 ? "high" : p.total_games >= 30 ? "medium" : "low",
  }));
  upsertCanonicalRows(db, { rates, matchups: [], builds: [] });
  return rates.length;
}

// ── Edge rates (cloudflare-worker /v1/rates — P2 app-wiring) ──────────────────
// Worker, Match-V5 maçlarını server-side prod-key ile toplar; desktop yalnız
// aggregate sonucu çeker. Base URL `EDGE_BASE_URL` env/.env anahtarından gelir;
// yoksa kaynak scheduler planında dürüst skip_disabled kalır (Riot-key deseni).

export interface EdgeRateRow {
  champion_id: number;
  role: string;
  games: number;
  win_rate: number;
  pick_rate: number;
  ban_rate: number;
}

export interface EdgeMatchupRow {
  champion_id: number;
  opponent_id: number;
  role: string;
  games: number;
  wins: number;
  win_rate: number;
}

export interface EdgeBuildRow {
  champion_id: number;
  role: string;
  item_ids: number[];
  rune_ids: number[];
  summoner_spells: number[];
  games: number;
  wins: number;
  win_rate: number;
}

export function edgeRatesUrl(baseUrl: string, region: string): string {
  return `${baseUrl.replace(/\/+$/, "")}/v1/rates?region=${encodeURIComponent(region)}`;
}

export function edgeMatchupsUrl(baseUrl: string, region: string): string {
  return `${baseUrl.replace(/\/+$/, "")}/v1/matchups?region=${encodeURIComponent(region)}`;
}

export function edgeBuildsUrl(baseUrl: string, region: string): string {
  return `${baseUrl.replace(/\/+$/, "")}/v1/builds?region=${encodeURIComponent(region)}`;
}

/** Varsayılan edge fetch'i (15s timeout). */
export const defaultEdgeFetch: FetchJson = async (url) => {
  const res = await fetch(url, { signal: AbortSignal.timeout(15_000) });
  if (!res.ok) throw new Error(`HTTP ${res.status}: ${url}`);
  return res.json();
};

/** Edge worker'dan aggregate rate + matchup + build'leri çek ve canonical
 *  upsert et. Bölge aktif summoner'dan (en yeni cached_at), yoksa "tr1". Patch
 *  worker yanıtından gelir (worker en son gördüğü patch'i çözer). Rates
 *  zorunlu, matchups/builds best-effort (u.gg deseni). Satır sayılarını döner. */
export async function syncEdgeRates(
  db: DatabaseSync,
  baseUrl: string,
  fetchJson: FetchJson = defaultEdgeFetch,
): Promise<{ rates: number; matchups: number; builds: number }> {
  const summoner = db
    .prepare("SELECT region FROM summoners ORDER BY cached_at DESC LIMIT 1")
    .get() as unknown as { region?: string } | undefined;
  const region =
    typeof summoner?.region === "string" && summoner.region ? summoner.region : "tr1";

  const resp = (await fetchJson(edgeRatesUrl(baseUrl, region))) as {
    patch?: string;
    region?: string;
    rates?: EdgeRateRow[];
  };
  if (!resp || !Array.isArray(resp.rates) || typeof resp.patch !== "string") {
    throw new Error("edge: geçersiz /v1/rates yanıtı");
  }

  const rates = resp.rates
    .filter((r) => Number(r.games) > 0 && Number(r.champion_id) > 0)
    .map((r) => ({
      region: resp.region ?? region,
      patch: resp.patch as string,
      champion_id: Number(r.champion_id),
      position: String(r.role),
      win_rate: Number(r.win_rate),
      pick_rate: Number(r.pick_rate),
      ban_rate: Number(r.ban_rate),
      sample_size: Number(r.games),
      source: "cloud_edge",
      confidence: confidenceFromMatches(Number(r.games)),
    }));

  // Matchups best-effort: endpoint/tablo henüz boşsa veya hata dönerse rates
  // yine de yazılır (u.gg sync'indeki matchup deseniyle aynı dürüstlük).
  let matchups: Record<string, unknown>[] = [];
  try {
    const mu = (await fetchJson(edgeMatchupsUrl(baseUrl, region))) as {
      patch?: string;
      matchups?: EdgeMatchupRow[];
    };
    if (mu && Array.isArray(mu.matchups)) {
      matchups = mu.matchups
        .filter(
          (m) =>
            Number(m.games) > 0 && Number(m.champion_id) > 0 && Number(m.opponent_id) > 0,
        )
        .map((m) => ({
          region: resp.region ?? region,
          patch: typeof mu.patch === "string" && mu.patch ? mu.patch : (resp.patch as string),
          champion_id: Number(m.champion_id),
          opponent_id: Number(m.opponent_id),
          position: String(m.role),
          games: Number(m.games),
          wins: Number(m.wins),
          win_rate: Number(m.win_rate),
          sample_size: Number(m.games),
          source: "cloud_edge",
          confidence: confidenceFromMatches(Number(m.games)),
        }));
    }
  } catch {
    /* matchups best-effort */
  }

  // Builds best-effort: en popüler loadout (champ, rol) başına tek satır.
  let builds: Record<string, unknown>[] = [];
  try {
    const bu = (await fetchJson(edgeBuildsUrl(baseUrl, region))) as {
      patch?: string;
      builds?: EdgeBuildRow[];
    };
    if (bu && Array.isArray(bu.builds)) {
      builds = bu.builds
        .filter(
          (b) =>
            Number(b.champion_id) > 0 &&
            Number(b.games) > 0 &&
            (Array.isArray(b.item_ids) ? b.item_ids.length : 0) +
              (Array.isArray(b.rune_ids) ? b.rune_ids.length : 0) >
              0,
        )
        .map((b) => ({
          region: resp.region ?? region,
          patch: typeof bu.patch === "string" && bu.patch ? bu.patch : (resp.patch as string),
          champion_id: Number(b.champion_id),
          position: String(b.role),
          item_ids: (b.item_ids ?? []).map(Number).filter((n) => n > 0),
          rune_ids: (b.rune_ids ?? []).map(Number).filter((n) => n > 0),
          summoner_spells: (b.summoner_spells ?? []).map(Number).filter((n) => n > 0),
          games: Number(b.games),
          win_rate: Number(b.win_rate),
          pick_rate: 0,
          sample_size: Number(b.games),
          source: "cloud_edge",
          confidence: confidenceFromMatches(Number(b.games)),
        }));
    }
  } catch {
    /* builds best-effort */
  }

  upsertCanonicalRows(db, { rates, matchups, builds });
  return { rates: rates.length, matchups: matchups.length, builds: builds.length };
}
