# Match Fetch Planner Core — Audit

> Tarih: 2026-06-06 · Sahip: Claude (2. mühendis) · Kapsam: Match-V5 veri büyütmeden ÖNCE hangi match
> id'lerin çekileceğini deterministik, dedup'lu, coverage-aware planlayan **saf motor**. DB/network/command/
> UI YOK. Veri uydurma yok: coverage açığı yoksa hiç fetch yok (rate budget boşa harcanmaz). Codex sonra
> fetch-history tablosu + batch fetch'e bağlar. Tamamlayıcı:
> [match-v5-aggregation-core.md](match-v5-aggregation-core.md) · [scheduler-policy](data-pipeline-scheduler-policy.md).

## 1. Modül
`recommendation/match_fetch_planner.rs` (saf, `#[allow(dead_code)]` bağlanana dek).
Giriş: `plan_match_fetch(&MatchFetchPlannerInput) -> MatchFetchPlan`.

## 2. DTO'lar
**Input (Rust-only):** `MatchCandidate` (match_id/region/patch/queue_id/role_hint?/discovered_at) ·
`FetchedMatchRecord` (match_id/region/patch/status/fetched_at) · `CoverageGap` (region/patch/role/
current_samples/target_samples/priority) · `MatchFetchPlannerInput` (now/champ_select_active/rate_budget/
batch_limit/candidates/fetched_records/coverage_gaps).
**Output (ts-rs, bigint yok):** `MatchFetchDecision` (match_id/decision/reason/priority) ·
`MatchFetchPlan` (to_fetch/decisions/batch_limit/selected_count/skipped_count).

## 3. Token vocabulary (sabit `pub const`)
`fetch` · `skip_already_fetched` · `skip_rate_limited` · `skip_champ_select` · `skip_batch_full` ·
`skip_invalid` · `skip_no_gap`.

## 4. Karar mantığı
1. `champ_select_active` → **hepsi `skip_champ_select`**, to_fetch boş (network-yok kuralı bu katmanda da).
2. match_id boş → `skip_invalid`.
3. fetch-history'de status `fetched`/`parsed`/`processed` → `skip_already_fetched` (dedup; en yüksek progress alınır).
4. `failed` kayıt → **retry edilebilir ama düşük öncelik** (priority = gap_priority / 2).
5. Eşleşen aktif coverage gap (current < target, region+patch + role_hint) yoksa → `skip_no_gap`.
   **Hiç aktif gap yoksa → fetch boş** (uydurma coverage yok).
6. Eligible adaylar: `(priority desc, discovered_at desc, match_id asc)` sıralı; ilk `min(batch_limit, rate_budget)`
   → `fetch`; rate_budget 0 → `skip_rate_limited`; batch aşımı → `skip_batch_full`; budget aşımı → `skip_rate_limited`.

> Coverage gap eşleşen adaylar daha yüksek öncelik alır (gap.priority). Eksik örneklem önceliklenir, dolu gap atlanır.

## 5. Test matrisi (13 — hepsi geçti)
already-fetched dedup · failed retry (düşük priority) · champ-select all skip · rate budget 0 skip · batch cap ·
invalid id skip · coverage gap priority sıralaması · no-gap → no fetch (+ dolu gap = no-gap) · determinism ·
selected+skipped=total · **token vocab lock** · mixed-decision karışımı.

## 6. ts-rs + contract
`MatchFetchDecision` · `MatchFetchPlan` üretildi (**bigint yok** — timestamp'lar Rust-only input'ta). TS contract
guard: `src/types/match-fetch-planner.contract.test.ts` (shape + decision token vocabulary).

Baseline: cargo test **495** · gate clippy `-D warnings` **0** · fmt-all temiz · pnpm typecheck pass · vitest **185/47**.

## 7. i18n token vocab (Codex UI bağlayınca)
`dataPipeline.fetchPlan.decision.*` (7 token) henüz yok → şimdilik Rust `pub const` vocab-lock. UI plan/karar
gösterince i18n eklenmeli; o zaman `every_emitted_fetch_decision_has_an_i18n_label` drift guard'ını kurarım.

## 8. Codex'e (runtime binding — fetch-history + batch)
1. **`match_v5_fetch_history` SQLite migration** (match_id/region/patch/status/fetched_at; UNIQUE match_id).
2. Match history'den **candidate id toplama** → `MatchCandidate[]`.
3. **CoverageGap** üretimi: `champion_rates`/`matchups`/`builds` örneklem sayımları vs hedef → `CoverageGap[]`.
4. `plan_match_fetch(input)` → `MatchFetchPlan`. `champ_select_active` runtime state'ten; `rate_budget`
   `compute_rate_budget`'tan; `batch_limit` config.
5. `to_fetch` id'lerini **batch detail fetch** → `match_v5_from_detail` → `aggregate_matches` → `to_canonical_rows`
   → upsert; `parsed`/`processed`/`failed` flag'lerini fetch-history'ye yaz.
6. scheduler `match_v5` source'unu bu batch planner ile genişlet (mevcut policy + bu planner birlikte).

> Sıra: Claude saf selection/dedup/coverage motorunu kurdu (fixture-test'li, key/timer beklemez); Codex
> fetch-history tablosu + batch fetch + flag yazımına bağlar. Bu blok bitince veri büyütme **duplicate ve
> rate-limit israfı olmadan** güvenli.
