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

## Iter 13+ — loop keşif (Discovery-3, 2026-06-17)

- **Discover** — `csa-loop-discovery-3` workflow (20 ajan, 5 lane: core-correctness/
  core-data/host/renderer/cross-cutting) → 15 aday → adversaryal koddan-teyit → **4
  doğrulanmış** (B-38 high, B-39/B-40 med, B-24 low=mevcut). 11 yanlış-alarm elendi.
- **Iter 13** — **B-38** stretch-pick risk notu u32 underflow koruması: not-üretimi
  saf `stretch_risk_note`'a çıkarıldı + `saturating_sub` + 3 birim testi
  (zero/normal/bozuk-wins>games). ✅ core 569 test + clippy `--all-targets` temiz.

## Durum — backlog esas olarak TÜKENDİ (2026-06-16, ~30 commit)

**Bu oturumda (devam, 8 commit `1967d79…0f8d64b`):**
- **B-12c** OCE match-v5 routing-split (`matchRoutingForRegion` oc1→sea; account-v1 americas'ta).
- **B-10** noMeta yapısal `missing_signals`'a (sihirli-sabit `meta_score==0.3` kalktı, %50.1 yanlış-poz. giderildi).
- **B-23** canlı-veri dürüst chip testleri (noRiotKey + bayat liveDataAge + taze=yok).
- **B-08** wontfix (cold-DB champMap riski B-01 onError + B-15 ile kapsanmış).
- **B-02** cold-start seed priming (`primeColdStartSeeds`, scheduler DDragon-sonrası, atomik+best-effort; FK silent-fail tuzağı yakalanıp doğru yere kondu).
- **B-24** kısmen çözüldü (noMastery chip ölü DEĞİL — engine.rs:73-117 doğrulandı); motor-e2e ertelendi.
- **B-33** useChampSelect 7 türev-effect → `useSessionDerived` (TDD-first: önce güvenlik-ağı testleri).
- **B-32** `syncDataPipelineInner` god-function → `runSource<T>` helper (5 kaynak DRY).

**Tüm test yeşil:** renderer 243 · desktop 155 · worker 16 · core (clippy) — typecheck temiz, i18n parite.

**Kalan (1 açık, ERTELENDİ):**
- **B-24** (low) — kalan motor-e2e: mastery'siz+kombo'lu session fixture'ı kurup engine'in
  combo-backed stretch (comfort 0) listesi ürettiğini kilitlemek + orWarnDefault/engine-0.3.
  Yüksek-efor fixture; chip davranışı zaten DataStatusBadges testinde kapsalı. Taze bağlam ister.

> Lider değerlendirmesi: güvenli + değerli + doğrulanabilir backlog bitti. Kalan tek iş
> (B-24 motor-e2e) elaborate fixture gerektiren, görünür-değeri düşük bir test-kilidi.
