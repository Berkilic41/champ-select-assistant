# TASKS — aktif iterasyon

> Tek seferde tek küçük görev. Tarih: 2026-06-16.

## Büyük geliştirme modu — EPIC #2: Lane-Matchup Veri-Dürüstlüğü (2026-06-17)
> Mesaj-beklemeden otonom devam. Match-History Epic bitti → öncelik #2 lane-matchup dürüstlük.
- **Iter Slice-1 (done)** — LaneMatchup barlarına "KB tahmini" kaynak etiketi. Koddan doğrulandı: phase_advantage
  yalnız arketip power_curve'den (`adv()`), ölçülen matchup'a bakmıyor → hep heuristic. core `LaneMatchup.source`
  ("kb_estimate") + recommendation.ts `source?` + LaneMatchupPanel rozeti (tooltip) + i18n kbEstimate/Hint (tr/en)
  + core/renderer testleri. Engine purity korundu (yalnız read etiketi; engine.rs/scoring.rs el değmedi). WASM rebuild
  (core/pkg gitignore). ✅ core 570 + renderer 268 + host 161 + clippy + typecheck 0 + i18n parite. (commit hazırlanıyor)
- **Sıradaki:** Slice 2 (ölçülen matchup plumbing → source="measured"; geniş core, ayrı tur) ya da Epic #3 (post-game koçluk).

## Büyük geliştirme modu — EPIC: Match-History Browser (2026-06-17, kullanıcı direktifi)
> "Küçük-yüzey tükendiyse durma. En yüksek-değerli büyük yönü Epic seç, MVP'ye böl, ilk dikey dilimi uygula.
> Kapsam belirsizse soru sormadan makul varsay (→ DECISIONS ADR-004). Her tur tek dikey değer; salt kozmetik yok.
> Test+typecheck+desktop testleri geçmeden bitmiş sayma." Öncelik: match-history (#1) → lane-matchup dürüstlük → …
- **Plan** — 3 paralel Explore (DB/data-layer, GameReviewCard reuse, lobby nav/test). Koddan doğrulandı: `matches`
  (V003) tüm alanlar var; `recentMatches` JOIN hazır; LobbyView 3→4 sekme additive; ipc-contract testi yeni komutu yakalar.
- **Iter Slice-1 (done)** — Maç Geçmişi liste sekmesi. Host: `getMatchHistory` (game-review.ts, `game_reviews` EXISTS
  → has_review) + ipc.ts kayıt. Renderer: `match-history.ts` tipi + `MatchHistoryView.tsx`/`.css` + LobbyView 4. sekme +
  i18n `matchHistory.*`/`lobby.tabMatchHistory` (tr/en). GOTCHA: played_at Unix SANİYE (×1000); RTL getByText doğrudan-
  metin-düğümü eşler → rol meta satırında regex. Testler: host getMatchHistory (sıralama/JOIN/has_review/limit) + 4
  renderer (satır/rozet-gizleme/boş/hata). ✅ renderer 262 + host 160 + typecheck 0 + i18n parite. (commit hazırlanıyor)
- **Iter Slice-2 (done)** — karne detay paneli. Host `getGameReviewByMatchId` + `get_game_review` kaydı.
  GameReviewCard opsiyonel `matchId` (verilince get_game_review; yeniden-üretme/streak yok; prop'suz "en yeni"
  StatsView'da korundu). MatchHistoryView: has_review satırı `role="button"`+Enter/Space → `selected` state →
  detay (GameReviewCard + "← Maçlara dön"). i18n `matchHistory.back`/`openReview` (tr/en). GOTCHA: RTL accessible-
  name (openReview aria-label) ile satır bulunur; back butonu "← " önekli → regex. ✅ renderer 264 + host 161 +
  typecheck 0 + i18n parite. (commit hazırlanıyor)
- **Iter Slice-3 (done)** — filtreler: rol/şampiyon/sonuç 3 açılır `<select>` (yalnız listede var-olan rol+şampiyon
  seçenek). Client-side `filtered` (yeni fetch yok); eşleşme yoksa `matchHistory.noFilterMatch`. i18n filters/
  filterRole/filterChampion/filterResult/filterAll (tr/en). GOTCHA: option metni satır metniyle çakışır → testte
  `within(getByRole('list'))` ile kapsa. ✅ renderer 266 + host 161 + typecheck 0 + i18n parite. (commit hazırlanıyor)
- **EPIC TAMAM (Match-History MVP):** liste + detay + filtre. Sonraki Epic önceliği: lane-matchup veri-dürüstlüğü (#2)
  — core `ScoringContext`→`lane_matchup_from_json`'a matchup plumbing + LaneMatchupPanel "KB tahmini" rozeti.

## Ürün geliştirme modu (2026-06-17, kullanıcı direktifi)
> "Artık yalnızca bugfix/audit değil — otonom ürün geliştirme lideri. Her iterasyonda fırsat keşfet,
> P0/P1 yoksa en yüksek kullanıcı-değerli küçük özelliği seç, uygula+test+dokümante+commit, sonra devam."
- **Keşif** — 3 Explore ajanı (feature inventory / latent intelligence / professional polish) + lider
  koddan-çapraz-doğrulama. Ajanlar olgun kod tabanında çok "açık" abarttı; ELENENLER: drills (PoolBuilder'da
  render'lı), win-prob (HeroCard'da), combo-history (HeroCard'da). Büyükler ertelendi (match-history,
  opponent-scouting=ToS).
- **Iter P-01 (done)** — draft simülatörü koçluk derinliği: GOTCHA — DraftSimulatorPanel.test.tsx ZATEN
  vardı (Glob yanlış-negatif verdi; grep yakaladı → mevcut test genişletildi, yeni dosya YOK). Koddan
  doğrulandı: `DraftSimResult.why_this_move` + `deltas` hesaplanıp render edilmiyordu. Eklendi: her pick
  "Neden bu?" gerekçesi + faktör chip'lerine işaretli delta (`signedScore` yeniden kullanıldı). Renderer-only
  (core değişmedi), `.draft-sim__why-this` CSS, tr/en `draftSimulator.whyThis` parite, +3 test.
  ✅ renderer 253 + typecheck 0 + i18n parite.
- **Iter P-03 (done)** — ComboBoard'da gerçek co-pick track-record. GOTCHA — `comboOutcomes` ZATEN
  ChampSelectWrapper'da fetch'leniyordu (yalnız HeroCard birincil combo'su için kullanılıyordu); ComboBoard'a
  tek `trackRecord` prop'u (wrapper'da memo + ChampSelectScreen pass-through) iletildi. my-key=`lockedAnalysis.champion_key`;
  eşleşmezse graceful gizli (yanlış-veri YOK). pairKey host kuralıyla aynı (inline). ≥2 maç gate'i. Renderer-only,
  `.combo-board__record` CSS, tr/en `comboBoard.trackRecord` parite, +3 test (gösterir/yok/<2-maç). ✅ renderer 256 + typecheck + i18n. (commit bekliyor)
- **Iter P-06 (done)** — Ayarlar paneli native `window.confirm` → temalı discard dialog. Lider seçimi: P-02
  (LLM test) fonksiyonel ama dar+5-dosya-IPC; confirm-modal GENİŞ (tüm ayar kullanıcıları)+profesyonel+renderer-only
  (düşük mimari risk) → tercih edildi. Agent-3 bulgusu KODDAN teyit (SettingsPanel:58,66 gerçekten window.confirm).
  `confirmingClose` state + Escape-önce-onayı-kapat; footer İptal dokunulmadı (explicit discard). `.settings-confirm`
  CSS, tr/en parite, +1 test. ✅ renderer 257 + typecheck + i18n. (commit bekliyor)
- **Iter P-07 (done)** — PoolBuilder dürüst veri-hatası: `error` state + üç-durum render (loading/error/empty);
  `get_pool_suggestions` reddedilince `app.dataError` (RankCard:41,60 deseni), `Promise.allSettled` reject'i
  artık sessizce "öneri yok"a düşmüyor. Lider seçimi: 5-boyutlu doğrulanmış keşif (19 ajan, 14 aday/0 çürütüldü)
  sonrası window-opacity (IPC ölü AMA `setWindowOpacity` event-only+saydamlık tasarımı belirsiz) ve
  lane-matchup-badge (core plumbing/engine-purity) ertelendi → P-07 en temiz küçük+güvenli+tema-uyumlu aday.
  Koddan doğrulandı (app.dataError tr+en'de var). Renderer-only, sıfır yeni i18n, +1 test.
  ✅ renderer 258 + typecheck 0 + i18n parite. (commit hazırlanıyor)
- **Sıradaki:** dar/kısmen-kapsalı küçük adaylar tükeniyor (P-02 LLM-test dar · P-04 klavye · P-05 snapshot ·
  düşük-değer rozetler/skeleton'lar). **Lider notu: gerçek değer artık BÜYÜK özelliklerde** — match-history browser
  (geniş, yüksek-değer; ürün-kararı/onay ister) ya da lane-matchup veri-dürüstlüğü (core, ayrı dikkatli tur).

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

**Discovery-4 sonrası micro-backlog tükendi → kullanıcı YÖN seçti: WS3 overlay polish.**
- B-24 (son açık iş) wontfix → motor-e2e zaten `core/tests/recommendation_tests.rs`'te
  oracle seviyesinde kapsalı (yukarıda kanıt). Yeni bir test eklemek churn olurdu.

## WS3 — overlay / in-game UX polish (2026-06-17, kullanıcı onaylı yön)
- **Iter 19 (W-01 core)** — `IngamePlan`'a `power_early/mid/late: f32` eklendi,
  `build_ingame_plan` arketipten doldurur; e2e testi arketiple birebir kilitler.
  Bulgu: core `IngamePlan` artık `#[ts(export)]` TÜRETMİYOR (Tauri host göçte öldü) →
  `IngamePlan.ts` orphan/elle-bakımlı; paralel elle senkronlandı. ✅ core 505 + clippy. (91aaa12)
- **Iter 20 (W-01 renderer)** — PowerCurveBar: 3-segment (erken/orta/geç) HUD çubuğu,
  zirve faz teal; tek `role=img`+yüzdeli aria-label (çubuklar aria-hidden); tr/en parite;
  4 izole test (named export). ✅ renderer 247 + typecheck + WASM rebuild + desktop 158. (64f4c27)
  > Teyit: spike_window/matchup_tips/opponent core'da ZATEN tam+test'li+render'lı (memory
  > notu kısmen bayatmış); gerçek açık "HUD görsel" → power-curve viz (yeni glance-değeri).
- **Iter 21 (W-02)** — güç çubuğu canlı oyun fazına bağlandı: `currentPhase`=`macro.phase`
  (koddan teyit: `GAME_PHASES`≡`POWER_PHASES`=erken/mid/late, mapping gerekmez) → o kolon
  "şu an buradasın" (▾)+teal; `isKnownPhase` guard bilinmeyen fazı yok sayar; `ariaNow` aria
  fazı içerir; tr/en parite; 3 yeni izole test (işaretçi/yok/bilinmeyen). Renderer-only (core
  değişmedi → WASM rebuild gerekmez). ✅ renderer 250 + typecheck + i18n. (7eb7b6c)
- **Iter 22 (W-03)** — kullanıcı "kalan küçük overlay işleri" dedi → hesaplanan-ama-gizli plan
  alanları: `damage_profile` (hasar tipi) role'den sonra PlanRow (boş-string guard); `level`
  KDA satırına önek (Sv/Lv). Koddan teyit: ikisi de core'da üretiliyor, IngameView render
  etmiyordu. Wiring (sıfır yeni mantık; diğer 8 PlanRow gibi test'siz, i18n parite kapsar);
  fragile IngameView integration testi oransız bulundu (host-invoke+settings+fake-timer mock).
  ✅ renderer 250 + typecheck + i18n parite. (80aea1f)
  > WS3 overlay polish'in tüm gerçek açıkları kapandı; daha fazlası churn olurdu.

## Canlı-veri yolu sağlamlaştırma (2026-06-17, lider kararı — roadmap #1)
> Kullanıcı "lider olarak sonraki hamle" diye sordu → lider canlı-veriyi seçti (en yüksek değer
> tavanı; prod-key dış-engeli yalnız ingestion cron'unu kapsar, okuma+wiring+dürüstlük+test otonom).
> Koddan denetim: worker okuma yolu + app edge-sync OLGUN; gerçek açık = staleness dürüstlük asimetrisi.
- **Iter 23 (L-01)** — bayat edge matchup/build confidence dürüstlüğü: `syncEdgeRates`'in
  staleness downgrade'i (>48s→'low') yalnız rates'e uygulanıyordu; matchups+builds aynı bayat
  ingestion'dan örnek-bazlı 'medium'/'high' görünüyordu → durmuş ingestion/dev-key expiry'de
  sahte-tazelik. Koddan teyit: rates yanıtının kanonik `updated_at`'i (worker `ingest_meta`)
  üç tabloyu kapsar; `stale` flag matchups(641)+builds(678) confidence'ına da uygulandı.
  TDD-first: önce RED (games=800 bayat matchup 'medium' kalıyordu) → fix → GREEN. ✅ desktop
  159 + typecheck. (c72dd1c)

> **Lider değerlendirmesi (2026-06-17):** 4 keşif turu + her adayın koddan-teyidi sonrası
> güvenli + değerli + doğrulanabilir backlog TÜKENDİ. Olgun kod tabanı (~620+ test, clippy
> temiz, 4-job CI). Bundan sonraki değer küçük otonom adımlarda DEĞİL — roadmap'in
> ürün-kararı gerektiren büyük kalemlerinde: canlı-veri (Riot prod-key inceleme), ML/LLM
> faz, overlay. Bunlar kullanıcı yönü ister; spekülatif 5. tur micro-tarama churn olur.
> Döngü burada kullanıcı girdisi için duruyor (manufacture-churn kuralı).
