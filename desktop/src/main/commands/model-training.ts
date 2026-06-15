// FAZ 3 / Sprint 3B — öğrenilen ModelPack eğitimi (host tarafı).
//
// Resolved + feature'lı `recommendation_outcomes` etiketlerini (pick anındaki
// per-signal vektör → win/loss) core'a verir; core ridge-to-prior + gate ile
// ModelPack ağırlıklarını öğrenir. Sonuç (varsa) `draft_brain_packs kind='model_pack'`e
// yazılır — get_recommendations bunu MEVCUT seam'den (apply_model_score) otomatik
// tüketir. Min-sample altında core null döner → pack yazılmaz → local_rules fallback.
// Determinist motor hâlâ oracle: yalnız ağırlık katsayısı değişir, audit/fallback aynı.

import type { DatabaseSync } from "node:sqlite";

import type { Engine } from "../engine";

const MODEL_PACK_KIND = "model_pack";

export interface TrainModelResult {
  trained: boolean;
  version: string | null;
  /** Eğitime giren (feature'lı, resolved) örnek sayısı. */
  samples: number;
}

interface TrainedPack {
  version: string;
  weights: Record<string, number>;
}

/**
 * Yerel etiketlerden ModelPack'i (yeniden) eğit ve persist et. Idempotent +
 * best-effort: feature'lı resolved örnek yoksa ya da core gate'i geçmezse yazmaz.
 */
export function trainLocalModelPack(
  engine: Engine,
  db: DatabaseSync,
  nowSecs: number = Math.floor(Date.now() / 1000),
): TrainModelResult {
  // Savunma sınırı: en güncel 5000 etiket (yıllarca yeterli; gecikme kuyruğunu
  // bağlar). Recent örnekler öğrenme için zaten daha alakalı (güncel meta/skill).
  const rows = db
    .prepare(
      `SELECT context_json, win FROM recommendation_outcomes
        WHERE resolved_at IS NOT NULL AND win IS NOT NULL
        ORDER BY resolved_at DESC LIMIT 5000`,
    )
    .all() as unknown as { context_json: string; win: number }[];

  const examples: { features: unknown; won: boolean }[] = [];
  for (const r of rows) {
    let features: unknown = null;
    try {
      features = (JSON.parse(r.context_json) as { features?: unknown }).features ?? null;
    } catch {
      features = null; // bozuk context → eğitime girmez (liste-dışı pick gibi)
    }
    if (features && typeof features === "object") {
      examples.push({ features, won: Number(r.win) === 1 });
    }
  }
  if (examples.length === 0) {
    return { trained: false, version: null, samples: 0 };
  }

  const pack = engine.trainModelPack<TrainedPack | null>({ examples });
  if (!pack || typeof pack.version !== "string") {
    return { trained: false, version: null, samples: examples.length }; // gate altı → rules
  }

  // Öğrenilen pack YEREL, süresiz bir artefakttır (data_pack'in aksine TTL yok).
  // UPDATE'te expires_at'e DOKUNULMAZ → re-train'de NULL-churn / yanlış "stale"
  // damgası olmaz (mevcut değer korunur).
  db.prepare(
    `INSERT INTO draft_brain_packs (kind, version, payload_json, source, fetched_at, expires_at)
     VALUES (?, ?, ?, 'local_training', ?, NULL)
     ON CONFLICT(kind) DO UPDATE SET
       version = excluded.version, payload_json = excluded.payload_json,
       source = excluded.source, fetched_at = excluded.fetched_at`,
  ).run(MODEL_PACK_KIND, pack.version, JSON.stringify(pack), nowSecs);

  return { trained: true, version: pack.version, samples: examples.length };
}
