# Data Coverage Expansion Policy Core v1 — Audit

> Tarih: 2026-06-06 · Sahip: Claude (2. mühendis) · Kapsam: "hangi coverage frontier önce büyütülmeli?"
> kararı veren **saf motor**. DB/network/command/UI YOK. Veri uydurma yok (açık yoksa hedef yok). **PII yok**
> (oyuncu katkısı anonim örneklem sayıları). Codex sonra Riot key + fetch-history + scheduler runtime'a bağlar.
> Tamamlayıcı: [match-fetch-planner-core.md](match-fetch-planner-core.md) · [scheduler-policy](data-pipeline-scheduler-policy.md).

## 1. Modül
`recommendation/coverage_expansion_policy.rs` (saf, `#[allow(dead_code)]` bağlanana dek).
Giriş: `plan_coverage_expansion(&CoverageExpansionInput) -> CoverageExpansionPlan`.

## 2. DTO'lar
**Input (Rust-only):** `FrontierSample` (region/patch/role/champion_id?/current_samples/target_samples) ·
`CoverageExpansionInput` (champ_select_active/frontiers/**player_sample_counts** [anonim, PII yok]/max_targets).
**Output (ts-rs, bigint yok):** `CoverageFrontier` (+ deficit + coverage_ratio) · `CoverageTarget`
(frontier/priority/needed_samples/rationale) · `ExpansionRisk` (level/factors/summary) · `CoverageExpansionPlan`
(data_state/targets/risk/total_deficit/frontier_count).

## 3. Token vocabulary (sabit `pub const`)
- **risk factors:** `champ_select_active` · `single_player_overload` · `thin_data` · `no_open_frontier`
- **risk level:** `low` · `medium` · `high` · **data_state:** `rich` · `thin` · `insufficient`

## 4. Karar mantığı
- **Düşük sample öncelikli:** `coverage_ratio` bandı → priority (ratio <0.25→4, <0.5→3, <0.75→2, <1.0→1).
  Hedefi karşılayan (deficit 0) frontier target olmaz.
- **Deterministik sıralama:** priority desc → deficit desc → region/patch/role/champion_id asc. `max_targets` cap.
- **data_state:** frontier yok ∥ total current 0 → `insufficient`; total < 200 → `thin`; else `rich`.
- **Risk (`ExpansionRisk`):**
  - `champ_select_active` → high (plan yine hesaplanır, çalıştırma runtime'da ertelenir — **champ-select'te network yok**).
  - `single_player_overload` → tek oyuncu örneklem payı > %70 → high (tek oyuncuya aşırı yüklenme).
  - `thin_data` → medium · `no_open_frontier` (frontier var ama hepsi dolu) → medium.
- **No fabrication:** açık yoksa target yok; veri yoksa `insufficient` (sahte target üretmez).
- **PII yok:** oyuncu katkısı sadece anonim `player_sample_counts` (id yok).

## 5. Test matrisi (14 — hepsi geçti)
en-düşük-coverage en-yüksek-priority · dolu frontier→target yok+no_open_frontier · frontier yok→insufficient ·
champ-select→high risk (plan yine hesaplanır) · single_player_overload flag · thin_data flag · max_targets cap ·
determinism (region tie-break) · total_deficit/counts doğru · **token vocab lock**.

## 6. ts-rs + contract
`CoverageFrontier` · `CoverageTarget` · `ExpansionRisk` · `CoverageExpansionPlan` üretildi (**bigint yok** —
champion_id `number|null`). TS contract guard: `src/types/coverage-expansion.contract.test.ts`.

Baseline: cargo test **512** · gate clippy `-D warnings` **0** · fmt-all temiz · pnpm typecheck pass · vitest **187/48**.

## 7. i18n token vocab + drift guard (✅ BAĞLANDI)

**Durum:** Codex `dataPipeline.coverageExpansion.{risk,level,dataState}.*` i18n'i tr/en'e ekledi +
REQUIRED_KEYS'e kilitledi. Claude **drift guard'ı bağladı**:
`coverage_expansion_policy::every_emitted_expansion_token_has_an_i18n_label` — `include_str!(tr.json)` ile
motorun ürettiği her risk factor (4) / level (3) / dataState (3) token'ının `dataPipeline.coverageExpansion.*`
label'ı olduğunu doğrular. Yeni token + eksik i18n → **kırmızı** (pool_coach/data_pipeline_quality deseni).
en parity TS `i18n tr/en parity` testinde. (Aşağıdaki bloklar tarihsel referans olarak korunuyor.)

**1. Codex tr.json + en.json'a ekler (her ikisi, parity guard'a):**
```json
// tr.json — dataPipeline.coverageExpansion
"coverageExpansion": {
  "risk": { "champ_select_active": "Champ-select aktif (ertelendi)", "single_player_overload": "Tek oyuncuya aşırı yük",
            "thin_data": "İnce veri", "no_open_frontier": "Açık frontier yok" },
  "level": { "low": "düşük", "medium": "orta", "high": "yüksek" },
  "dataState": { "rich": "zengin", "thin": "ince", "insufficient": "yetersiz" }
}
// en.json
"coverageExpansion": {
  "risk": { "champ_select_active": "Champ-select active (deferred)", "single_player_overload": "Single-player overload",
            "thin_data": "Thin data", "no_open_frontier": "No open frontier" },
  "level": { "low": "low", "medium": "medium", "high": "high" },
  "dataState": { "rich": "rich", "thin": "thin", "insufficient": "insufficient" }
}
```

**2. Claude drift guard'ı bağlar** (`coverage_expansion_policy.rs` testine — i18n geldiğinde eklenecek):
```rust
#[test]
fn every_emitted_expansion_token_has_an_i18n_label() {
    const TR: &str = include_str!("../../../src/i18n/tr.json");
    let tr: serde_json::Value = serde_json::from_str(TR).unwrap();
    let ce = &tr["dataPipeline"]["coverageExpansion"];
    for f in RISK_FACTORS { assert!(!ce["risk"][f].is_null(), "risk factor '{f}' i18n yok"); }
    for l in RISK_LEVELS  { assert!(!ce["level"][l].is_null(), "level '{l}' i18n yok"); }
    for s in DATA_STATES  { assert!(!ce["dataState"][s].is_null(), "dataState '{s}' i18n yok"); }
}
```
Motora yeni token eklenip i18n unutulursa kırmızı (pool_coach/data_pipeline_quality deseni). en parity TS testinde.

## 8. Codex'e (runtime binding)
1. **FrontierSample üretimi:** `champion_rates`/`matchups`/`builds` örneklem sayıları (region/patch/role/champion) +
   hedef (TARGET_CHAMPIONS/HIGH_MATCHUPS/build hedefi) → `FrontierSample[]`.
2. **player_sample_counts:** Match-V5 katkılarının **anonim** dağılımı (id yok, sadece sayı) → over-load tespiti.
3. `plan_coverage_expansion(input)` → `CoverageExpansionPlan`. `champ_select_active` runtime state'ten.
4. **Bağlama:** plan'ın `targets` frontier'larını `match_fetch_planner`'ın `CoverageGap`'lerine çevir → batch fetch
   öncelikleri bu frontier'lara yönlensin. scheduler `match_v5` source'u expansion plan'a göre genişler.
5. `risk` (single_player_overload/thin_data) UI'da uyarı; `champ_select_active` → çalıştırma ertelenir.

> Sıra: Claude saf frontier-önceliklendirme + risk + no-PII/no-fabrication motorunu kurdu (fixture-test'li);
> Codex sample sayımı + anonim dağılım + plan→fetch binding'e bağlar. Bu blokla veri büyütme **stratejik
> önceliklendirme**yle (en eksik frontier önce, tek-oyuncu/thin riskleri görünür) güvenli.
