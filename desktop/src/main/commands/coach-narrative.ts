// FAZ 4 / Sprint 1 — coach narrative host komutu (pluggable seam, offline-first).
//
// Bir öneri + opsiyonel FAZ 3 sinyalleri (win-prob / combo geçmişi) + opsiyonel
// dış (LLM) aday prose → core'da CoachNarrative. Dış aday core audit'ini geçerse
// o, yoksa deterministik koçluk notu. candidate yoksa (şu an) hep deterministik.
// Gelecekte yerel-ONNX/edge LLM provider yalnız `candidate` sağlar — REARCHITECTURE
// gerekmez; core her zaman audit'ler + fallback eder. Determinist motor hâlâ oracle.

import type { DatabaseSync } from "node:sqlite";

import type { Engine } from "../engine";
import { getSettings } from "./settings";
import {
  fetchLlmCandidate,
  type CoachRecFacts,
  type FetchFn,
} from "./llm-narrator";

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

export async function getCoachNarrative(
  engine: Engine,
  db: DatabaseSync,
  input: CoachNarrativeInput,
  fetchFn?: FetchFn,
): Promise<CoachNarrative> {
  const wp = input.win_prob;
  const win_prob =
    wp && Number.isFinite(wp.probability)
      ? { pct: Math.round(wp.probability * 100), games: Number(wp.sample_size) || 0 }
      : undefined;
  const combo_history =
    input.combo_history && Number(input.combo_history.games) > 0
      ? { games: Number(input.combo_history.games), wins: Number(input.combo_history.wins) }
      : undefined;

  let candidate =
    typeof input.candidate === "string" && input.candidate.trim()
      ? input.candidate
      : undefined;

  // Aday verilmediyse + bir LLM endpoint yapılandırıldıysa (varsayılan boş=KAPALI),
  // adayı oradan çek. Endpoint yerel (Ollama) ise veri makineden çıkmaz. Aday yine
  // core audit'inden geçer; hata/timeout → null → deterministik.
  if (!candidate) {
    const settings = getSettings(db);
    if (settings.coach_llm_endpoint.trim()) {
      candidate =
        (await fetchLlmCandidate(
          settings.coach_llm_endpoint,
          settings.coach_llm_model,
          input.recommendation as CoachRecFacts,
          { win_prob: input.win_prob, combo_history: input.combo_history },
          fetchFn ?? (globalThis.fetch as unknown as FetchFn),
        )) ?? undefined;
    }
  }

  const narrative = engine.coachNarrative<CoachNarrative>({
    recommendation: input.recommendation,
    win_prob,
    combo_history,
    candidate,
  });
  // V3b: koçluk notunun kaynağı (deterministik mi LLM mi; LLM adayı reddedildi mi).
  console.info(
    `[pipeline] coach narrative source=${narrative.source}` +
      (narrative.external_rejected ? " (external_rejected)" : ""),
  );
  return narrative;
}
