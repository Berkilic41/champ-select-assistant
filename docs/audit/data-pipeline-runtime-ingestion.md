# Data Pipeline — Runtime Ingestion (Audit / QA)

> Tarih: 2026-06-06 · Sahip: Claude (2. mühendis, QA) · Kapsam: Codex'in `sync_data_pipeline` runtime
> bağlamasının QA dokümantasyonu + Match-V5 schema-drift test planı + canonical→DB mapping parity tablosu +
> cache/pipeline token drift guard durumu. `commands/data_quality.rs` **hot/runtime** → Claude dokunmadı
> (yalnız okudu + test-only güçlendirme kendi saf modüllerinde). Tamamlayıcı:
> [match-v5-aggregation-core.md](match-v5-aggregation-core.md) · [data-pipeline-ingestion-contract.md](data-pipeline-ingestion-contract.md).

## 1. Runtime akış (`sync_data_pipeline`, manuel refresh)
```
sync_data_pipeline (Settings butonu — manuel, champ-select DIŞI)
  ├─ DDragon champions cache
  ├─ Meraki rates sync
  ├─ build seed import · matchup seed import
  ├─ Match-V5 flow (yalnız aktif summoner + Riot client varsa):
  │     ranked solo match id'leri → match detayları (Riot API)
  │       → map_match_v5 (raw JSON → MatchV5, unwrap_or graceful defaults)
  │       → aggregate_matches  (saf · ARAM/Arena skip · per-patch)
  │       → to_canonical_rows(region)  (saf · canonical superset)
  │       → ensure_champions_for_rows
  │       → SQLite upsert (champion_rates / champion_matchups / builds)
  ├─ rebuild_local_data_pack (generated_at yazar → freshness/last-good gerçek timestamp)
  └─ decide_cache_promotion(candidate, current_good)  (saf · promote/keep_current/reject)
        → DataPipelineRefreshSummary { before/after_status, actions[], ddragon/meraki/builds/matchups,
                                       match_v5_{matches,rates,matchups,builds,errors}, data_pack_cached, cache_action }
```

**Garantiler (Codex):**
- **Champ-select içinde network YOK** — yalnız manuel refresh akışı.
- **Graceful skip:** Riot key / aktif summoner yoksa Match-V5 atlanır; refresh boşa kırılmaz (seed/cache yine çalışır).
- Raw map `unwrap_or` ile alan-eksiğine dayanıklı → eksik/null alan → default → aggregator filtreler.
- `decide_cache_promotion` runtime'da: kötü/high-risk/regresyon aday → last-good cache korunur.

## 2. Canonical row → DB hedef mapping parity

> ✅ = kolon kalıcı (V014 client / 0003 backend ile eklendi, upsert yazıyor). ⚠ = hedefte kolon yok
> (bilinçli — başka kolon türevi). Client **active-region snapshot** modeli (region saklanır, UNIQUE source
> davranışı korunur). Önceki turda çoğu alan ⚠'di; mapping parity bloğu bu turda kapatıldı.

### `CanonicalRateRow` → champion_rates
| canonical | client (V008) | backend (0001) |
|---|---|---|
| region | region ✅ (V014 · **active-region snapshot**) | region |
| patch | patch | patch (PK) |
| champion_id | champion_id | champion_id (PK) |
| position | position | position (PK) |
| win_rate / pick_rate / ban_rate | win_rate / pick_rate / ban_rate | aynı |
| sample_size | sample_size | sample_size |
| source | source (UNIQUE) | source (PK) |
| confidence | confidence | confidence |
| — | cached_at = now | updated_at = now() |

> **Active-region snapshot:** `region` artık client'ta saklanıyor (V014) ama mevcut UNIQUE davranışı korunur —
> farklı region refresh'i aynı `source` satırını **günceller** (snapshot), per-region geçmiş tutmaz.

### `CanonicalMatchupRow` → champion_matchups
| canonical | client (V006 + V014) | backend (0001) |
|---|---|---|
| region | region ✅ (V014, snapshot) | region |
| patch | patch_version | patch (PK) |
| champion_id / opponent_id / position | aynı | aynı |
| games / wins / win_rate | aynı | aynı |
| sample_size | sample_size ✅ (V014) | ⚠ (games = örneklem) |
| source | source | source |
| confidence | confidence ✅ (V014) | confidence |

### `CanonicalBuildRow` → builds (client) / champion_builds (backend)
| canonical | client (V004 + V014) | backend (0001 + 0003) |
|---|---|---|
| region | region ✅ (V014, snapshot) | region |
| patch | patch_version | patch (PK) |
| champion_id / position | aynı | aynı |
| item_ids | item_ids (TEXT serialize) | payload.items (JSONB) |
| rune_ids | rune_ids (TEXT) | payload.runes |
| summoner_spells | builds.summoner_spells TEXT (mevcut kolon, JSON serialize) | payload.spells |
| games | games ✅ (V014) | payload |
| win_rate / pick_rate | win_rate / pick_rate | payload |
| sample_size | ⚠ (client builds'te yok) | sample_size ✅ (0003) |
| source | source | source |
| confidence | confidence ✅ (V014) | confidence |

> ✅ = V014 (client) / 0003 (backend) ile kalıcılaştırıldı; runtime upsert canonical metadata'yı **düşürmüyor**
> (`canonical_upsert_persists_ingestion_metadata` testi metadata kaybını yakalar). V014 kolonları:
> champion_rates(+region), champion_matchups(+region/confidence/sample_size), builds(+region/games/confidence).
> 0003: champion_builds(+sample_size). Kalan bilinçli parity notları: backend matchups `sample_size` ayrı kolon
> değil (games = örneklem); client builds `sample_size` yok (games aynı anlam); `summoner_spells` ayrı yeni kolon
> değil → mevcut `builds.summoner_spells` TEXT alanına JSON serialize; client `region` = multi-region history değil,
> **active-region snapshot**. Schema guard: `db::schema_parity::v014_ingestion_parity_columns_exist`.

## 3. Match-V5 schema-drift test planı

**Engine tarafı (Claude, test-edildi):** raw map sonrası motora ulaşan drift:
| Drift | Davranış | Test |
|---|---|---|
| championId eksik → 0 | participant elenir | `drifted_participants_are_skipped_gracefully` |
| teamPosition null/"" | participant elenir | aynı |
| bilinmeyen position ("AFK") | elenir | aynı |
| item eksik / boş / trinket | core build boş, panik yok | `empty_items_yield_no_core_items_without_panicking` |
| role swap (2 aynı-pos aynı-team) | rate sayılır, matchup ÜRETİLMEZ | `role_swap_same_team_counts_rates_but_no_matchup` |
| <10 participant | kısmi agregat, eksik lane matchup atlar | (drift testleriyle örtülü) |
| ARAM/Arena queue | skip + warning | `aram_matches_are_skipped` |

**Raw-mapper tarafı (✅ ARTIK test-edildi):** Codex `normalize_patch` / `parse_items` / `parse_rune_ids` /
`match_v5_from_detail`'i saf `recommendation/match_v5_mapper.rs`'e çıkardı (data_quality.rs artık raw JSON parse
etmiyor, sadece mapper sonucunu tüketiyor). Claude raw-JSON drift fixture'larını **doğrudan mapper'a** bağladı —
`recommendation/match_v5_mapper_drift.rs` (test-only modül, Codex'in dosyasına dokunulmadı, `pub` API üstünden):
| Raw drift | Davranış | Test |
|---|---|---|
| metadata.matchId eksik/yok | `fallback_id` kullanılır | `missing_match_id_falls_back_to_provided_id` |
| participants null / string / {} / yok / [] | boş liste, panik yok | `null_or_missing_participants_yield_empty_list` |
| malformed `perks.styles` (string/yanlış tip/perk string) | boş runes, panik yok | `malformed_perks_styles_never_panic` |
| item slotları kısmi / string | eksik→0, hep 7 slot | `partial_or_string_item_slots_default_to_zero` |
| garip gameVersion ("16"/"16."/".10"/"  ") | güvenli "unknown" | `weird_game_versions_normalize_without_panic` |
| queueId/championId/teamId string/null, win string | as_u64/as_bool None → default 0/false | `string_or_null_numeric_fields_default_to_zero_no_panic` |
| info yok / null detail | None | `completely_empty_object_returns_none` |

> Mapper hard-fail tek yol: `info` yoksa `None`. Diğer her şey graceful default → Riot schema değişimi refresh'i
> **çökertmez**. (Command/runtime fetch akışı Codex sahipliğinde kalır; bu testler yalnız mapper tarafında.)

## 4. Cache / pipeline token drift guard durumu
| Token seti | i18n | Drift guard |
|---|---|---|
| `dataPipeline.status/confidence/gap/severity/action` | ✓ | `data_pipeline_quality::every_emitted_pipeline_token_has_an_i18n_label` |
| `dataPipeline.cacheAction.{promote,keep_current,reject}` | ✓ | **YENİ:** `ingestion_contract::every_cache_action_has_an_i18n_label` (bu tur eklendi) |
| canonical `source`/`confidence` vocab | — | `ingestion_contract::emitted_tokens_stay_in_known_vocabulary` + TS contract |

**Bu turda kapatılan boşluk:** `cache_action` (promote/keep_current/reject) i18n'e bağlı drift guard'ı yoktu →
eklendi (8 karar dalını sürer, her `action`'ın `dataPipeline.cacheAction.*` label'ı var mı doğrular). Yeni cache
action eklenip i18n unutulursa → **Rust testi kırmızı**.

## 5. Durum
- Baseline: cargo test **459** · gate clippy `-D warnings` **0** · fmt-all temiz · pnpm typecheck pass · vitest **179/45**.
- Eklenenler (test-only + doc): engine schema-drift +3, cache-action i18n drift guard +1, **raw-mapper drift +7**
  (`match_v5_mapper_drift.rs`, Codex'in mapper'ına dokunmadan `pub` API üstünden). Hot dosyaya dokunulmadı,
  davranış değişmedi.

### Codex'e
- Mapping parity'deki ⚠ alanlar (region/confidence/sample_size/spells/games) upsert'te bilinçli drop/serialize edilmeli.
- Mapper artık tam drift-kalkanlı; yeni Riot alanı eklersen mapper testleri grafiksel-bozulmayı yakalar.
