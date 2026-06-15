// FAZ 3 / Sprint 3C — co-pick combo geçmişi (display-augment).
//
// Resolved outcome'lardan (picked + allies + win) oyuncunun her (pick, ally)
// çifti için maç/galibiyet sayımını üretir. SAF görsel: determinist scoring'e
// DOKUNMAZ; HeroCard combo ipucunda "geçmişin: nM %W" satırı gösterir. Ham sayım
// (display'de Bayesian shrink yerine ham oran daha dürüst); renderer ≥2 maç
// gate'iyle tek-maç gürültüsünü eler.

import type { DatabaseSync } from "node:sqlite";

export interface ComboRecord {
  games: number;
  wins: number;
}

/** Anahtar: iki şampiyon key'inin canonical (lowercase, sıralı) "a|b" biçimi. */
export type ComboOutcomes = Record<string, ComboRecord>;

/** İki key'i yöne bağımsız tek anahtara indirger (host + renderer aynı kural). */
export function comboPairKey(a: string, b: string): string {
  return [a.toLowerCase(), b.toLowerCase()].sort().join("|");
}

/**
 * Oyuncunun co-pick combo geçmişi: her (pick, ally) çiftinin maç/galibiyet
 * sayımı. Renderer yalnız GÖRÜNEN combo çiftlerini sorgular (draft_plan.combo_with
 * → zaten bilinen combo) — burada KB filtresi gerekmez. Aynı maçta bir çift iki
 * kez sayılmaz.
 */
export function getComboOutcomes(db: DatabaseSync): ComboOutcomes {
  const keyById = new Map<number, string>();
  const champs = db
    .prepare("SELECT champion_id, key FROM champions")
    .all() as unknown as { champion_id: number; key: string }[];
  for (const c of champs) keyById.set(Number(c.champion_id), String(c.key));

  const rows = db
    .prepare(
      `SELECT context_json, win FROM recommendation_outcomes
        WHERE resolved_at IS NOT NULL AND win IS NOT NULL
        ORDER BY resolved_at DESC LIMIT 5000`,
    )
    .all() as unknown as { context_json: string; win: number }[];

  const out: ComboOutcomes = {};
  for (const r of rows) {
    let ctx: { picked?: number; allies?: unknown };
    try {
      ctx = JSON.parse(r.context_json) as { picked?: number; allies?: unknown };
    } catch {
      continue;
    }
    const pickKey = keyById.get(Number(ctx.picked));
    if (!pickKey) continue;
    const allies = Array.isArray(ctx.allies) ? ctx.allies : [];
    const won = Number(r.win) === 1;
    const seen = new Set<string>();
    for (const allyId of allies) {
      const allyKey = keyById.get(Number(allyId));
      if (!allyKey || allyKey.toLowerCase() === pickKey.toLowerCase()) continue;
      const key = comboPairKey(pickKey, allyKey);
      if (seen.has(key)) continue; // aynı maçta tekrarlı çift sayma
      seen.add(key);
      const rec = out[key] ?? { games: 0, wins: 0 };
      rec.games += 1;
      if (won) rec.wins += 1;
      out[key] = rec;
    }
  }
  return out;
}
