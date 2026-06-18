// FAZ 4 / LLM provider — opsiyonel LLM koçluk anlatıcısı (offline-capable, pluggable).
//
// Yapılandırılmış bir OpenAI-uyumlu chat endpoint'i (ör. YEREL Ollama
// http://localhost:11434/v1/chat/completions → veri makineden ÇIKMAZ) için
// grounded bir prompt kurar ve aday prose çeker. Aday core audit'inden
// (over-promising + grounding) geçer; geçmezse deterministik fallback.
// Hata/timeout/boş yanıt → null → deterministik. Determinist motor hâlâ oracle;
// LLM yalnız aynı gerçekleri akıcı anlatır, yeni iddia ekleyemez (core reddeder).

export interface CoachFaz3 {
  win_prob?: { probability: number; sample_size: number } | null;
  combo_history?: { games: number; wins: number } | null;
}

/** Prompt için rec'ten okunan grounded alanlar (core Recommendation alt kümesi). */
export interface CoachRecFacts {
  champion_name?: string;
  headline?: string | null;
  decision_sentence?: string;
  reason?: string;
  lane_opponent_name?: string | null;
  core_item_name?: string | null;
  risk_summary?: string | null;
  draft_plan?: { combo_with?: { ally_champion_key?: string }[] } | null;
  /** Rakip kompo özeti (ör. "AP ağırlıklı · frontline yok") — nota bağlam verir. */
  enemy_team_summary?: string;
  /** [erken, orta, geç] faz avantajı 0..1 (0.5 = nötr). */
  phase_matchup?: [number, number, number] | null;
  /** Gerçek verisi olmayan sinyaller ('meta'|'matchup'|'build') — LLM bunlar
   *  hakkında iddia ETMEMELİ (anti-halüsinasyon grounding). */
  missing_signals?: string[];
}

/** Test seam: global fetch ile yapısal uyumlu. */
export type FetchFn = (
  url: string,
  init: { method: string; headers: Record<string, string>; body: string; signal?: AbortSignal },
) => Promise<{ ok: boolean; json: () => Promise<unknown> }>;

const SYSTEM_PROMPT =
  "Sen bir League of Legends draft koçusun. SADECE sana verilen gerçeklere dayan; " +
  "yeni şampiyon, sayı ya da iddia UYDURMA. 'Kesin', 'garanti', 'her zaman', 'asla' " +
  "gibi mutlak dil KULLANMA. Türkçe, en fazla 3 kısa cümle, net ve sakin bir koçluk " +
  "notu yaz. Notta şampiyonun adını mutlaka geçir.";

export function buildCoachUserPrompt(
  rec: CoachRecFacts,
  faz3: CoachFaz3,
  vary = false,
): string {
  const facts: string[] = [];
  const champ = (rec.champion_name ?? "").trim() || "Bu pick";
  facts.push(`Şampiyon: ${champ}`);
  const drivers =
    (rec.headline ?? "").trim() ||
    (rec.decision_sentence ?? "").trim() ||
    (rec.reason ?? "").trim();
  if (drivers) facts.push(`Öne çıkan: ${drivers}`);
  if (rec.lane_opponent_name?.trim()) facts.push(`Lane rakibi: ${rec.lane_opponent_name.trim()}`);
  const ally = rec.draft_plan?.combo_with?.[0]?.ally_champion_key;
  if (ally) facts.push(`Combo müttefiki: ${ally}`);
  if (rec.core_item_name?.trim()) facts.push(`İlk item: ${rec.core_item_name.trim()}`);
  if (rec.risk_summary?.trim()) facts.push(`Risk: ${rec.risk_summary.trim()}`);
  if (rec.enemy_team_summary?.trim()) facts.push(`Rakip kompo: ${rec.enemy_team_summary.trim()}`);
  const pm = rec.phase_matchup;
  if (pm && pm.length === 3) {
    const p = (v: number) => Math.round(Math.max(0, Math.min(1, v)) * 100);
    facts.push(`Faz avantajın: erken ~%${p(pm[0])} · orta ~%${p(pm[1])} · geç ~%${p(pm[2])}`);
  }
  const wp = faz3.win_prob;
  if (wp && Number.isFinite(wp.probability)) {
    facts.push(
      `Bu skor seviyesinde geçmiş kazanma: ~%${Math.round(wp.probability * 100)} (${wp.sample_size} maç)`,
    );
  }
  const ch = faz3.combo_history;
  if (ch && ch.games > 0) {
    facts.push(`Bu combo'da co-pick geçmişi: ${ch.games} maç, ${ch.wins} galibiyet`);
  }
  // Anti-halüsinasyon: gerçek verisi olmayan sinyalleri LLM'e bildir → uydurmasın.
  const miss = rec.missing_signals?.filter((s) => s.trim());
  if (miss && miss.length > 0) {
    facts.push(`Veri boşluğu (bunlar hakkında iddia ETME): ${miss.join(", ")}`);
  }
  // vary: kullanıcı notu "yeniden üret"tiyse (örtük beğenmedim) → farklı bir not iste.
  const closing = vary
    ? "Bu gerçeklere sadık, kısa bir koçluk notu yaz — öncekinden FARKLI bir açıdan, farklı kelimelerle."
    : "Bu gerçeklere sadık, kısa bir koçluk notu yaz.";
  return `Gerçekler:\n${facts.join("\n")}\n\n${closing}`;
}

/**
 * Yapılandırılmış endpoint'ten aday koçluk prose'u çek. endpoint boşsa ya da
 * herhangi bir hata/timeout/boş yanıtta null → çağıran deterministik'e düşer.
 */
export async function fetchLlmCandidate(
  endpoint: string,
  model: string,
  rec: CoachRecFacts,
  faz3: CoachFaz3,
  fetchFn: FetchFn,
  timeoutMs = 6000,
  vary = false,
): Promise<string | null> {
  const url = endpoint.trim();
  if (!url) return null;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const res = await fetchFn(url, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        model: model.trim() || "default",
        messages: [
          { role: "system", content: SYSTEM_PROMPT },
          { role: "user", content: buildCoachUserPrompt(rec, faz3, vary) },
        ],
        temperature: 0.4,
        max_tokens: 180,
        stream: false,
      }),
      signal: controller.signal,
    });
    if (!res.ok) return null;
    const json = (await res.json()) as {
      choices?: { message?: { content?: unknown } }[];
    };
    const text = json?.choices?.[0]?.message?.content;
    return typeof text === "string" && text.trim() ? text.trim() : null;
  } catch {
    return null; // hata/timeout → deterministik fallback
  } finally {
    clearTimeout(timer);
  }
}

/** Bağlantı testi sonucu. `reason` hata türü (i18n/log için), ok=true ise yok. */
export interface LlmTestResult {
  ok: boolean;
  /** 'empty' | 'http' | 'network' | 'bad_response' */
  reason?: string;
}

/**
 * LLM endpoint'ine MİNİMAL bir "ping" isteği atıp ulaşılabilirliği + OpenAI-uyumlu
 * yanıt biçimini doğrular. Kullanıcı/oyun verisi GÖNDERMEZ (yalnız "ping", max_tokens 1).
 * Kullanıcı endpoint'i girdikten sonra champ-select'i beklemeden kurulumu doğrular.
 */
export async function testCoachLlm(
  endpoint: string,
  model: string,
  fetchFn: FetchFn = globalThis.fetch as unknown as FetchFn,
  timeoutMs = 6000,
): Promise<LlmTestResult> {
  const url = endpoint.trim();
  if (!url) return { ok: false, reason: "empty" };
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const res = await fetchFn(url, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        model: model.trim() || "default",
        messages: [{ role: "user", content: "ping" }],
        max_tokens: 1,
        stream: false,
      }),
      signal: controller.signal,
    });
    if (!res.ok) return { ok: false, reason: "http" };
    const json = (await res.json()) as { choices?: unknown };
    if (Array.isArray(json?.choices)) return { ok: true };
    return { ok: false, reason: "bad_response" };
  } catch {
    return { ok: false, reason: "network" }; // ağ/timeout/abort
  } finally {
    clearTimeout(timer);
  }
}
