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
  (zero/normal/bozuk-wins>games). ✅ core 569 test + clippy `--all-targets` temiz. (a0c586c)
- **Iter 14** — **B-39** Arena (queue 1700) laneless: `my_pos()` →
  `matches!(queue_id, 450 | 1700)` (engine.rs is_aram ile hizalı) → sızan tercih-rolü
  Arena önerilerine yanlış "lane_performance eksik" basmaz. Regresyon testi. ✅ core
  570 test + clippy temiz.
- **Iter 15** — **B-40** `docs/api-key-policy.md` Tauri→Electron: dev-bölümü
  (`src-tauri/.env`+`dotenvy`→`process.env.RIOT_API_KEY`/`runtimeEnv()`), checklist
  (`tauri.conf.json`/`target/release` → bundled config + `app.asar` taraması), LCU-note
  (`champ_select.rs`→`commands/lcu.ts`). ✅ saf-doküman (253f3dd sonrası, commit'siz→commit).
- **Discover-4** — `csa-loop-discovery-4` (derin lane'ler: test-quality/concurrency/
  migration/contract/honesty-deep) → 11 aday. Verify fazı session-limit'e takıldı (2:10
  reset) → lider KODDAN self-verify etti.
- **Iter 16** — **B-41** (DB-003) matchup ingestion `wins > games` guard: u.gg
  `parseUggMatchups` + edge `syncEdgeRates` bozuk satırı (win_rate >1.0) filtreler →
  B-38'in upstream tamamlayıcısı. 2 regresyon testi. ✅ desktop 156 test + typecheck temiz.
  DB-001 (CHECK migration) reddedildi (SQLite ALTER ADD CONSTRAINT yok).
- **Iter 17** — **B-46** recommendations error-path testi: `recommendations({})` WASM
  sınırında `/invalid recommendations input/` fırlatır (draftVerdict zaten kilitliydi,
  bu birincil-input'u kilitler). Saf test, sıfır-risk. ✅ desktop 157 test + typecheck.
- **B-42 deferred** — KODDAN DOĞRULANDI: "süresiz offline" çerçevesi yanlış (döngü B-21
  ile korunlu); kalan marjinal floating-promise hijyeni, düzgün test watcher-injection
  refactor'ı ister → oransız. Uydurma değer yerine dürüst erteleme.
- **Iter 18** — **B-47** parseUggOverview kırpık-perk sınır testi: eşik-üstü (260≥200) ama
  5-perk sayfa → primary `rune_ids`=[8010,8000] üretilir, `secondary_runes`=[] kalır
  (`perks.length>=6` guard'ı; off-by-one `>=5` olsaydı [8300,9111,undefined] sızardı).
  Saf test, sıfır prod-kodu. ✅ desktop 158 test + typecheck temiz. (CHANGELOG'a yazılmaz — davranış değişmedi, B-46 emsali.)
- **Discovery-4 KAPANDI** — kalan adayların koddan-disposition'u: **B-45 wontfix** (yanlış alarm:
  cast'ın altı null-safe + core yeniden-doğruluyor) · **B-43/B-44 deferred** (renderer effect-race;
  self-correcting + deterministik race-test'i oransız ağır harness).
- **B-24 wontfix** — KODDAN DOĞRULANDI: "kalan motor-e2e" aslında AÇIK DEĞİL. `core/tests/recommendation_tests.rs`
  üç testle tam kilitliyor: `combo_backed_stretch_appears_even_with_no_mastery` (mastery'siz Orianna,
  Nocturne kombosu cb≥0.80 → çıkar), `stretch_pick_has_risk_note_and_one_at_most` (`comfort_score<0.10`
  stretch'in risk_note'u + max-1 gate), `no_stretch_when_no_strong_combo` (negatif). Bunlar test-oracle
  (core) seviyesinde — B-24'ün önerdiği WASM/TS tekrarı oracle kapsamını boilerplate'le KOPYALAR (churn).

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

**Kalan: YOK — açık/güvenli/değerli micro-iş kalmadı (2026-06-17).**
- B-24 (son açık iş) wontfix → motor-e2e zaten `core/tests/recommendation_tests.rs`'te
  oracle seviyesinde kapsalı (yukarıda kanıt). Yeni bir test eklemek churn olurdu.

> **Lider değerlendirmesi (2026-06-17):** 4 keşif turu + her adayın koddan-teyidi sonrası
> güvenli + değerli + doğrulanabilir backlog TÜKENDİ. Olgun kod tabanı (~620+ test, clippy
> temiz, 4-job CI). Bundan sonraki değer küçük otonom adımlarda DEĞİL — roadmap'in
> ürün-kararı gerektiren büyük kalemlerinde: canlı-veri (Riot prod-key inceleme), ML/LLM
> faz, overlay. Bunlar kullanıcı yönü ister; spekülatif 5. tur micro-tarama churn olur.
> Döngü burada kullanıcı girdisi için duruyor (manufacture-churn kuralı).
