# Feedback Sync + Analytics — Audit & Design (Sprint D)

> Tarih: 2026-06-05 · Sahip: Claude (2. mühendis) · Kapsam: yerel feedback kuyruğunun sync-state'i,
> backend ingestion kontratı, ve read-only feedback analytics. **Hot dosyalara dokunulmadı**
> (HeroCard/DataStatusBadges/ChampSelectWrapper.tsx, engine.rs, scoring.rs, commands/champ_select.rs).
> Privacy-first + veri uydurma yok. Tamamlayıcı: [feedback-loop-v1.md](feedback-loop-v1.md).

## 1. Local queue sync-state audit (A)

**Mevcut durum:** `submit_recommendation_feedback` satırı `synced_at IS NULL` ile yazıyor; **client-side
flush YOK** → her satır kalıcı `pending`. Idempotency anahtarı yok → aynı satır iki kez gönderilirse backend
**duplicate** üretir (her POST yeni UUID).

**Saf politika (uygulandı) — `recommendation/feedback_sync.rs`:**
| Helper | Davranış |
|---|---|
| `retry_backoff_secs(attempt)` | 30 → 60 → 120 … `RETRY_MAX_SECS=3600` cap (exponential, overflow-safe) |
| `next_retry_at(now, attempt)` | `now + backoff` |
| `should_retry(attempt)` | `attempt < MAX_ATTEMPTS(6)` → sonra terminal `failed` |
| `idempotency_key(user_hash, champion_id, verdict, created_at)` | deterministik FNV-1a hex; **duplicate ingestion'ı önler** |

**Sync-state makinesi:**
```
pending ──(online, flush dene)──▶ synced        (synced_at = now)
   ▲                                  
   └──(hata, should_retry)── failed ──(backoff geçti)── pending
                              │
                              └──(attempt ≥ MAX_ATTEMPTS)── terminal-failed (manuel/sonraki oturum)
```
- `pending`/`failed` ikisi de `synced_at IS NULL` (failed = retry sayacı/last_error olan pending alt-durumu).
- **Duplicate riski mitigasyonu:** flush her satırın `idempotency_key`'ini gönderir → backend unique index ile
  dedupe eder (önerilen migration §2). Yarıda kalan flush / çift-tık tekrar gönderse bile no-op.
- **Champ-select asla sync'e bloklanmaz** (flush arka planda / oturum sınırında).

### Sprint E — uygulandı

**Client queue audit sonucu:** V012 schema sync için **yetersizdi** (retry/failed izleme yok). Eklendi
(`V013__feedback_sync_state.sql`): `retry_count INTEGER DEFAULT 0`, `last_error TEXT`, `next_retry_at INTEGER`.
Bir satır `pending` iken `synced_at IS NULL`; `synced_at` set edilince `synced`; `failed` = retry_count/last_error
olan pending alt-durumu.

**Pure resolver (`feedback_sync.rs`):** `resolve_after_send(prev, SendResult, now)` + `is_due(state, now)`.
**Korupsiyon-güvenliği (unit-test'li):** satır **yalnız başarıda** `synced` işaretlenir; hata sadece
retry_count/last_error/next_retry_at günceller → feedback asla düşmez. `MAX_ATTEMPTS=6` sonrası terminal-`failed`.

**Flush komutu (`commands/feedback_flush.rs`): UYGULANDI** — `sync_recommendation_feedback`:
- `DRAFT_BRAIN_API_BASE` yoksa `offline: true` no-op.
- Due unsynced satırları okur (lock'u network'ten önce bırakır), her satır için idempotency_key + POST
  `/v1/recommendation-feedback`, `resolve_after_send` ile DB günceller.
- **PII policy:** session_hash'siz / kısa-hash satırlar atlanır (`skipped_no_hash`), ham kimlik gönderilmez.
- `FeedbackFlushSummary { offline, attempted, synced, failed, skipped_no_hash }` (ts-rs) döner.
- **Champ-select sırasında çağrılmaz** (Codex sync UX tetikler — buton/arka plan).

## 2. Backend ingestion contract (B) — `backend/src/main.rs`

**Önceki durum:** `/v1/recommendation-feedback` gelen `feedback` string'ini **doğrulamadan** INSERT ediyordu.

**Eklendi (additive validation):**
- **Canonical verdict guard:** `feedback` paylaşılan `src/types/feedback-vocabulary.json`'a karşı doğrulanır
  (`include_str!` → client parser + frontend union ile **aynı kaynak**, drift imkânsız). Dışındaki verdict → **422**.
- **Hashed user/session policy:** `user_hash.len() < 16` → **422**. Sunucu **asla** ham PUUID/summoner adı almamalı;
  client SHA-256 (64 hex) gönderir. Uzunluk tabanı ham kimliği eler. (Privacy guardrail.)
- **Payload validation:** `champion_id > 0`, `champion_key` boş değil, `score` finite → değilse 422.
- 4 backend testi: canonical payload geçer, **4 verdict'in hepsi** geçer (drift guard), non-canonical reddedilir,
  ham user_hash reddedilir.

**Idempotency migration (Sprint E — UYGULANDI):** `backend/migrations/0002_feedback_idempotency.sql` —
`idempotency_key TEXT` kolonu + unique index. FeedbackInput'a `idempotency_key: Option<String>` (additive,
serde default) eklendi; INSERT artık `ON CONFLICT (idempotency_key) DO NOTHING` → re-send **no-op** (NULL
key'ler distinct → eski/legacy client'lar etkilenmez). Backend testi: 4 verdict + ham-hash red (dedup
testi Postgres entegrasyon gerektirir → unit kapsam dışı, runtime'da geçerli).

## 3. Feedback analytics read-only layer (C) — `recommendation/feedback_analytics.rs`

Saf `analyze_feedback(events, now, window_days)` → `FeedbackAnalytics` (ts-rs export, hepsi `number`):
| Çıktı | Anlam |
|---|---|
| `total_events` | tüm feedback satırı |
| `recent_signal_count` | **son N gün** (default 7) polar sinyal |
| `trends[]` (`ChampionFeedbackTrend`) | şampiyon başına helpful/picked/not_helpful + ağırlıklı `net_sentiment` + `recent_count`; en çok feedback'li önce |
| `disliked[]` | **"hangi öneriler kötü bulunuyor?"** — `net ≤ −0.20` ve `sample ≥ 3`, en kötü önce |

- Verdict ağırlıkları `feedback_signal`'dan reuse (Helpful +1 / Picked +0.5 / NotHelpful −1) → scoring ile
  **asla drift etmez**. Low-sample (`< 3`) negatifler "disliked" damgalanmaz (tek tık bir öneriyi kötülemez).
- Command: `commands/data_quality.rs::get_feedback_analytics(window_days?)` — read-only, network yok, lib.rs kayıtlı.
- 7 saf test + TS contract guard (`feedback-analytics.contract.test.ts`: alanlar + hepsi number).

> UI bağlama Codex'te: "son 7 gün X sinyal", "şu şampiyonlarda öneriler beğenilmiyor" kartı `FeedbackAnalytics.ts`
> tipinden okur. Hot UI'ya Claude dokunmadı.

## 4. Durum (Sprint E)
- Baseline: client cargo test **354** · gate clippy **0** · backend clippy/test/fmt **0/4/temiz** · pnpm typecheck
  pass · vitest **152/36**.
- Sprint E ekleri: client V013 migration (retry/error/next_retry kolonları), `feedback_sync` resolver+is_due
  (korupsiyon-güvenli), **flush komutu** `sync_recommendation_feedback`, backend 0002 idempotency migration +
  ON CONFLICT, `FeedbackFlushSummary` + TS contract guard. `feedback_sync.rs` artık flush'a bağlı (dead_code kalktı).
- Privacy: yerel/saf + hashed-only (skipped_no_hash). No-fabrication: low-sample damping, ağırlıklı net.

### Codex'e (sonra — sadece UX)
1. **Flush UX:** `sync_recommendation_feedback`'i buton / arka-plan otomatik sync'e bağla; `FeedbackFlushSummary`
   ile sonucu göster (synced/failed/pending). **Champ-select sırasında tetikleme.**
2. İstenirse `failed` satırlar için "tekrar dene" aksiyonu (komut idempotent + due-mantığı zaten doğru iş yapar).
