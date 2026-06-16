// Data Dragon / Community Dragon sync — port of src-tauri/src/ddragon/{mod,cdragon}.rs
// + commands/ddragon.rs (sync_ddragon_champions / sync_cdragon_meta /
// get_ddragon_version). Parsers are pure and injectable for tests; the fetchers
// use global fetch (public CDNs, normal TLS — pinning is only for the LCU).

import type { DatabaseSync } from "node:sqlite";

/** AppState items_cache / rune_trees_cache / ddragon version karşılığı. */
export interface DdragonCaches {
  version: string | undefined;
  items: ItemData[];
  runeTrees: RuneTree[];
}

export function emptyCaches(): DdragonCaches {
  return { version: undefined, items: [], runeTrees: [] };
}

/** core types.rs ItemData. */
export interface ItemData {
  id: number;
  name: string;
  tags: string[];
  total_gold: number;
  is_completed: boolean;
}

/** core types.rs RuneTree / RuneEntry. */
export interface RuneTree {
  id: number;
  key: string;
  slots: { id: number; key: string }[][];
}

export interface ChampionListEntry {
  id: number;
  key: string;
  name: string;
  title: string;
}

export interface CdragonChampion {
  id: number;
  alias: string;
  roles: string[];
}

// ── Pure parsers (DDragon/CDragon JSON → DTOs; Rust parity) ──────────────────

/** champion.json → list. DDragon: "key" = numeric id string, "id" = key string. */
export function parseChampionList(data: unknown): ChampionListEntry[] {
  const out: ChampionListEntry[] = [];
  const champs = (data as { data?: Record<string, Record<string, unknown>> })?.data;
  if (!champs) return out;
  for (const champ of Object.values(champs)) {
    const id = Number.parseInt(String(champ.key ?? ""), 10) || 0;
    const key = typeof champ.id === "string" ? champ.id : "";
    if (id > 0 && key !== "") {
      out.push({
        id,
        key,
        name: typeof champ.name === "string" ? champ.name : "",
        title: typeof champ.title === "string" ? champ.title : "",
      });
    }
  }
  return out;
}

/**
 * item.json → filtered completed items (fetch_items parity): purchasable,
 * Summoner's Rift (map "11"), no requiredAlly, total gold ≥ 1800;
 * is_completed = `into` empty.
 */
export function parseItems(data: unknown): ItemData[] {
  const out: ItemData[] = [];
  const items = (data as { data?: Record<string, Record<string, unknown>> })?.data;
  if (!items) return out;
  for (const [idStr, item] of Object.entries(items)) {
    const id = Number.parseInt(idStr, 10);
    if (!Number.isFinite(id)) continue;
    const gold = (item.gold ?? {}) as { total?: number; purchasable?: boolean };
    const maps = (item.maps ?? {}) as Record<string, boolean>;
    const into = Array.isArray(item.into) ? item.into : [];
    const requiredAlly = typeof item.requiredAlly === "string" ? item.requiredAlly : "";
    if (!gold.purchasable) continue;
    if (!maps["11"]) continue;
    if (requiredAlly !== "") continue;
    if ((gold.total ?? 0) < 1800) continue;
    out.push({
      id,
      name: typeof item.name === "string" ? item.name : "",
      tags: Array.isArray(item.tags) ? (item.tags as string[]) : [],
      total_gold: gold.total ?? 0,
      is_completed: into.length === 0,
    });
  }
  return out;
}

/** runesReforged.json → RuneTree[] (fetch_runes parity). */
export function parseRuneTrees(data: unknown): RuneTree[] {
  if (!Array.isArray(data)) return [];
  return data.map((t: Record<string, unknown>) => ({
    id: Number(t.id ?? 0),
    key: typeof t.key === "string" ? t.key : "",
    slots: (Array.isArray(t.slots) ? t.slots : []).map(
      (s: { runes?: { id: number; key: string }[] }) =>
        (s.runes ?? []).map((r) => ({ id: Number(r.id), key: String(r.key) })),
    ),
  }));
}

/** champion-summary.json → CdragonChampion[]; "-1" placeholder atılır. */
export function parseChampionMeta(data: unknown): CdragonChampion[] {
  if (!Array.isArray(data)) return [];
  return data
    .map((c: Record<string, unknown>) => ({
      id: Number(c.id ?? 0),
      alias: typeof c.alias === "string" ? c.alias : "",
      roles: Array.isArray(c.roles) ? (c.roles as string[]) : [],
    }))
    .filter((c) => c.id > 0);
}

/** cdragon::is_melee_from_roles paritesi. */
export function isMeleeFromRoles(roles: string[]): boolean {
  const lower = roles.map((r) => r.toLowerCase());
  return (
    lower.some((r) => r === "fighter" || r === "tank" || r === "assassin") &&
    !lower.some((r) => r === "marksman")
  );
}

// ── Fetchers (injectable for tests) ──────────────────────────────────────────

export type FetchJson = (url: string) => Promise<unknown>;

const defaultFetchJson: FetchJson = async (url) => {
  const res = await fetch(url, { signal: AbortSignal.timeout(30_000) });
  if (!res.ok) throw new Error(`${url} → HTTP ${res.status}`);
  return res.json();
};

const DDRAGON = "https://ddragon.leagueoflegends.com";
const CDRAGON_URL =
  "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/champion-summary.json";

export async function fetchChampionList(
  fetchJson: FetchJson = defaultFetchJson,
): Promise<{ version: string; champions: ChampionListEntry[] }> {
  const versions = (await fetchJson(`${DDRAGON}/api/versions.json`)) as string[];
  const version = versions?.[0];
  if (!version) throw new Error("DDragon version yok");
  const data = await fetchJson(`${DDRAGON}/cdn/${version}/data/en_US/champion.json`);
  return { version, champions: parseChampionList(data) };
}

export async function fetchItems(
  version: string,
  fetchJson: FetchJson = defaultFetchJson,
): Promise<ItemData[]> {
  return parseItems(await fetchJson(`${DDRAGON}/cdn/${version}/data/en_US/item.json`));
}

export async function fetchRunes(
  version: string,
  fetchJson: FetchJson = defaultFetchJson,
): Promise<RuneTree[]> {
  return parseRuneTrees(
    await fetchJson(`${DDRAGON}/cdn/${version}/data/en_US/runesReforged.json`),
  );
}

export async function fetchChampionMeta(
  fetchJson: FetchJson = defaultFetchJson,
): Promise<CdragonChampion[]> {
  return parseChampionMeta(await fetchJson(CDRAGON_URL));
}

// ── Commands (ddragon.rs + sync_cdragon_meta parity) ─────────────────────────

export function upsertChampion(db: DatabaseSync, c: ChampionListEntry): void {
  db.prepare(
    `INSERT OR REPLACE INTO champions (champion_id, key, name, title, cached_at)
     VALUES (?, ?, ?, ?, ?)`,
  ).run(c.id, c.key, c.name, c.title, Math.floor(Date.now() / 1000));
}

export function getDdragonVersion(caches: DdragonCaches): string {
  // İlk açılışta sync henüz bitmeden çağrılırsa `caches.version` undefined olur.
  // ESKİDEN "unknown" dönerdi → renderer bunu geçerli sürüm sanıp ikon URL'ine
  // gömerdi (.../cdn/unknown/img/...) → 403 → ikonlar baş-harfe düşerdi (paketli
  // build'de görünür; dev'de sync genelde önce yetiştiği için fark edilmezdi).
  // Servable bir fallback dön (renderer'ın src/lib/ddragon.ts varsayılanıyla aynı);
  // sync tamamlanınca caches.version canlı patch'le güncellenir.
  return caches.version ?? "14.10.1";
}

/** sync_ddragon_champions: şampiyon listesi → DB; item+rune cache paralel doldur. */
export async function syncDdragonChampions(
  db: DatabaseSync,
  caches: DdragonCaches,
  fetchJson: FetchJson = defaultFetchJson,
): Promise<number> {
  const { version, champions } = await fetchChampionList(fetchJson);
  caches.version = version;
  console.info(`DDragon versiyonu güncellendi: ${version}`);

  for (const c of champions) upsertChampion(db, c);

  const [itemsRes, runesRes] = await Promise.allSettled([
    fetchItems(version, fetchJson),
    fetchRunes(version, fetchJson),
  ]);
  if (itemsRes.status === "fulfilled") {
    caches.items = itemsRes.value;
    console.info(`Item cache güncellendi: ${itemsRes.value.length} item`);
  } else {
    console.warn(`Item cache güncellenemedi: ${itemsRes.reason}`);
  }
  if (runesRes.status === "fulfilled") {
    caches.runeTrees = runesRes.value;
    console.info(`Rune cache güncellendi: ${runesRes.value.length} tree`);
  } else {
    console.warn(`Rune cache güncellenemedi: ${runesRes.reason}`);
  }

  return champions.length;
}

/** sync_cdragon_meta: CDragon rollerini champion_meta tablosuna yaz. */
export async function syncCdragonMeta(
  db: DatabaseSync,
  fetchJson: FetchJson = defaultFetchJson,
): Promise<number> {
  const champions = await fetchChampionMeta(fetchJson);
  const upsert = db.prepare(
    `INSERT OR REPLACE INTO champion_meta
       (champion_id, roles, is_melee, attack_range, resource_type)
     VALUES (?, ?, ?, ?, NULL)`,
  );
  for (const c of champions) {
    const melee = isMeleeFromRoles(c.roles);
    upsert.run(c.id, JSON.stringify(c.roles), melee ? 1 : 0, melee ? 175 : 550);
  }
  console.info(`CDragon meta senkronize edildi: ${champions.length} şampiyon`);
  return champions.length;
}
