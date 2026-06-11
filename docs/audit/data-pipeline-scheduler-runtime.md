# Data Pipeline Scheduler — Runtime (Audit / QA)

> Tarih: 2026-06-06 · Sahip: Claude (2. mühendis, QA) · Kapsam: Codex'in scheduler/cache runtime
> bağlamasının QA dokümantasyonu + status contract + source_fetch_log schema guard + token vocab notu.
> Hot runtime dosyalarına dokunulmadı (policy/doc/test-only). Saf karar motoru:
> [data-pipeline-scheduler-policy.md](data-pipeline-scheduler-policy.md).

## 1. Runtime bağlama (Codex)
- **V015 migration** `source_fetch_log` — `source / status / decision / message / started_at / finished_at /
  duration_ms` (+ source-finished ve finished index'leri). "Ne yenilendi, ne zaman, hangi karar/status, neden?"
- **Background scheduler** (app startup): ilk tick **5 dk** sonra, sonraki **saatte 1**. Global rate-limit
  **6 refresh attempt / saat**. AppState'e scheduler handle/cancel + **refresh lock** (manuel refresh ve
  scheduler aynı anda çalışmaz).
- **Command** `get_pipeline_scheduler_status` → `PipelineSchedulerStatus { champ_select_active, rate_limit,
  fetch_logs, plan }` — Claude'un `RateLimitBudget` / `FetchLogSummary` / `RefreshPlan` saf tiplerini sarar.
- Scheduler Claude'un saf motorunu kullanıyor: `plan_refresh` · `compute_rate_budget` · `summarize_fetch_logs`.
- **Kaynaklar:** `ddragon` · `meraki` · `match_v5` (yalnız Riot client + active summoner varsa enabled).
- Manuel `sync_data_pipeline` artık her kaynak sonucunu `source_fetch_log`'a yazıyor; cache
  promotion/keep/reject sonucu `data_pack_cache` source'u olarak loglanıyor.

## 2. Garantiler (Codex)
- **Champ-select içinde external network YOK** — manuel + background refresh policy ile `skip_champ_select`
  loglanıyor. Champ-select öneri latency'si hâlâ network'e bağlı değil.
- Background scheduler **sadece policy uygunsa** refresh yapıyor (`plan_refresh` kararı `refresh` olanlar).
- Source observability artık **gerçek DB loglarından** (`source_fetch_log` → `summarize_fetch_logs`).

## 3. Claude guard'ları (bu tur, test/doc-only)
| Guard | Yer | Korur |
|---|---|---|
| `get_pipeline_scheduler_status` response contract | `src/types/pipeline-scheduler.contract.test.ts` | `PipelineSchedulerStatus` (champ_select_active + rate_limit + fetch_logs + plan) shape; Rust struct drift → typecheck kırılır |
| V015 schema guard | `db::schema_parity::v015_source_fetch_log_columns_exist` | source_fetch_log kolonları (source/status/decision/message/started_at/finished_at/duration_ms) — migration kolonu düşerse kırmızı |
| V014 schema guard | aynı dosya | ingestion parity kolonları (önceki tur) |
| Scheduler token vocab | `pipeline_scheduler_policy::emitted_tokens_stay_in_fixed_vocabulary` | decision/health/status token'ları sabit `pub const` sette |

## 4. i18n token vocabulary (Codex UI bağlayınca)
`dataPipeline.scheduler.*` i18n **henüz yok**. Status panel'i decision/health/status token'larını gösterince
şu yapı eklenmeli (her ikisi tr/en, parity guard'a):

| Namespace | Token'lar |
|---|---|
| `dataPipeline.scheduler.decision.*` | refresh · skip_fresh · skip_rate_limited · skip_champ_select · skip_disabled · skip_no_budget |
| `dataPipeline.scheduler.health.*` | healthy · degraded · stale · insufficient |
| `dataPipeline.scheduler.status.*` | success · failed · rate_limited · skipped |

**Eklenince:** `pool_coach`/`data_pipeline_quality`/`cacheAction` deseniyle bir Rust drift guard kurarım —
`every_emitted_scheduler_token_has_an_i18n_label` (`include_str!(tr.json)` ile motorun ürettiği her token'ın
`dataPipeline.scheduler.*` label'ı var mı doğrular). İstersen bu turun ardından eklerim.

## 5. Durum
- Baseline: cargo test **482** · gate clippy `-D warnings` **0** · fmt-all temiz · pnpm typecheck pass · vitest **183/46**.
- Bu tur: +1 status contract test, +1 V015 schema guard, runtime doc. Hot runtime dosyasına dokunulmadı, davranış değişmedi.

> Veri pipeline'ı artık uçtan uca: mapper drift → aggregation → canonical → cache policy → upsert metadata →
> **scheduler policy + rate-limit + observability + runtime** → quality/freshness. Hepsi QA-kalkanlı
> (contract + schema + token/i18n drift guard'lar).
