# Feedback Loop v1 — Audit, Design & Wiring

> Tarih: 2026-06-04 (Sprint A/B Claude) → güncelleme 2026-06-05 (Sprint C Claude) · Sahip: Claude (saf
> brain + contract + observability), Codex (canlı wiring). Privacy-first + veri uydurma yok: feedback bir
> **personalization nudge**, sıralama sürücüsü değil.
>
> **✅ Döngü KAPALI (Codex wiring, 2026-06-05):** feedback artık write-only değil — champ-select'te
> `recommendation_feedback` okunuyor, `aggregate_feedback` ile sinyale dönüşüyor, `ScoringContext.feedback_signals`
> üzerinden saf engine'e giriyor, `apply_feedback` ile bounded nudge uygulanıyor; HeroCard canonical verdict
> gönderiyor. Hot dosyalara Claude dokunmadı (engine.rs/scoring.rs/commands/champ_select.rs/HeroCard.tsx Codex'in).

## 1. Mevcut altyapı (envanter)

| Parça | Yer | Durum |
|---|---|---|
| Tablo `recommendation_feedback` | `migrations/V012__draft_brain_feedback.sql` | champion_id, champion_key, **feedback (TEXT)**, session_hash, model_version, score, payload_json, synced_at, created_at |
| Komut `submit_recommendation_feedback` | `commands/draft_brain.rs:285` | satır INSERT eder, ack döner |
| Quality report | `commands/draft_brain.rs:166` | `feedback_total` + `feedback_unsynced` sayar, "N feedback cloud sync bekliyor" notu |
| Backend `/v1/recommendation-feedback` | `backend/` | cloud sync hedefi |

**~~🔴 Boşluk: feedback WRITE-ONLY~~ → ✅ ÇÖZÜLDÜ (Sprint C wiring).** Eskiden saklanıp sayılıyordu ama
okunmuyordu; artık champ-select'te okunup bounded sinyale dönüşüyor (bkz. §4 canlı wiring yolu).

## 2. v1 saf brain (uygulandı) — `recommendation/feedback_signal.rs`

Saf, IO'suz, ts-rs export'suz (frontend etkisi yok):
- `FeedbackVerdict::parse(&str)` — serbest `feedback` string'ini **savunmacı + ağırlıklı** sınıflar:
  `Helpful` (+1.0), `Picked` (**+0.5 zayıf**), `NotHelpful` (−1.0), `Skipped`/`Unknown` (0.0, non-polar).
  Sinonim/emoji kabul edilir; bilinmeyen → `Unknown` (asla polariteye tahmin yok).
- `aggregate_feedback(&[FeedbackInput]) -> HashMap<u32, FeedbackSignal>` — şampiyon başına ham pozitif/negatif
  sayar **ve ağırlıklı** `net_sentiment = Σweight/sample ∈ [-1,1]`; `confidence` (sample-size), `suggested_delta`.
- `apply_feedback(base, &signal) -> f32` — bounded nudge'i skora uygular (≥0 clamp).

### Uydurma-önleyici matematik (sabitler dosyada)
- `MAX_DELTA = 0.03` — feedback bir skoru en fazla ±0.03 oynatır (sıralama sürücüsü değil, tie-breaker).
- `MIN_SAMPLE = 3` altı → `damping = 0` → **birkaç tık skoru HİÇ oynatmaz** (anekdot kozmetik kalır).
- `FULL_WEIGHT_SAMPLE = 8`'e kadar damping lineer ramp → tam (yine küçük) ağırlık.
- **`Picked` zayıf-pozitif (0.5):** 8 picked → net 0.5 → helpful'ın yarısı kadar nudge; "seçti" ≠ "beğendi".
- Dengeli feedback (eşit +/−) → `net_sentiment 0` → delta 0.
- Fuzz testi: 0–30 × 0–30 tüm oranlar `|delta| ≤ MAX_DELTA` doğrular.

11 test: vocab parse, **verdict ağırlıkları**, **picked < helpful nudge**, low-sample kozmetik, confident +/−
cap içinde, mixed nets-out, **skipped-only→sinyal yok**, **unknown ignored**, fuzz cap.

## 3. Verdict vocabulary CONTRACT (Codex UI → command)

UI `submit_recommendation_feedback`'e **kanonik** `feedback` string'i göndermeli (parse bunları + sinonim/emoji
kabul eder; başka her şey Neutral = yoksayılır):

| feedback değeri | Anlam | Ağırlık |
|---|---|---|
| `helpful` | "İşe yaradı" | **+1.0** |
| `not_helpful` | "İşe yaramadı" | **−1.0** |
| `picked` | öneriyi seçti (zayıf olumlu) | **+0.5** |
| `skipped` | önerilen ama seçmedi | **0.0** (non-polar) |

> `skipped` bilinçli non-polar: bir öneriyi seçmemek onu kötü yapmaz (rol/ban/comfort başka sebepler).

### Verdict etki tablosu (kilitli — drift testleri kırmızı yakar)

| feedback | weight | 8× tek-şampiyon net | 8× delta | Etki |
|---|---|---|---|---|
| `helpful` | +1.0 | +1.0 | +0.03 (cap) | en güçlü olumlu nudge |
| `picked` | +0.5 | +0.5 | +0.015 | zayıf olumlu (helpful'ın yarısı) |
| `not_helpful` | −1.0 | −1.0 | −0.03 (cap) | en güçlü olumsuz nudge |
| `skipped` | 0.0 | — (non-polar) | 0 | sinyal yok, ceza yok |

> **Personalization, ranking driver DEĞİL:** `MAX_DELTA = 0.03` ve `MIN_SAMPLE=3` altı `damping=0`.
> Yani tüm feedback bir öneriyi en fazla ±0.03 oynatır; birkaç tık hiçbir şeyi kıpırdatmaz. Meta/matchup/
> comfort ana sıralamayı sürmeye devam eder — feedback yalnızca tie-break + kişiselleştirme.

### TS-safe contract (Sprint C — ÇÖZÜLDÜ)
`RecommendationFeedbackInput` ts-rs export'u `payload: Option<serde_json::Value>` yüzünden temiz türemiyordu.
Çözüm — UI'nın import ettiği **tek kaynak** `src/types/feedback.ts`:
- `FeedbackVerdict` union (`'helpful'|'not_helpful'|'picked'|'skipped'`)
- `RecommendationFeedbackPayload` (`payload?: Record<string, unknown>` = `serde_json::Value`'nun TS-safe karşılığı)
- `FeedbackQueueState` + `QueuedFeedback`
- `FEEDBACK_VERDICTS` runtime listesi `src/types/feedback-vocabulary.json`'dan (paylaşılan SoT).

**Cross-language drift guard:** aynı `feedback-vocabulary.json`'ı hem Rust (`feedback_observability.rs` testi
`include_str!` ile parse'ı doğrular) hem TS (`feedback.test.ts` union↔JSON eşler) okur. Vocabulary ayrışırsa
**bir taraf kırmızı**.

> **Codex'e opsiyonel öneri:** Rust struct'ı kendisi export etsin istenirse, payload alanına
> `#[ts(type = "Record<string, unknown> | null")]` annotation'ı ekle → `#[derive(TS)]` artık çalışır,
> generated tip `feedback.ts` union'ıyla hizalanır. (Hot dosya olduğu için Claude eklemedi.)

## 3.5 `recommendation_feedback` okuma senaryoları (command layer → `FeedbackInput`)

Wiring'de command layer satırları okuyup `Vec<FeedbackInput>`'e indirger. Senaryolar:

| Senaryo | Okuma | Sinyal sonucu |
|---|---|---|
| Hiç feedback yok | 0 satır | boş map → hiçbir öneri etkilenmez |
| Sadece `skipped`/unknown | non-polar satırlar | `is_polar()` eler → map boş |
| Tek şampiyona 1-2 polar | sample < 3 | `damping=0` → delta 0 (kozmetik) |
| Tek şampiyona ≥8 polar | sample ≥ 8 | confidence "high", delta cap'e yakın |
| Karışık +/− dengeli | net 0 | delta 0 |
| Çok şampiyon | champ-id'ye göre gruplanır | her biri bağımsız sinyal |

**Okuma stratejisi (öneri):** `SELECT champion_id, feedback FROM recommendation_feedback` (gerekirse son N
ay / `created_at` penceresi). Senkron durumdan **bağımsız** oku (synced/unsynced ikisi de kişiselleştirmede
sayılır — sync ayrı bir endişe). Champ-select sırasında **cloud çağrısı YOK**; sadece yerel DB.

## 3.6 Offline queue shape (öneri)

Feedback offline-first: önce yerel yaz (anında), online olunca `/v1/recommendation-feedback`'e flush et.
**Ek tabloya gerek yok** — `recommendation_feedback` zaten kuyruktur: `synced_at IS NULL` = bekleyen.

```ts
// src/types/feedback.ts'te kilitli (feedback.test.ts guardrail)
type FeedbackQueueState = 'pending' | 'synced' | 'failed';
interface QueuedFeedback {
  payload: RecommendationFeedbackPayload; // championId, championKey, feedback, ...
  createdAt: number;
  syncedAt: number | null;                // null = henüz flush edilmedi
  state: FeedbackQueueState;
}
```

**Queue durumları:**
| Durum | DB karşılığı | Anlam | Geçiş |
|---|---|---|---|
| `pending` | `synced_at IS NULL` | yerel yazıldı, cloud'a gönderilmedi | online → flush dene |
| `synced` | `synced_at = <ts>` | cloud'a başarıyla gönderildi | terminal |
| `failed` | `synced_at IS NULL` + son denemede hata | gönderim hata aldı, retry edilebilir | backoff → tekrar `pending` |

> `failed` DB'de ayrı kolon değil — `pending`'in son-deneme-hatalı alt-durumu (retry sayacı/last_error sync
> katmanında tutulabilir; v1 için `pending` yeterli). Flush online'da unsynced gönderir → başarıda `synced_at=now`.
> **Champ-select asla sync'e bloklanmaz.** Quality report zaten `feedback_unsynced` (synced_at IS NULL) sayıyor;
> `get_feedback_observability` `pending_sync` olarak yüzeye çıkarır (bkz. §6).

## 4. Canlı wiring yolu (✅ TAMAMLANDI — Codex, 2026-06-05)

```
recommendation_feedback (yerel DB)
   │  commands/champ_select.rs: SELECT champion_id, feedback → Vec<FeedbackInput>
   ▼
aggregate_feedback()  (feedback_signal.rs, saf)
   │  HashMap<u32, FeedbackSignal>  (bounded suggested_delta)
   ▼
ScoringContext.feedback_signals  (scoring.rs — meta_rates pattern, optional/default)
   │  saf engine map ile okur (DB lookup command'da kalır → engine saflığı korunur)
   ▼
apply_feedback(base, signal)  (engine.rs — comfort/total nudge, sadece sinyalli şampiyonlar)
   ▼
HeroCard 👍/👎  →  submit_recommendation_feedback({ feedback: 'helpful'|'not_helpful', championId, ... })
   └─ canonical verdict (feedback.ts), yeni satır INSERT → döngü başa
```

- **Champ-select sırasında cloud çağrısı YOK** — sadece yerel DB okuması.
- `meta_rates` pattern'i birebir izlendi → **engine saf kaldı** (map ile geçer, DB command'da).
- Codex regresyon testi (`tests.rs`) nudge'ın küçük + bounded kaldığını kilitliyor.
- Claude bu hot dosyalara dokunmadı; saf brain + contract + observability + doc sağladı.

## 5. Feedback observability (Sprint C — `get_feedback_observability`)

Data-quality yüzeyi için saf özet + yeni command (Claude, hot dosyasız):
- `recommendation/feedback_observability.rs` — `FeedbackObservability` (ts-rs export, tüm `number`) +
  saf `summarize_observability(rows, pending_sync)`. `feedback_signal` verdict classifier'ını reuse eder
  → polar/neutral ayrımı scoring yoluyla **asla drift etmez**.
- `commands/data_quality.rs::get_feedback_observability` — `SELECT champion_id, feedback, synced_at`
  okur, pending sayar, özetler (read-only, network yok). lib.rs handler'a kayıtlı.

| Alan | Anlam |
|---|---|
| `total` | tüm feedback satırı |
| `polar` | sinyal taşıyan (helpful/picked/not_helpful) |
| `neutral` | skipped/unknown |
| `active_champion_signals` | aggregate sonrası sinyalli şampiyon sayısı |
| `pending_sync` | `synced_at IS NULL` |

### Personalization status token (UI kalite kartı için)
`FeedbackObservability` → kanonik tek token: `personalization_status()` (saf, `feedback_observability.rs`).
ts-rs union `FeedbackPersonalizationStatus.ts` = `"no_signal" | "warming_up" | "active" | "needs_sync"`.

**Öncelik (bilinçli karar — UI'da en net model):** önce "senkron bekleyen veri var mı?", sonra
"kişiselleştirme çalışıyor mu?":

| Sıra | Koşul | Token | Anlam (kart copy yönü) |
|---|---|---|---|
| 1 | `pending_sync > 0` | `needs_sync` | yerel feedback cloud'a gönderilmeyi bekliyor |
| 2 | `active_champion_signals > 0` | `active` | kişiselleştirme sinyali çalışıyor |
| 3 | `polar > 0` | `warming_up` | sinyal birikiyor (henüz eşik altı) |
| 4 | (hiçbiri) | `no_signal` | henüz öğrenecek veri yok |

> Token Rust'ta tek kaynak (öncelik mantığı testlerle kilitli); UI sadece switch'ler. Codex UI turunda
> komuta bağlar (örn. `get_feedback_observability` yanıtını `personalization_status` ile zenginleştirir) +
> i18n copy'sini bu 4 token'a map'ler. Şu an `#[allow(dead_code)]` (bağlanana dek), TS tipi + testler hazır.

### Contract guard (Sprint C — #1)
`src/types/feedback-observability.contract.test.ts`: `FeedbackObservability`'nin **tam 5 sayaç** olduğunu
(`keyof` exhaustive) ve hepsinin **`number`** kaldığını (i64→bigint olursa compile kırılır) + status union'ın
**tam 4 token** olduğunu kilitler. Rust alanları drift ederse `pnpm typecheck`/test **kırmızı**.

## 6. Durum (Sprint C)
- Saf brain (ağırlıklı verdict) + observability summarizer + TS-safe contract (`feedback.ts` + vocabulary JSON)
  + cross-language drift guard + bu doc. **Claude hot dosyalara dokunmadı; davranış değişikliği Codex wiring'inde.**
- Baseline: cargo test **332** · clippy `-D warnings` **0** · fmt temiz · pnpm typecheck pass · vitest **143/33**.
- Privacy: tümü yerel/saf, telemetri yok. No-fabrication: bounded ±0.03 + sample-damped + picked zayıf-ağırlık.

### İleride (v1+)
- Recency ağırlığı (son feedback daha çok sayar) — v1'de ham sayım, dürüstlük için yeterli.
- Context-aware sinyal (rol/patch bazında) — şu an şampiyon-bazlı; tablo session_hash/payload_json taşıyor.
- `failed` queue alt-durumu için retry sayacı/last_error (sync katmanı; v1'de `pending` yeterli).
