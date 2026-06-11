# Data Pipeline Production v1 — Quality/Freshness Core (Audit)

> Tarih: 2026-06-06 · Sahip: Claude (2. mühendis) · Kapsam: veri pipeline'ının kalite/tazelik karar
> motoru — **saf**, backend ingestion'dan **bağımsız**. Hot UI'ya, backend deploy'a, Riot key'e
> dokunulmadı. Veri uydurma yok: yargılayacak veri yoksa açıkça `insufficient`.

## 1. Konum + bağımsızlık
`recommendation/data_pipeline_quality.rs` (yeni saf modül, `#[allow(dead_code)]` bağlanana dek).
**Backend ingestion'dan bağımsız:** DB/network/Riot yok. Sadece *sayım + tazelik girdileri* alır, *karar* verir.
Codex daha sonra bunu gerçek veri akışına bağlar (aşağıda §7).

## 2. Motor
`evaluate_pipeline_quality(&PipelineQualityInput) -> PipelineQualityReport` (saf + deterministik).
Input: now, current_patch, data_patch, hedef şampiyon, rate/matchup/build/role sayımları, `sources`
(updated_at + risk_level), fallback + last-good-cache bayrakları.

## 3. Değerlendirme boyutları (9)
patch freshness · source freshness (≥48h bayat) · champion rate coverage · build coverage · exact matchup
coverage (hedef 1000) · role coverage · source risk · fallback availability · last-good-cache availability.
Hedefler backend/client confidence band'leriyle aynı: TARGET_CHAMPIONS=172, HIGH_MATCHUPS=1000,
HIGH_BUILD_COVERAGE=0.95.

## 4. Çıktı (`PipelineQualityReport`)
- **status** (machine-key): `healthy` | `degraded` | `stale` | `insufficient`
  (öncelik: insufficient > stale > degraded > healthy).
- **confidence**: `high` | `medium` | `low`.
- **coverage** (`PipelineCoverage`): tüm fraksiyonlar + bayraklar + `sources: SourceFreshness[]`.
- **gaps** (`DataGap[]`): `matchup_coverage_low` · `build_coverage_low` · `rate_coverage_low` ·
  `role_coverage_low` · `patch_stale` · `source_stale` · `high_risk_source`.
- **actions** (`PipelineAction[]`, deduped): `refresh_rates` · `refresh_builds` · `refresh_matchups` ·
  `use_last_good_cache` · `manual_seed_required`.
- **summary**: TR, hedged.

**No fabrication:** veri yoksa (`empty`) → `insufficient` + `manual_seed_required`; yüksek riskli kaynak +
fallback yok → `insufficient`. Bayat kaynak + son-iyi cache → `degraded` + `use_last_good_cache`.

## 5. Test matrisi (8 senaryo — hepsi geçti)
| Senaryo | Beklenti | Sonuç |
|---|---|---|
| full fresh data | healthy / high | ✓ |
| old patch | stale | ✓ (+ patch_stale gap, refresh_rates) |
| low matchups | degraded / medium-veya-low | ✓ (+ refresh_matchups) |
| low build coverage | degraded | ✓ (+ refresh_builds) |
| high-risk source + no fallback | insufficient | ✓ (+ manual_seed_required) |
| source stale + last-good cache | degraded + use_last_good_cache | ✓ (age_hours doğru) |
| empty input | insufficient / low | ✓ (+ manual_seed_required) |
| determinism + action dedup | aynı çıktı, refresh_rates tek | ✓ |

## 6. ts-rs + contract
`PipelineQualityReport` · `PipelineCoverage` · `DataGap` · `PipelineAction` · `SourceFreshness` üretildi
(hepsi number/string/bool — **bigint yok**). TS contract guard: `src/types/pipeline-quality.contract.test.ts`
(5 tipin exhaustive `keyof` key-guard'ı).

Baseline: cargo test **412** · gate clippy `-D warnings` **0** · fmt-all temiz · pnpm typecheck pass · vitest **171/43**.

## 7. Codex command/UI binding (✅ uygulandı, 2026-06-06)
- **Command:** `get_pipeline_quality_report` (commands/data_quality.rs) — local DB coverage + source freshness +
  patch bilgisini `PipelineQualityInput`'a çevirip `evaluate_pipeline_quality`'yi çağırıyor (lib.rs handler kayıtlı).
- **UI:** `DataStatusBadges.tsx` — pipeline status data-badge alanında; gap/action bilgileri tooltip'e bağlı.
  ChampSelectWrapper/ChampSelectScreen çağrıyı yapıyor.
- **i18n:** `dataPipeline.*` (status / confidence / gap / severity / action) tr/en eklendi, parity guard'a bağlı.

### Drift guard (Claude, bu tur)
`every_emitted_pipeline_token_has_an_i18n_label` (`data_pipeline_quality.rs` testi): 4 girdiyle (healthy /
stale-patch / degraded-her-gap / empty) motorun ürettiği **her token**'ı (`status`, `confidence`, gap
`dimension`/`severity`, `action`) `include_str!(tr.json)` ile `dataPipeline.*`'a karşı doğrular. Motora yeni
bir token eklenip i18n unutulursa → **Rust testi kırmızı**. en parity TS `i18n tr/en parity` testinde.
Baseline (bu tur): cargo test **413** · gate clippy **0** · fmt-all temiz · pnpm typecheck pass · vitest **172/43**.

## 8. Codex'e (sonraki büyük iş — actual ingestion)
Saf karar motoru + bağlama hazır; sıradaki: gerçek veri akışı. `actions` token'ları bu fetcher'lara bağlanır:
- Meraki / DataDragon cache iyileştirme
- Riot Match-V5 aggregation (match → rates/matchups)
- build/matchup fetcher'lar
- scheduler + last-good cache davranışı (`use_last_good_cache` action'ının arkası)

> Bu sıra: Claude saf kalite **karar** motorunu + drift guard'ı kurdu; Codex command/UI'ya bağladı; sıradaki
> actual ingestion Codex'te.
