# TASKS — aktif iterasyon

> Tek seferde tek küçük görev. Tarih: 2026-06-16.

## Tamamlanan iterasyonlar
- **Iter 0** — Sistem kurulumu (7 yönetim dosyası + döngü). ✅
- **Iter 1** — B-01 image fallback (`BanIcon`+`CounterItemIcon`). ✅ 214 test.
- **Iter 2** — B-05 BuildSummary 'none' dürüst mesaj. ✅ 217 test.
- **Discover** — `csa-backlog-discovery` workflow (36 ajan) → 21 doğrulanmış bulgu (B-09…B-24). ✅
- **Iter 3** — B-09 DataStatusBadges cap önceliği (actionable-first sort) + eviction testi. ✅ 218 test.

- **Iter 4** — B-16 PoolBuilder loading-vs-empty (loading state + `poolBuilder.loading`). ✅ 220 test.

- **Iter 5** — B-14 worker patch leksik-sort → `updated_at DESC` (recency) + davranışsal regresyon testi. ✅ worker 16 test. (deploy bekliyor)

- **Iter 6** — B-13 u.gg satırlarını `uggPatch` ile etiketle (staleness maskesi kalktı). ✅ desktop 16 sources test.

- **Iter 7** — B-12 routingForRegion `br1→americas` (BR account-v1+match-v5). ✅ desktop test. (oc1 → B-12b)

- **Iter 8** — B-18 StatsView ince-veri notu (+tr/en+2 test). ✅ renderer 222.

- **Iter 9** — B-20 outcomes `pickRecorded` retry-safe (flag try içine) + retry testi. ✅ desktop 15 outcomes test.

- **Iter 10** — B-22 roleSource 'preferred' provenance + nötr hint (+tr/en+test). ✅ renderer 222. (commit 94037dd SONRASI, henüz commit'siz)

- **Iter 11** — B-15 onboarding'de ddragon sync (placeholder key önlendi) + sıra testi. ✅ renderer 222.

- **Iter 12** — B-11 puuid çözülünce aktif session için recs refetch + test. ✅ renderer 223.

## Durum (Iter 21 sonu) — hızlı-win backlog tükendi
**Bitmiş (21 iterasyon, 13 commit):** B-01/05/09/16/14/13/12/18/20/22/15/11/06/21/26/25/35/34/29/31/27/30 + sistem.
**Kalan (daha büyük / taze bağlam ister):**
- **B-03** (med) worker freshness sinyali — worker `/v1` response'a `updated_at`/age + desktop tüketim + UI stale-chip (3-katman, deploy ister).
- **B-28** (low) modal `aria-labelledby` + focus trap/restore (SettingsPanel + ChampionDetailCard).
- **B-32** (low) `syncDataPipelineInner` 220-satır god-function refactor (5 kopya source-step → ortak helper).
- **B-33** (low) useChampSelect 7 kopya fetch-on-signature effect → ortak helper.
- **B-19** (low) App mount global `get_ddragon_version`. **B-17** (low) RankCard/Trend/Weekly puuid retry. **B-24** test gaps (wasm). **B-12b** OC1 routing-split.

## Iterasyon 13+ — durum
İlk keşif batch'inin (B-09…B-24) **yüksek-değer + kolay** işleri bitti. Kalan: B-03
(med, worker freshness — çok-katmanlı), B-19/B-21/B-17 (low), B-24 (wasm), B-12b/B-06.
→ **İkinci keşif workflow'u** (wtpa90ort) çalışıyor (perf/a11y/mimari/güvenlik/concurrency/robustness).
**B-06** (docs: `.claude/CLAUDE.md` Tauri→Electron) ✅ tamam. Workflow bulguları gelince yeni batch.

### Sonraki adaylar (önceki liste)
- **B-22** (low, renderer) — roleSource kalıcı tercihi 'manual' etiketliyor → 'preferred' ekle.
- **B-11** (med, renderer) — puuid yarışı (puuid gelince refetch yok) — dikkatli (useChampSelect).
- **B-19** (low, renderer) — App mount'ta global `get_ddragon_version` (champ-select yolu da canlı patch alsın).

### Diğer adaylar (data-honesty cluster kalanı)
- **B-14** (high, worker) — patch leksik-sort → `ORDER BY updated_at DESC` (deploy gerekir; worker test).
- **B-12** (med, desktop) — routingForRegion: br1→americas NET; oc1 account-v1(`americas`) vs match-v5(`sea`) çakışması → routing'i API'ye göre ayırmak gerekebilir (dikkatli).
- **B-10** (med, renderer) — noMeta `missing_signals`'a geçir; AMA test fixture `rec()` missing_signals set etmeli (14 test etkilenir) → dikkatli.
- **B-13** (med, desktop) — u.gg satırlarını `uggPatch` ile etiketle (staleness maskesini kaldır).
- **B-11** (med, renderer) — puuid yarışı: puuid çözülünce mevcut session için recs refetch.
