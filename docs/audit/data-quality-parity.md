# Data Quality Parity Audit — Backend `/v1/data-quality` ↔ Client `get_data_source_registry`

> Tarih: 2026-06-03 · Sahip: Claude (2. mühendis) · Kapsam: Data Supremacy v1 alan parity

## Amaç

Cloud backend `/v1/data-quality` (Axum/Postgres, `backend/src/main.rs::data_quality`) ile
client `get_data_source_registry` (`recommendation/draft_brain_data.rs::DataSourceRegistryReport`)
**aynı kavram sözlüğünü** taşımalı ki cloud ve local "veri kalitesi" okuması tek zihinsel modelde olsun.

## Alan eşleşmesi

| Alan | Backend `/v1/data-quality` | Client `DataSourceRegistryReport` | Durum |
|---|---|---|---|
| `champion_rates_count` | ✅ | ✅ | ✓ |
| `matchup_count` | ✅ | ✅ | ✓ |
| `build_count` | ✅ | ✅ | ✓ |
| `primary_role_build_coverage` | ✅ (distinct champ / 172) | ✅ (distinct champ / total) | ✓ |
| `meta_role_coverage` | ✅ | ✅ | ✓ |
| `exact_matchup_coverage` | ✅ (= matchup_count) | ✅ (= matchup_count) | ✓ |
| `stale_sources` | ✅ (`source_fetch_log`, 24h) | ✅ (pack cache expiry) | ✓ (kaynak farklı, kavram aynı) |
| `high_risk_sources` | ✅ (`source_fetch_log.risk_level='high'`) | ✅ (registry risk='high') | ✓ |
| `fallback_active` | ✅ | ✅ | ✓ |
| `confidence` | ✅ **(bu audit'te eklendi)** | ✅ | ✓ |
| `generated_at` | ✅ | ✅ | ✓ |
| `sources` (zengin `DataSourceEntry[]`) | ➖ flat `sources` yalnız `data-pack/latest`'te | ✅ (DataSourceKind registry) | **Bilinçli divergence** |
| backward-compat flat (`rates/matchups/builds/feedback/draft_samples`) | ✅ | ➖ | Backend-only (eski tüketiciler) |

## Bulgular

1. **`confidence` eksikti** → backend `data_quality`'ye **additive** eklendi; client `data_confidence`
   ile birebir aynı eşik mantığı (yüksek coverage + non-fallback → high; bir miktar veri → medium;
   hiç → low). Hedef roster sabiti her iki tarafta da **172**.
2. **`sources` (zengin kaynak listesi) divergence** — client `DataSourceKind` registry'sini
   (8 kind + risk/confidence/region/fallback_used) tutar; backend bunu flat olarak yalnız
   `/v1/data-pack/latest.sources`'ta verir. Bu **bilinçli**: source registry client-side bir kavram
   (offline davranışı yönetir), backend'in cloud ingestion'ı zaten `source_fetch_log`'ta tutuluyor.
   Gerekirse ileride backend'e `SELECT DISTINCT source, risk_level FROM source_fetch_log` ile
   zengin `sources` eklenebilir (additive).
3. **Eşik sabitleri ortak:** `HIGH_MATCHUPS = 1000`, `HIGH_BUILD_COVERAGE = 0.95`,
   `TARGET_CHAMPIONS = 172` — iki tarafta da aynı. İleride tek kaynaktan paylaşmak (örn. paylaşılan
   sabitler dosyası) drift riskini azaltır; v1'de değer bazında hizalı.

## Pack metadata parity (Data Supremacy v1.1 — 2026-06-03)

Coverage raporundan ayrı olarak, **data pack'in kendisi** (model/data pack sync hattı) artık
`confidence` + `generated_at` taşıyor. Bu, öneri `DataSourceBadge`'lerini ve quality raporunu besler.

| Alan | Backend `/v1/data-pack/latest` | Client `DataPack` (draft_brain.rs) | Durum |
|---|---|---|---|
| `version` / `patch` / `region` | ✅ | ✅ | ✓ |
| `sources` (flat) | ✅ | ✅ `Vec<String>` | ✓ |
| `quality` (rates/matchups/builds/…) | ✅ (`rates` key) | ✅ (`#[serde(alias="rates")]`) | ✓ |
| `fallback` | ✅ | ✅ | ✓ |
| `generated_at` | ✅ | ✅ `Option<u32>` | ✓ |
| `confidence` | ✅ **(bu audit'te eklendi — boşluk kapandı)** | ✅ `Option<u32→String>` | ✓ |

**Kapanan boşluk:** `/v1/data-pack/latest` `confidence` GÖNDERMİYORDU → cloud pack'leri client'ta
confidence'sız geliyordu (`DataPack.confidence = None`), badge cloud güvenini yansıtmıyordu. Artık
backend her iki uçta da (`data-pack/latest` + `data-quality`) tek paylaşılan `confidence_band()`
helper'ını kullanıyor → formül drift'i imkânsız. Local fallback pack zaten `confidence: Some("low")`.

## Client `DraftBrainQualityReport` (commands/draft_brain.rs, Codex) eşleşmesi

Pack-merkezli client raporu; coverage-merkezli `DataSourceRegistryReport`'tan farklı bir görünüm:

| `DraftBrainQualityReport` alanı | Kaynak | Backend karşılığı |
|---|---|---|
| `data_pack_version` | cache'lenmiş pack | `data-pack/latest.version` |
| `data_pack_confidence` | `DataPack.confidence` | `data-pack/latest.confidence` ✓ (yeni) |
| `data_pack_generated_at` | `DataPack.generated_at` | `data-pack/latest.generated_at` ✓ |
| `data_pack_fresh` | `generated_at` < 24h | (client türetir; backend'de TTL yok) |
| `model_pack_version` | cache'lenmiş model pack | `model-pack/latest.version` |
| `feedback_total` / `feedback_unsynced` | local SQLite | `/v1/data-quality.feedback` (cloud toplamı) |
| `cloud_configured` / `notes` | runtime/env | — (client-only) |

> İki client raporu kasıtlı ayrı: `DraftBrainQualityReport` = **pack/sync/freshness**;
> `DataSourceRegistryReport` = **coverage/registry**. Backend `/v1/data-quality` ikincisinin
> kavram setini taşır; `/v1/data-pack/latest` birincisini besler.

## Codex için önerilen düşük-çakışmalı testler (commands/draft_brain.rs)

Hot dosya olduğundan Claude eklemedi; öneri olarak:
1. `data_pack_fresh` sınırı: `generated_at = now-23h` → `true`; `now-25h` → `false`.
2. `data_pack_confidence` pass-through: cache'te `confidence:"high"` olan pack → rapor `Some("high")`.
3. Cloud pack `confidence` yoksa (eski backend) → `data_pack_confidence = None`, fallback notu eklenir.
4. Local fallback aktifken `data_pack_confidence = Some("low")` + düşük-kalite notu.

## Test coverage (regresyon kalkanı — 2026-06-03)

Parity artık testlerle kilitli; bir tarafı bozan değişiklik kırmızı yanar:

| Kapsanan alan | Test | Sahip |
|---|---|---|
| `data_pack_fresh` 24h sınırı (23h→true / 25h→false) | `commands/draft_brain.rs` metadata testleri | Codex |
| `data_pack_confidence` pass-through (cache "high" → rapor) | `commands/draft_brain.rs` | Codex |
| Eski cloud pack `confidence` yok → `None` + uyarı notu | `commands/draft_brain.rs` | Codex |
| Local fallback → `confidence = low` + düşük-kalite notu | `commands/draft_brain.rs` | Codex |
| Confidence eşikleri (1000 / 0.95 / 172) client tarafı | `draft_brain_data::confidence_thresholds_match_documented_parity` | Claude |
| Üretilen TS tipleri (registry/scouting + DraftBrain pack/sync) shape | `src/types/data-supremacy-contract.test.ts` | Claude + Codex |

`get_draft_brain_quality_report` rapor üretimi artık test-edilebilir private helper'a çıkarıldı (Codex),
yani metadata path'i deterministik doğrulanıyor.

## TS type parity (API contract)

Tauri command dönüş tipleri ve generated TypeScript durumu:

| Command | Rust dönüş tipi | Generated TS? | UI bağlı? |
|---|---|---|---|
| `get_data_source_registry` | `DataSourceRegistryReport` | ✅ `generated/DataSourceRegistryReport.ts` | ➖ (Codex'e bırakıldı) |
| `get_lobby_scouting` | `ScoutingReport` | ✅ `generated/ScoutingReport.ts` | ✅ (ChampSelectWrapper) |
| `rebuild_local_data_pack` | `DataPack` | ✅ `generated/DataPack.ts` + `generated/DataPackQuality.ts` | ➖ |
| `get_draft_brain_quality_report` | `DraftBrainQualityReport` | ✅ `generated/DraftBrainQualityReport.ts` | ➖ |
| `sync_model_pack` / `sync_data_pack` | `PackSyncStatus` | ✅ `generated/PackSyncStatus.ts` | ➖ |
| `get_recommendations` / draft brain | `Recommendation` | ✅ `generated/Recommendation.ts` | ✅ |

**Kapanan boşluk (Codex):** `DraftBrainQualityReport`, `PackSyncStatus`,
`DataPack` ve `DataPackQuality` artık `#[derive(TS)]`
`#[ts(export, export_to="../../src/types/generated/")]` taşıyor. Frontend UI bağlaması
`any`/elle tip kullanmak zorunda değil; pack/sync tipleri de
`data-supremacy-contract.test.ts` ile compile-time kilitli.

### Audit bulgusu: `generated_at` tip tutarsızlığı (ÇÖZÜLDÜ — Claude)

TS-contract audit'inde tespit: aynı kavram olan `generated_at`, pack ailesinde (`DataPack`,
`PackSyncStatus`, `DraftBrainQualityReport.data_pack_generated_at`) Rust `u32` → TS **`number`**;
ama `DataSourceRegistryReport.generated_at` Rust `i64` → TS **`bigint`** idi. Aynı alanın UI'da hem
`number` hem `bigint` gelmesi format/karşılaştırma hatası riski.

**Çözüm:** `DataSourceRegistryReport.generated_at` (Claude dosyası) `i64`→`u32` hizalandı → artık
**tüm `generated_at` = `number`**. ts-rs üretimi güncellendi, contract testi `number` bekliyor.
Kalan `bigint` (i64) alanlar bilinçli: `DataSourceEntry.updated_at` (= `DataSourceBadge.updated_at`)
ve `DraftBrainQualityReport.feedback_total/unsynced` (sayaçlar). Yani **field-adı bazında tutarlı**:
tüm `generated_at` → number; tüm `updated_at` → bigint.

> UI notu (Codex): `feedback_total`/`feedback_unsynced` ve `updated_at` **`bigint`** gelir
> (`Number(x)` ile formatla); `generated_at`/`data_pack_generated_at` artık `number`.

## Sonuç

Hem **coverage sözlüğü** (`/v1/data-quality` ↔ `DataSourceRegistryReport`) hem **pack metadata**
(`/v1/data-pack/latest` ↔ `DataPack` ↔ `DraftBrainQualityReport`) paritede; `confidence` boşluğu
kapatıldı, backend tek `confidence_band()` ile drift'e kapalı. Parity artık **testlerle kilitli**
(Codex command-level + Claude threshold/TS-contract + Codex pack/sync TS export contract).
Sonraki açık iş UI bağlama: quality badge / freshness warning / DraftBrain detail panel. Tüm backend
değişiklikleri additive + geriye uyumlu.
