// Port of get_champions (src-tauri/src/commands/ddragon.rs → champion_repo::list_all).

import type { DatabaseSync } from "node:sqlite";

export interface ChampionRecord {
  champion_id: number;
  key: string;
  name: string;
  title: string;
}

export function getChampions(db: DatabaseSync): ChampionRecord[] {
  const rows = db
    .prepare(
      "SELECT champion_id, key, name, title FROM champions ORDER BY name ASC",
    )
    .all() as unknown as ChampionRecord[];
  return rows.map((r) => ({
    champion_id: Number(r.champion_id),
    key: r.key,
    name: r.name,
    title: r.title,
  }));
}
