# Data Pipeline Scheduler Policy Core — Audit

> Tarih: 2026-06-06 · Sahip: Claude (2. mühendis) · Kapsam: runtime refresh scheduler'ın **saf karar
> motoru** — policy + rate-limit + fetch-log observability. DB / network / tokio / timer / command / UI YOK.
> Veri uydurma yok. Codex sonra gerçek scheduler task + source_fetch_log yazımı + fetcher binding'i yapar.
> Tamamlayıcı: [data-pipeline-production-v1.md](data-pipeline-production-v1.md) · [...ingestion-contract.md](data-pipeline-ingestion-contract.md).

## 1. Modül
`recommendation/pipeline_scheduler_policy.rs` (saf, `#[allow(dead_code)]` bağlanana dek). 3 saf fonksiyon:
`plan_refresh` · `compute_rate_budget` · `summarize_fetch_logs`.

## 2. Token vocabulary (sabit — `pub const`)
- **refresh decision:** `refresh` · `skip_fresh` · `skip_rate_limited` · `skip_champ_select` · `skip_disabled` · `skip_no_budget`
- **source health:** `healthy` · `degraded` · `stale` · `insufficient`
- **fetch status:** `success` · `failed` · `rate_limited` · `skipped`

## 3. `plan_refresh(&RefreshPolicyInput) -> RefreshPlan`
Kaynak-başına refresh/skip kararı. **Deterministik** (source adına göre sıralı). Karar önceliği:
1. `champ_select_active` → **hepsi `skip_champ_select`** (champ-select sırasında network YOK — hard rule).
2. `!enabled` → `skip_disabled`.
3. per-source cooldown (`next_allowed_at > now`) → `skip_rate_limited` (+ next_allowed_at taşınır).
4. fresh (TTL dolmadı) **ve** health `healthy` → `skip_fresh`.
5. eligible (bayat ∥ health stale/insufficient/degraded) → bütçe varsa `refresh` (bütçe -1), yoksa `skip_no_budget`.

> `insufficient` health → TTL taze olsa bile `refresh` (veri yok). Bütçe global (`remaining_budget`), refresh'ler tüketir.

## 4. `compute_rate_budget(&RateLimitInput) -> RateLimitBudget`
Sliding-window: `[now - window_secs, now]` içindeki istek sayısı → `used`/`remaining`. Bütçe 0 ise
`next_allowed_at` = pencerede en eski istek + window (bir slot ne zaman boşalır). Pure.

## 5. `summarize_fetch_logs(&[FetchLogEntry], now) -> FetchLogSummary`
Kaynak-başına `SourceFetchHealth` (deterministik, source sıralı): total/success/failed, success/fail streak
(en son girdiden geriye), last_success_at, last_attempt_at, health. Health bandları:
- `success == 0` → **`insufficient`** (hiç başarı yok — uydurma yok).
- `fail_streak ≥ 3` → `degraded`.
- son başarı > 48h eski → `stale`.
- aksi → `healthy`.

## 6. Test matrisi (19 — hepsi geçti)
champ-select tüm kaynak skip · fresh→skip_fresh · stale→refresh · insufficient→refresh (taze olsa bile) ·
disabled→skip_disabled · rate-limit cooldown→skip_rate_limited+next_allowed_at · bütçe tükenince→skip_no_budget ·
rate budget in-window sayım · tükenmiş bütçe next_allowed_at · fetch-log streak/last_success · no-success→insufficient ·
health bandları (healthy/stale/degraded) · determinism+sort · **token vocabulary lock** (her emitted decision/health
fixed sette).

## 7. ts-rs + contract
Output DTO'lar üretildi: `RefreshSourceDecision` · `RefreshPlan` · `RateLimitBudget` · `SourceFetchHealth` ·
`FetchLogSummary`. **i64 unix-second alanları (`next_allowed_at`/`window_secs`/`last_success_at`/`last_attempt_at`)
→ TS `bigint`**; sayımlar `number`. TS contract guard: `src/types/pipeline-scheduler.contract.test.ts`
(5 tipin exhaustive `keyof` + bigint timestamp kontrolü). Input struct'lar (RefreshSourceInput/RefreshPolicyInput/
RateLimitInput/FetchLogEntry) Rust-only (command builds).

Baseline: cargo test **480** · gate clippy `-D warnings` **0** · fmt-all temiz · pnpm typecheck pass · vitest **182/46**.

## 8. i18n drift guard durumu
`dataPipeline.scheduler.*` i18n **henüz yok** → spec gereği şu an **Rust vocabulary-lock** testi
(`emitted_tokens_stay_in_fixed_vocabulary`: her emitted decision/health fixed `pub const` sette). **Codex UI/i18n
bağlayınca:** decision/health/status token'ları için `dataPipeline.scheduler.{decision,health,status}.*` i18n eklenmeli;
o zaman `pool_coach`/`data_pipeline_quality`'deki gibi bir `every_emitted_scheduler_token_has_an_i18n_label` Rust
drift guard'ı eklerim (include_str!(tr.json)). İstenirse ben kurarım.

## 9. Codex'e (runtime binding)
1. **tokio scheduler task** + timer (manuel refresh + background refresh ayrımı).
2. **source_fetch_log** SQLite/backend yazımı (status: success/failed/rate_limited/skipped + at) → `summarize_fetch_logs`'a besle.
3. **Champ-select active guard runtime binding:** champ-select state → `RefreshPolicyInput.champ_select_active`
   (policy zaten hard-skip ediyor; runtime sadece bayrağı doğru geçirmeli).
4. `compute_rate_budget` ile Riot/Meraki rate-limit penceresini takip et → `remaining_budget`.
5. `plan_refresh` kararlarını DDragon/Meraki/Match-V5 fetcher'larına bağla (`refresh` → fetch et, diğer skip'ler → atla).

> Sıra: Claude saf karar + rate-limit + observability motorunu kurdu (fixture-test'li, key/timer beklemez);
> Codex tokio scheduler + log yazımı + fetcher binding'e bağlar.
