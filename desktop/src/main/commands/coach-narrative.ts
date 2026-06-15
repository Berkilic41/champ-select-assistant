// FAZ 4 / Sprint 1 — coach narrative host komutu (pluggable seam, offline-first).
//
// Bir öneri + opsiyonel FAZ 3 sinyalleri (win-prob / combo geçmişi) + opsiyonel
// dış (LLM) aday prose → core'da CoachNarrative. Dış aday core audit'ini geçerse
// o, yoksa deterministik koçluk notu. candidate yoksa (şu an) hep deterministik.
// Gelecekte yerel-ONNX/edge LLM provider yalnız `candidate` sağlar — REARCHITECTURE
// gerekmez; core her zaman audit'ler + fallback eder. Determinist motor hâlâ oracle.

import type { Engine } from "../engine";

export interface CoachNarrative {
  text: string;
  /** "deterministic" | "external". */
  source: string;
  external_rejected: boolean;
}

export interface CoachNarrativeInput {
  recommendation: unknown;
  /** Renderer rec.win_prob şekli (3A): probability 0..1 + sample_size. */
  win_prob?: { probability: number; sample_size: number } | null;
  /** Renderer rec.combo_history şekli (3C). */
  combo_history?: { games: number; wins: number } | null;
  /** Dış (LLM) aday prose — yoksa deterministik narrative. */
  candidate?: string | null;
}

export function getCoachNarrative(
  engine: Engine,
  input: CoachNarrativeInput,
): CoachNarrative {
  const wp = input.win_prob;
  const win_prob =
    wp && Number.isFinite(wp.probability)
      ? { pct: Math.round(wp.probability * 100), games: Number(wp.sample_size) || 0 }
      : undefined;
  const combo_history =
    input.combo_history && Number(input.combo_history.games) > 0
      ? { games: Number(input.combo_history.games), wins: Number(input.combo_history.wins) }
      : undefined;
  const candidate =
    typeof input.candidate === "string" && input.candidate.trim()
      ? input.candidate
      : undefined;

  return engine.coachNarrative<CoachNarrative>({
    recommendation: input.recommendation,
    win_prob,
    combo_history,
    candidate,
  });
}
