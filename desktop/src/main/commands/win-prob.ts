// FAZ 3 / Sprint 3A — kalibre win-probability host komutu.
//
// Resolved `recommendation_outcomes` etiketlerini (rec_score → gerçek win/loss)
// core kalibrasyonuna besler ve verilen öneri skorları için kalibre kazanma
// olasılığı döndürür. SAF görsel augment: determinist öneri skorunu DEĞİŞTİRMEZ
// (ayrı komut, hot recommendation yolu dokunulmadan kalır). Min-sample altında
// core `available=false` döner → renderer hiç göstermez (dürüst soğuk-başlangıç).

import type { DatabaseSync } from "node:sqlite";

import type { Engine } from "../engine";

/** core win_calibration::WinProbEstimate paritesi (ts-rs üretimi renderer'da). */
export interface WinProbEstimate {
  score: number;
  probability: number;
  confidence: string;
  sample_size: number;
}

/** core win_calibration::WinProbReport paritesi. */
export interface WinProbReport {
  available: boolean;
  base_rate: number;
  total: number;
  estimates: WinProbEstimate[];
}

/**
 * Verilen öneri skorları için kalibre kazanma olasılığı. Etiket kaynağı: yalnız
 * çözülmüş + skorlu outcome satırları (rec_score NULL = liste-dışı pick, atlanır).
 */
export function getWinProbEstimates(
  engine: Engine,
  db: DatabaseSync,
  scores: number[],
): WinProbReport {
  // Savunma sınırı: en güncel 5000 etiket (latency kuyruğunu bağlar; tipik
  // oyuncu yıllarca bunun altında kalır → kalibrasyon pratikte tüm veriyi görür).
  const rows = db
    .prepare(
      `SELECT rec_score AS score, win
         FROM recommendation_outcomes
        WHERE resolved_at IS NOT NULL AND win IS NOT NULL AND rec_score IS NOT NULL
        ORDER BY resolved_at DESC LIMIT 5000`,
    )
    .all() as unknown as { score: number; win: number }[];

  const examples = rows.map((r) => ({
    score: Number(r.score),
    won: Number(r.win) === 1,
  }));
  const queryScores = scores.map(Number).filter((s) => Number.isFinite(s));

  return engine.winProbEstimates<WinProbReport>({ examples, scores: queryScores });
}
