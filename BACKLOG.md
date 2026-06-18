# BACKLOG

> Skor: `değer = etki + borç + test-edilebilirlik + hedef-uyum − risk − efor`.
> Durumlar: `todo · doing · done · blocked · wontfix`. Tarih: 2026-06-16.
> Aşağıdaki B-09+ işleri `csa-backlog-discovery` workflow'unun (36 ajan, 30 aday →
> **21 doğrulanmış bulgu**, adversaryal koddan-teyit) çıktısıdır.

## Aktif
**EPIC: ML/LLM Koçluk Fazı (2026-06-18, kullanıcı AskUserQuestion ile seçti).** Güvenli küçük-yüzey tükenince
yön soruldu → ML/LLM seçildi. Keşif: pipeline ZATEN tam bağlı+test'li (coach_narrator+audit, host llm-narrator,
settings, render, 6 test); MVP çalışıyor. Motor purity by-design korunur (LLM yalnız anlatım seam'i; scoring değişmez).
- **Slice 1 done (ADR-010)** — koç-notu kaynak şeffaflığı: `CoachNarrative.source`/`external_rejected` (computed-but-
  unrendered) DeepDiveTab koç-notu başlığına rozet (external→"LLM"; rejected→"LLM reddedildi"+tooltip; düz deterministik→
  rozet yok). Renderer+i18n+CSS; core/host EL DEĞMEDİ. renderer 285 + typecheck 0 + i18n parite. +3 test.
- **Slice 2 done** — Settings "Bağlantı test et" butonu: girilen LLM endpoint+model'i champ-select'i beklemeden
  doğrular (minimal "ping" isteği, kullanıcı/oyun verisi YOK; dürüst ✓/✗ durum). Yeni host `testCoachLlm` (llm-narrator.ts;
  FetchFn seam → test edilebilir; OpenAI-uyumlu yanıt biçimi doğrulaması) + `test_coach_llm` ipc + SettingsPanel buton/durum
  + `.sp-llm-*` CSS + i18n coachLlmTest/Testing/Ok/Fail (tr/en). Engine purity (core el değmedi). host 173 + renderer 285 +
  typecheck 0 + i18n parite; +5 host test (ok/http/bad-response/network/boş).
- **Slice 3 done** — daha zengin + güvenli grounded promptlar: buildCoachUserPrompt'a `enemy_team_summary` (rakip kompo),
  `phase_matchup` (faz avantajı erken/orta/geç ~%), `missing_signals` (veri boşluğu → "iddia ETME" anti-halüsinasyon guard).
  Hepsi recommendation payload'ında MEVCUT (tam rec geçiriliyor — ChampSelectWrapper:181 teyitli); prose plan'lar EKLENMEDİ
  (dairesel olurdu). Host-only (core/renderer el değmedi — engine purity). host 174; +1 birim testi (yeni fact'ler + omit).
- **Slice 4 done** — koç-notu "Yeniden üret" butonu (kullanıcı AskUserQuestion'da "ML/LLM'i derinleştir" seçti). LLM
  kaynaklı/reddedilen notta görünür; tıklayınca `get_coach_narrative` re-invoke → taze LLM notu (memnuniyetsizliği TÜKETEN
  etkileşim, sadece veri-toplama değil). Renderer-only (DeepDiveTab local override state + şampiyon-değişince-reset;
  core/host el değmedi — engine purity). i18n coachNoteRegenerate/Regenerating + `.hero-detail-coach-regen` CSS. renderer 287; +2 test.
- **Slice 5+ (aday)** — like/dislike feedback'i TÜKETEN loop (re-prompt varyasyonu/model seçimi) · model preset'leri ·
  SettingsPanel buton davranış testi. **NOT: ML/LLM hızlı-kazanç yüzeyi büyük ölçüde işlendi** (Slice 1-4 done).

**MOD: Büyük geliştirme — EPIC (2026-06-17 kullanıcı direktifi).** Küçük-yüzey tükendi → en yüksek-değerli
BÜYÜK yönü Epic seç, MVP'ye böl, her tur tek dikey dilim (salt kozmetik yok). Öncelik: 1) match-history
2) lane-matchup dürüstlük 3) post-game koçluk 4) havuz gelişim 5) in-game makro. Kapsam belirsizse sorma,
makul varsay (→ DECISIONS ADR-004). Test+typecheck+desktop testleri geçmeden bitmiş sayılmaz.
- **DISCOVERY-6 done (gerçek yeniden-keşif #2, 2026-06-18)** — "hazırlanmış-ama-bağlanmamış" damarı sürdü:
  `ChampionDetail.mobility` (high/medium/low/none) + `utility_tags` (16 sabit tag) core'da arketipten hesaplanıp
  payload'da AMA hiçbir bileşende render edilmiyordu (yalnız test mock'unda). ChampionDetailCard'a mobility rozeti
  + "Takım katkısı" bölümü (etiketler WIN_LABELS desenli modül-map → ağır i18n yok; +2 bölüm-etiketi i18n key).
  Saf renderer, sıfır fabrikasyon (KB). +1 test. renderer 282 + typecheck 0 + i18n parite. Ajan yanlış-alarmları
  (heroCard.scoreBreakdown/dataSources i18n = defaultValue fallback, kozmetik → ertelendi).
- **DISCOVERY-5 done (gerçek yeniden-keşif, 2026-06-18)** — bilinen thin-adaylar tükendiğinde 2 Explore ajanı +
  koddan-teyit ile gerçek boşluk arandı. BULUNDU: `DraftPlan.blind_pick_safety` + `execution_difficulty` core'da
  hesaplanıp payload'a giriyor AMA hiçbir champ-select yüzeyinde render edilmiyordu; `draftPlan.{pickSafety,
  execDifficulty,safetySafe/Medium/Risky,execEasy/Medium/Hard}` i18n bantları DA hazırdı (kullanılmıyordu) →
  hazırlanmış-ama-bağlanmamış özellik. DeepDiveTab'a "Pick profili" bölümü (bantlı etiketler, eşik 0.6=core
  BLIND_SAFE_THRESHOLD). Saf renderer, sıfır fabrikasyon (KB-türevli), +1 i18n key (pickProfile) + 2 test.
  renderer 281 + typecheck 0 + i18n parite. Ajan yanlış-alarmları (combo-history/macro-timeline=veri yok) elendi.
- **EPIC #5: In-game Overlay Makro/Objective** (renderer; core/host DEĞİŞMEZ).
  - **Slice 1 done** — objective mutlak doğuş saati (@mm:ss) geri sayımın altında. `next_spawn_secs` zaten
    payload'da (macro_timers.rs:56, generated ObjectiveTimer) ama IngameView yalnız `countdown(seconds_until)`
    gösteriyordu. `gameClock` modül-export + birim test; `overlay.spawnAtHint` tr/en. state≠up koşullu.
    renderer 272 + typecheck 0 + i18n parite. core/host değişmedi. Lider seçimi: geniş kitle (tüm in-game
    kullanıcısı) + minik + sıfır-risk → öğrenme-progress'ten (dar kitle, orta efor) önce.
- **EPIC #4: Havuz Gelişim Sistemi** (renderer+host-query; core DEĞİŞMEZ).
  - **Slice 1 done** — öğrenme-hedefi ilerleme kartı: kullanıcının "Öğreniyorum" işaretlediği
    (ChampionDetailCard.tsx:95-96) şampiyonların son-30g mastery ilerlemesi PoolBuilder "Öğrenme hedeflerin"
    bölümünde ("+N puan · Sv X" / işaretli-ama-hareket-yok). Yeni host `getLearningProgress`
    (user_preferences.learning ⋈ mastery_snapshots; gained>=0 dahil) + `get_learning_progress` kaydı +
    i18n poolCoach.learningTitle/learningGain/learningNoMove (tr/en). `.pool-progress` CSS yeniden kullanıldı
    (yeni CSS yok). Core değişmedi. renderer 273 + host 164 + typecheck 0 + i18n parite; player komutlarının ilk testi.
  - **Slice 2 done** — öğrenme hedefinde gerçek maç-sonucu: kart artık mastery-puanı yanında o hedefte son-30g
    oynanan **maç sayısı + WR** gösterir ("N maç · %WR"). Host `getLearningProgress` aynı pencerede `matches`'ten
    `games_played`+`wins` ekler (additive; mastery-snapshot'lı hedef seti korunur). İnce-örneklem dürüstlüğü:
    games≥3 → WR; 1–2 maç → yalnız sayı (B-18 emsali); games=0 → alt-satır gizli. recommend→işaretle→pratik→SONUÇ
    döngüsünü kapatır. i18n learningGames/learningWinRate (tr/en) + `.pool-progress__sub` nötr stil. Core DEĞİŞMEDİ.
    host 164 + renderer 273 + typecheck 0 + i18n parite. → ADR-007.
  - **Slice 3 (aday, ertelendi)** — havuz-derinlik/kapsam ZAMAN-İÇİ trendi (mastery_snapshots pool-aggregate). Daha büyük, ayrı tur.
- **EPIC #3: Post-game Gelişim Koçluğu** (gelişim geri-bildirimi; core DEĞİŞMEZ — engine purity).
  - **Slice 1 done** — Maç Sonu Karnesinde hedef-tutturma serisi görseli: focus_goals met/missed geçmişi ✓/✗
    dot dizisiyle (önceden yalnız streak SAYISI). Yeni host `get_focus_history` (met/missed, en yeni önce,
    superseded hariç) + GameReviewCard dot satırı + i18n `review.focusHistoryTitle`/`focusHistoryAria` (tr/en).
    Explore 7 boşluk önerdi; koddan elendi: form-per-metric TrendPanel'de, CS@10 lesson bilinçli kapalı (honest),
    off-role/combo-outcome/macro-timeline core/timeline (ertelendi → ADR-006). renderer 270 + host 162 +
    typecheck 0 + i18n parite; GameReviewCard'ın ilk testi. core değişmedi.
  - **Slice 2 done** — off-rol zayıflık kartı: İstatistik sekmesinde ana rolden (en çok oynanan) daha düşük WR'li
    off-roller ("Üst: %20 · 5 maç") en zayıf önce. KODDAN doğrulandı: ajan "core gerekir" dedi ama tamamen ölçülen
    veriyle (`matches.position`+`win`) **host-query+renderer** (core'suz) yapıldı. Host `getOffRolePerformance`
    (GROUP BY LOWER(position); ana rol ≥3 maç; off ≥3 maç + WR<ana; yoksa null) + `OffRoleCard` (`.grc-card` reuse,
    TrendPanel deseni) StatsView'da. ARAM/Arena rolsüz → 5 SR rolü filtresiyle hariç. Sıfır fabrikasyon (tüm WR ölçülen).
    i18n offRole.* (tr/en). Core DEĞİŞMEDİ. host 168 + renderer 275 + typecheck 0 + i18n parite. → ADR-008.
  - **Slice 3+ (aday, ertelendi)** — combo-outcome feedback · macro/objective timeline (timeline ingestion gerekir)
    — daha büyük/core, ayrı tur.
- **EPIC #2: Lane-Matchup Veri-Dürüstlüğü** (core read etiketi; scoring DEĞİŞMEZ — engine purity).
  - **Slice 1 done** — LaneMatchup faz-avantaj barları artık dürüstçe "KB tahmini" etiketli. Koddan doğrulandı:
    `lane_matchup_from_json` (json_api.rs) phase_advantage'ı YALNIZ arketip `power_curve`'den (`adv()`) üretiyor,
    ölçülen matchup'a hiç bakmıyor → her zaman heuristic. Eklendi: `LaneMatchup.source: String` ("kb_estimate"
    sabit; ileride "measured") + recommendation.ts `source?` + LaneMatchupPanel "KB tahmini" rozeti (tooltip'li)
    + i18n `laneMatchup.kbEstimate`/`kbEstimateHint` (tr/en). core 570 + renderer 268 + host 161 + clippy temiz +
    typecheck 0 + i18n parite. WASM rebuild (core/pkg gitignore). engine.rs/scoring.rs el değmedi.
  - **Slice 2 done (ADR-009)** — ölçülen genel WR AYRI dürüst satır. ADR-008'in reçetesi uygulandı: `champion_matchups`
    ölçülen win_rate'i LaneMatchupPanel'de "Ölçülen: %X · N maç" satırı (tone'lu) olarak gösterilir — faz barlarına
    BÖLÜNMEZ (fabrikasyon yok), barlar "KB tahmini" kalır. Core `LaneMatchup.measured_win_rate/games` (skip-if-none) +
    `TeamContextInput.matchups` (serde default, geriye-uyumlu) + lookup (games≥20 eşiği); host getLaneMatchup
    `matchupsForPosition` geçirir. **Scoring/engine EL DEĞMEDİ** (yalnız json_api presentation). i18n laneMatchup.measured
    (tr/en). core 571 + clippy + WASM + host 168 + renderer 277 + typecheck 0 + i18n parite. Kontrollü core testi.
- **EPIC: Match-History Browser** (yerel DB; cloud/yeni Riot çağrısı YOK).
  - **Slice 1 done** — "Maç Geçmişi" liste sekmesi: yeni host `get_match_history` (`recentMatches` JOIN +
    `game_reviews` EXISTS has_review) + `MatchHistoryView` (4. lobby sekmesi). Şampiyon/rol/sonuç/tarih/KDA/
    CS-dk/vision + "İncelendi" rozeti; P-07 dürüst loading/error/empty; cs null→"—". Saf host+renderer
    (core/ts-rs/WASM değişmedi). renderer 262 + host 160 + typecheck 0; tr/en parite; +4 renderer +1 host test.
    Varsayımlar ADR-004 (A1–A7). GOTCHA: played_at Unix SANİYE (×1000); getByText doğrudan-metin-düğümü → rol regex.
  - **Slice 2 done** — satıra tıkla → detay paneli: karnesi olan satır (`role="button"`+klavye) tıklanınca
    `GameReviewCard matchId={id}` ile tam karne (lines/went_right/to_fix/focus) detayda; "← Maçlara dön" listeye döner.
    Host `get_game_review` (by match_id) + GameReviewCard opsiyonel `matchId` (prop'suz "en yeni" StatsView'da korundu).
    renderer 264 + host 161 + typecheck 0; +1 host +2 renderer test; i18n `matchHistory.back`/`openReview`.
  - **Slice 3 done** — filtreler: rol + şampiyon + sonuç (3 açılır `<select>`, yalnız listede var-olan seçenekler);
    client-side, yeni fetch yok; eşleşme yoksa dürüst "Bu filtreye uygun maç yok". renderer 266 + host 161 +
    typecheck 0; +2 renderer test; i18n `matchHistory.filters/filterRole/filterChampion/filterResult/filterAll/noFilterMatch`.
  - **Slice 4 done** — özet başlığı: gösterilen (filtrelenmiş) maçların rekor+WR+toplam KDA'sı ("12G 8M · %60 ·
    2.85 KDA"). Saf renderer (zaten çekilen entry'lerden; yeni fetch yok); filtrelerle birleşince "bu şampiyonda/
    rolde rekorum" sinyali. i18n matchHistory.summary (tr/en). renderer 278 + typecheck 0 + i18n parite.
  - **Slice 5 done** — "daha fazla yükle": limit state (+20/sayfa) + buton; host `get_match_history` limit param zaten
    vardı. Tam sayfa (rows.length>=limit) → buton görünür, az dönünce gizli. "daha fazla" sırasında liste kalır
    (fetching && matches=0 yalnız ilk yükleme tam-ekran). Saf renderer + mevcut host. i18n loadMore (tr/en). renderer 279.
  - **MVP++ TAMAM** — Match-History (liste + detay + filtreler + özet + sayfalama). Sonraki Epic: lane-matchup (#2, S2'ye kadar bitti).

**MOD: Otonom ürün geliştirme (2026-06-17 kullanıcı direktifi).** Artık bugfix/audit değil — her
iterasyonda fırsat keşfet → P0/P1 yoksa en yüksek-değerli KÜÇÜK özelliği seç → uygula+test+dokümante+commit.
Keşif: 3 Explore ajanı (feature/latent-intelligence/polish) + lider koddan-çapraz-doğrulama (ajanlar olgun
kod tabanında çok "açık" abarttı; drills/win-prob/combo-history zaten render'lı → reddedildi).
- **P-01 done** — draft simülatöründe daha derin koçluk: `DraftSimulatorPanel` core'un hesapladığı ama
  gösterilmeyen `why_this_move` (her pick "Neden bu?" gerekçesi) + faktör `deltas` (chip'lerde sayısal
  büyüklük, ör. "Engage +0.17") alanlarını yüzeye çıkardı. Renderer-only, tr/en parite, +3 test. renderer 253. (bdf64ef)
- **P-03 done** — ComboBoard'da gerçek co-pick track-record: her müttefik combo'su için oyuncunun o eşle
  geçmişi (≥2 maç → "Geçmişin: N maç · %WR"), `get_combo_outcomes`'tan (HeroCard yalnız birincil combo'yu
  gösteriyordu). my-key locked analizden; eşleşmezse gizli (graceful, yanlış-veri yok). Wrapper'da memo +
  ChampSelectScreen pass-through → ComboBoard tek `trackRecord` prop. Renderer-only, tr/en parite, +3 test. renderer 256. (7ed31d6)
- **P-06 done** — Ayarlar paneli native `window.confirm` → temalı discard dialog (Agent-3 bulgusu, koddan teyitli:
  SettingsPanel:58,66 window.confirm kullanıyordu). Dirty kapatma (X/Escape/backdrop) `role="alertdialog"` gösterir;
  Escape önce onayı kapatır; footer İptal doğrudan atar. Geniş (tüm ayar kullanıcıları) + profesyonel + renderer-only.
  `.settings-confirm` CSS, tr/en `settings.keepEditing`/`discardChanges` parite, +1 test (dirty→dialog→dön/at). renderer 257.
- **P-07 done** — PoolBuilder dürüst veri-hatası durumu: öneri fetch'i (`get_pool_suggestions`) reddedilince
  sessizce "öneri yok" yerine `app.dataError` ("Veri alınamadı") gösterir (kardeş RankCard/TrendPanel/
  WeeklySummaryCard deseni; `Promise.allSettled` reject'i artık silent-empty'ye düşmüyor). 5-boyutlu DOĞRULANMIŞ
  keşif workflow'u (19 ajan, 14 aday → çekişmeli "zaten var mı" doğrulaması, 0 çürütüldü) bu turun en temiz
  küçük+güvenli+tema-uyumlu (veri-dürüstlüğü) adayıydı. `error` state + üç-durum render. Renderer-only, sıfır
  yeni i18n, +1 test. renderer 258.
- **Yeni keşif (2026-06-17) ertelenenler (koddan teyitli, "küçük" değil):** window-opacity-control (IPC yazılı
  ama `setWindowOpacity` native değil event-only → renderer CSS-listener + saydamlık tasarım kararı gerekir) ·
  lane-matchup-heuristic-badge (ORTA değer; core `ScoringContext`→`lane_matchup_from_json`'a geniş plumbing →
  engine-purity riski, ayrı tur) · poolbuilder-loading-skeleton / lobby-cards-shimmer / objective-absolute-clock /
  sound-section-relocation / lane-form & draft-winrate rozetleri (düşük değer, sıradaki turlara).
- **Aday kuyruğu (koddan teyitli; ajanlar olgun kodda abarttı):** P-02 LLM-koç "Bağlantı test et" (host-fetch,
  gizlilik copy'si ZATEN var → yalnız test-butonu kaldı, dar kitle) · P-04 klavye-kısayol yardımı (HeroCard
  selectHint kısmen kapsıyor) · P-05 lobby Performance Snapshot (GameReviewCard kısmen kapsıyor). ELENEN: pool
  progression/drills (PoolBuilder'da built), win-prob/combo-history (HeroCard'da), role-prompt copy (değeri zaten belirtiyor).
  Büyük/ertelenen: match-history browser, opponent-scouting (ToS). **Not: küçük-özellik yüzeyi büyük ölçüde tükendi.**

**Önceki yön: canlı-veri yolu sağlamlaştırma** (lider kararı 2026-06-17: roadmap #1, prod-key dış-engeli yalnız
ingestion cron'unu kapsar; okuma yolu + app-wiring + dürüstlük + test otonom-yapılabilir).
- **L-01 done** — bayat edge matchup/build confidence dürüstlüğü (c72dd1c): staleness downgrade
  (>48s→'low') yalnız rates'e uygulanıyordu; matchups+builds aynı bayat ingestion'dan 'medium'/'high'
  görünüyordu → sahte-tazelik. `stale` flag üçüne de uygulandı. TDD RED→GREEN. desktop 159 + typecheck.
> Worker okuma yolu (`/v1/{health,rates,matchups,builds}`, secret-gated ingest, constant-time auth,
> leak'siz hata, cron labeled-log) ve app edge-sync (3 endpoint, B-41 bozuk-satır guard'ları) OLGUN+test'li.

**Önceki yön: WS3 — overlay / in-game UX polish** (2026-06-17 TAMAMLANDI: W-01 power-curve viz, W-02 canlı
faz işaretçisi, W-03 hesaplanan-ama-gizli alanlar). Tüm gerçek overlay açıkları kapandı.
- **W-01 done** — in-game güç eğrisi görsel çubuğu (PowerCurveBar): core `IngamePlan`'a
  `power_early/mid/late` (91aaa12) + renderer 3-segment HUD çubuğu (64f4c27). Glance "şu an
  güçlü müyüm?" — spike_note'u görselleştirir, redundant değil. core 505 + renderer 247 +
  desktop 158 + i18n parite + WASM rebuild temiz.
- **W-02 done** — güç çubuğu canlı oyun fazına bağlandı (7eb7b6c): `currentPhase`=`macro.phase`
  (GAME_PHASES≡POWER_PHASES) → o kolon "şu an buradasın" (▾)+teal; isKnownPhase guard;
  ariaNow tr/en. Statik referans → canlı "neredeyim". renderer 250 + typecheck + i18n.
- **W-03 done** — hesaplanan-ama-gizli plan alanları yüzeye çıktı (80aea1f): `damage_profile`
  (hasar tipi) role'den sonra PlanRow; `level` KDA satırına önek (Sv/Lv). Wiring (yeni mantık
  yok, test'li PlanRow/KDA + i18n parite). renderer 250 + typecheck.

> Önceki tur (Discovery-4) KAPANDI: B-41+B-46+B-47 done; B-45 wontfix (yanlış alarm);
> B-42/B-43/B-44 deferred; B-24 wontfix (motor-e2e zaten oracle seviyesinde kapsalı).

## Açık — yüksek/orta değer (koddan teyitli)
| id | sev | dosya | özet | durum |
|---|---|---|---|---|
| ~~B-37~~ | low | worker `index.ts` | (loop-keşif) cron `scheduled` yolu `runIngestion` reddini bağlamlı loglamıyordu (HTTP yolu logluyor) → `.catch(console.error("scheduled ingest failed", e))`. Cron production birincil sürücü; sessiz hata `wrangler tail`'de görünür. +regresyon-kilidi testi (reddi yutar). worker 17 test | **done** |
| ~~B-36~~ | low | worker `ingest.ts` | (loop-keşif) readRates/readMatchups/readBuilds 3× birebir aynı patch-çözümleme bloğu (B-14 yorumu dâhil) → `resolveLatestPatch` helper'ı. Bakım tuzağı (1 yer); davranış (boş-string dâhil `patch ?? …`) korundu. worker 16 test + typecheck | **done** |
| ~~B-14~~ | **high** | worker `ingest.ts` | patch leksik-sort → `ORDER BY updated_at DESC` (recency). Worker 16 test. **deploy bekliyor** | **done (deploy bekliyor)** |
| ~~B-03~~ | med | worker `ingest.ts` + `sources.ts` | freshness sinyali UÇTAN UCA: worker readRates `updated_at` döndürür; desktop syncEdgeRates >48s bayatsa confidence'ı 'low'a düşürür → mevcut data-quality/öneri akışı dürüstçe yansıtır. worker+desktop test. **(prod: worker deploy ister)** | **done** |
| ~~B-10~~ | med | `DataStatusBadges.tsx` | noMeta artık yapısal `missing_signals` ('meta') alanını kullanır (core json_api::compute_missing_signals); `meta_score==0.3` sihirli-sabiti kalktı → ~%50.1 WR yanlış-pozitifi de giderildi. renderer 15 test | **done** |
| ~~B-11~~ | med | `useChampSelect.ts` | puuid çözülünce aktif session için recs yeni puuid'le refetch edilir (ayrı effect); boş-puuid stale öneriler kalmıyor | **done** |
| ~~B-12~~ | med | `riot/client.ts` | routingForRegion **br1→americas** eklendi (BR account-v1+match-v5 doğru host). desktop test | **done (br1)** |
| ~~B-12b~~ | low | `riot/client.ts` | `oc1→americas` (account-v1 OCE doğru host; eskiden 'europe' default → 404). test. | **done** |
| ~~B-12c~~ | low | `match-v5.ts` + `riot-sync.ts` + `client.ts` | `matchRoutingForRegion` (oc1→`sea`) eklendi; iki match-v5 caller'ı ona geçti, account-v1 `routingForRegion`→americas'ta kaldı → OCE maç-çekimi 404 yemez. desktop test | **done** |
| ~~B-13~~ | med | `sources.ts` | u.gg fallback satırları `uggPatch` (gerçek kaynak patch) ile etiketleniyor → staleness maskesi kalktı. desktop 16 sources test | **done** |
| ~~B-15~~ | med | `OnboardingWizard.tsx` | onboarding LCU sync'inden ÖNCE `sync_ddragon_champions` çağırır → şampiyon tablosu gerçek anahtarla dolar, placeholder numeric key (ikon 404) yazılmaz | **done** |

## Açık — düşük değer / cila
| id | sev | dosya | özet | durum |
|---|---|---|---|---|
| ~~B-16~~ | low | `PoolBuilder.tsx` | loading vs empty (loading state + `poolBuilder.loading`) | **done** |
| ~~B-17~~ | low | `RankCard/TrendPanel/WeeklySummaryCard` | ortak `useActiveSummonerPuuid` hook'u (retry'lı); puuid çözülünce kartlar verilerini yeniden çeker (kalıcı-null giderildi + DRY). test | **done** |
| ~~B-18~~ | low | `StatsView.tsx` | WR grafiği <3 maçlık havuzda ince-veri notu gösterir (notsuz gizleme yok) | **done** |
| ~~B-19~~ | low | `src/lib/ddragon.ts` + `App.tsx` | App mount'ta global `get_ddragon_version`→`applyDdragonVersion` (sentinel-guard'lı); tüm giriş yolları canlı patch alır. test | **done** |
| ~~B-20~~ | low | `outcomes.ts` | `pickRecorded=true` try içine alındı → DB hatasında sonraki IN_GAME event'i retry eder, eğitim etiketi kaybolmaz. retry testi | **done** |
| ~~B-21~~ | low | `lcu/websocket.ts` | reconnect catch hata sebebini loglar (`catch (err)`+warn) → cert/pin/upgrade hataları sessiz değil. davranışsal test | **done** |
| ~~B-22~~ | low | `useChampSelect.ts` + `RoleSelector.tsx` | roleSource kalıcı tercih → 'preferred' (nötr "Geçen oyundan hatırlandı" hint), yanlış "Rolü sen seçtin" kalktı | **done** |
| ~~B-23~~ | low | `DataStatusBadges.test.tsx` | noRiotKey + liveDataAge (bayat>24s) + taze=chip-yok testleri eklendi (baseTrajectory fixture). renderer 18 test | **done** |
| ~~B-24~~ | low | `recommendations.test.ts` / `engine.rs` | cold-start recs e2e — **KODDAN DOĞRULANDI: ZATEN KAPSALI → wontfix**. ✅ "noMastery chip ölü mü?" doğrulandı — DEĞİL (aday havuzu TÜM şampiyonlar+rol-filtre, engine.rs:23,73; stretch gate `comfort<0.10` eler ama cb≥0.80'li 1 stretch geçer). ❗ "Kalan motor-e2e" aslında AÇIK DEĞİL: `core/tests/recommendation_tests.rs` üç testle tam kilitliyor — `combo_backed_stretch_appears_even_with_no_mastery` (mastery'siz Orianna, Nocturne kombosu cb≥0.80 → çıkar), `stretch_pick_has_risk_note_and_one_at_most` (`comfort_score<0.10` stretch'in risk_note'u var + max-1), `no_stretch_when_no_strong_combo` (negatif). Bunlar test-oracle seviyesinde (standing: core=oracle). B-24'ün tek deltası aynı mantığı WASM/TS sınırından tekrar assert etmek → oracle kapsamını boilerplate'le KOPYALAR (churn) | **wontfix** |
| ~~B-06~~ | low | `.claude/CLAUDE.md` | Tauri→Electron güncellendi (stack/komut/klasör/kurallar; PROJECT_STATE/AGENTS/QUALITY_CHECKS'e işaret) | **done** |
| ~~B-08~~ | low | `useSummonerData.ts:83` | **wontfix (kapsanmış)**: cold-DB boş-champMap riski dar; görünür semptom (bozuk ikon) **B-01 onError fallback'iyle** çözülü, onboarding yolu **B-15** ddragon-önce-sync. Guard warm-path'i yavaşlatır + re-render riski → değer/risk düşük | **wontfix** |
| ~~B-02~~ | med | `scheduler.ts` + `data-pipeline.ts` | cold-start priming: `primeColdStartSeeds` (atomik, boş-tablo guard) DDragon source'undan HEMEN sonra (FK-valid champions) bundled offline seed'leri içe aktarır → otomatik yol artık manuel Settings sync'i beklemeden offline build/matchup kapsaması verir. **Not:** boot'ta DEĞİL (FK ON + champions boş → silent-fail); ilk edge fetch zaten 30s scheduler tick'inde. best-effort+atomik. desktop 155 test | **done** |

## Discovery-3 batch (loop, 2026-06-17 — `csa-loop-discovery-3`: 20 ajan, 15 aday → 4 doğrulanmış)
| id | sev | dosya | özet | durum |
|---|---|---|---|---|
| ~~B-38~~ | **high** | `engine.rs:110` | stretch-pick risk notu `losses = games - wins` korumasız u32 çıkarma; host SQLite `wins<=games` zorlamıyor (`SUM(win)`, CHECK yok) → bozuk satır `wins>games` → release/WASM `overflow-checks` kapalı, sessiz underflow "4294967290L" çöp not (debug panik). Not-üretimi saf `stretch_risk_note`'a çıkarıldı + `saturating_sub` (crate konvansiyonu) + 3 birim testi. core 569 test + clippy | **done** |
| ~~B-39~~ | med | `json_api.rs:332-337` | `my_pos()` Arena (queue 1700) brawl'ı ele almıyordu → `else` dalında `assigned_position` döner; renderer `applyRole` kalıcı tercih-rolünü (örn. "middle") queue-koşulsuz enjekte edince Arena session'a SR-rol sızar → satır 497 yanlış "lane_performance eksik" rozeti basar (Arena'da lane yok). Fix: `matches!(queue_id, 450\|1700)` (engine.rs `is_aram` ile hizalı). Regresyon testi (queue 1700 fixture → sinyal yok). core 570 test + clippy | **done** |
| ~~B-40~~ | med | `docs/api-key-policy.md` | stale Tauri referansları: `src-tauri/.env`, `dotenvy::dotenv()`, `tauri.conf.json` checklist, `target/release/*.exe` tarama — Electron+Node'a göçtü; gerçek mekanizma `desktop/src/main/riot/client.ts` `process.env.RIOT_API_KEY` (+ yakın `.env`). Dev-bölümü+checklist+LCU-note (`champ_select.rs`→`commands/lcu.ts`) güncellendi. Saf-doküman | **done** |

## Discovery-4 batch (loop, 2026-06-17 — `csa-loop-discovery-4`: derin tarama, 11 aday; Verify fazı session-limit → lider KODDAN self-verify)
> Not: 5 derin lane 11 aday buldu ama tüm Verify ajanları session-limit'e (2:10 reset) takıldı.
> Lider (ana döngü, subagent değil) gerçek kodu okuyarak doğruladı — standing kural zaten "koddan teyit".
| id | sev | dosya | özet | durum |
|---|---|---|---|---|
| ~~B-41~~ | med | `sources.ts` | (DB-003) u.gg+edge matchup ingestion `wins > games` (win_rate >1.0) bozuk satırı filtrelemiyordu → `champion_matchups`'a sızıp skoru şişirir. İki yola defensive guard. B-38'in upstream tamamlayıcısı. desktop 156 test + typecheck | **done** |
| ~~DB-001~~ | — | `V006__matchups.sql` | CHECK(wins<=games) migration önerisi **REDDEDİLDİ**: SQLite `ALTER TABLE ADD CONSTRAINT` desteklemez → tablo-rebuild gerekir (destructive, lane kuralı yasak). Aynı koruma B-41 ile ingestion'da sağlandı | **wontfix** |
| B-42 | low | `lcu.ts:219` | (FLOATING_PROMISE) **KODDAN DOĞRULANDI + ERTELENDİ**: agent'ın "süresiz offline felaketi" çerçevesi YANLIŞ — `start()` döngüsü (websocket.ts:91-111) `runOnce()`'ı try/catch'le sarıyor ve B-21 iç-catch'i zaten logluyor. `void this.watcher.start()` yalnız `onStatus` callback'i throw ederse veya gelecekteki korumasız bir throw'da reddeder → nadir `unhandledRejection`. Gerçek-ama-marjinal floating-promise hijyeni (`.catch`→log+status). Test: `startWsListener` watcher'ı İÇERİDE kurar (181, injectable değil); mevcut testler stub'lar (commands.test.ts:153) → düzgün test watcher-injection refactor'ı ister, marjinal değere oransız. Uydurma değer üretme | **deferred** |
| B-43 | low | `IngameView.tsx` | (TIMING_RACE_INGAME_POLLING) **KODDAN DOĞRULANDI + ERTELENDİ**: iki effect (71-87 `get_macro_state`, 91-111 `get_ingame_plan`) `alive` guard'lı ama seq-guard'sız. Ancak: macro-poll 1.5s, tam-snapshot idempotent (her tick state'i tümüyle değiştirir) → out-of-order ≤1.5s'de KENDİ-KENDİNİ düzeltir; plan-fetch ilk başarıda durur + plan maç-içi sabit. Görünür etki ihmal-edilebilir; deterministik effect-race testi ağır harness ister → oransız | **deferred** |
| B-44 | low | `ChampSelectWrapper.tsx:199` | (BAN_SUGGESTIONS_FETCH_RACE) **KODDAN DOĞRULANDI + ERTELENDİ**: ban-fetch effect'i (194-204) `cancelled` guard'sız — kardeş narratives effect'i (161-180) zaten kullanırken tutarsız. Ban-fazında dep'ler (ban sayıları) birikerek değişince out-of-order olası ama bir-sonraki-dep'te kendini düzeltir. Düzeltme küçük (3-satır guard) ama deterministik test ağır renderer-race harness'ı ister; üretim-kodu dokunuşu test'siz tam-iterasyon olmaz → şimdilik ertelendi | **deferred** |
| ~~B-45~~ | low | `recommendations.ts:47` | (IPC-PARSE-UNVALIDATED-CAST) **YANLIŞ ALARM (KODDAN DOĞRULANDI)**: `parseSessionArg` non-raw dalı `as SessionLike` cast'liyor AMA (a) sonraki tüm TS erişimleri null-safe (`session.queue_id === 450`, `local_player?.assigned_position ?? ""`) → TS-tarafı çökmez, (b) `session` tüm input'la `engine.recommendations()`'a gidip core serde'de YENİDEN doğrulanır (B-46 fırlattığını kilitledi). Runtime guard core validasyonunu tekrarlar → single-source kuralını çiğner | **wontfix** |
| ~~B-46~~ | low | `engine.test.ts` | recommendations error-path test'siz (yalnız draftVerdict kilitliydi). KODDAN DOĞRULANDI: RecommendationsInput session/weights/all_champions `#[serde(default)]` YOK → `recommendations({})` WASM sınırında `/invalid recommendations input/` fırlatır (sessizce boş liste DÖNMEZ). Saf test eklendi; #[serde(default)] eklenirse sessiz-degrade regresyonunu yakalar. desktop 157 test + typecheck | **done** |
| ~~B-47~~ | low | `sources.test.ts` | (SECONDARY_RUNES partial-perk) **KODDAN DOĞRULANDI**: `parseUggOverview` (sources.ts:148-152) `secondary_runes`'i yalnız `perks.length>=6` ise doldurur; mevcut testler yalnız tam-6-perk (dolu) ve düşük-örneklem (perk mantığına varmadan elenen) durumu kapsıyordu. Eşik-üstü 5-perk kırpık sayfası (primary üretilir ama secondary boş kalmalı, `perks[4]/perks[5]` undefined sızmamalı) test EDİLMEMİŞTİ. Saf test eklendi (off-by-one `>=5` kalkanı). desktop 158 test + typecheck | **done** |
| ~~DB-002~~ | low | `repos.ts:167` | COALESCE eksik — ama `Number(null)→0` zaten doğru sonuç veriyor → kozmetik. **Düşük değer** | wontfix |
| UNHANDLED_CATCH_SCHEDULER | — | `scheduler.ts` | circuit-breaker önerisi: efor M + "her zaman reschedule" aslında dayanıklılık (kasıt). Marjinal | wontfix |
| SAMPLE_SIZE_ZERO | — | engine.rs+2 renderer | "n=0" yanıltıcı iddiası: n=0 aslında dürüst (0 maç). Subjektif/çok-katman | wontfix |

## Discovery-2 batch (a11y/concurrency/arch — 11 doğrulanmış; verify kısmen session-limit'e takıldı)
| id | sev | dosya | özet | durum |
|---|---|---|---|---|
| ~~B-26~~ | med | `useChampSelect.ts` | fetchRecommendations seq-guard'lı: out-of-order yanıt eskiyi ezmiyor + session-null sonrası bayat recs yazılmıyor. test | **done** |
| ~~B-25~~ | med | `Toast.tsx` | toast'lar SR'a duyuruluyor: error/warning `role=alert`/assertive, info/success `role=status`/polite. test | **done** |
| ~~B-27~~ | low | `PoolBuilder.tsx` | bozuk `role=tablist` → etiketli `role=group`+`aria-pressed` toggle grubu. test | **done** |
| ~~B-28~~ | low | `SettingsPanel.tsx`+`ChampionDetailCard.tsx` | **a)** dialog `aria-labelledby`→başlık ✅ **b)** `useModalFocus`: açılışta ilk-odak + kapanışta restore ✅. test. (Tab döngü-kapanı → B-28c nice-to-have) | **done** |
| ~~B-29~~ | low | `Timer.tsx` | SVG geri sayım `role=img`+`aria-label` ("{{n}} saniye kaldı") → SR'a duyurulur. test | **done** |
| ~~B-30~~ | low | `SettingsPanel.tsx` | bölge + pencere-boyutu `<select>` `aria-label`'lı (SR-etiketli). test | **done** |
| ~~B-31~~ | low | `ConnectionBadge.tsx` | tek persistent `role=status`+`aria-live` span → bağlantı durum değişimleri SR'a duyurulur. test | **done** |
| ~~B-32~~ | low | `data-pipeline.ts` | `syncDataPipelineInner` god-function (~140 satır) → `runSource<T>` helper (5 kaynak bloğu DRY). "tekdüze değil" endişesi çözüldü: kaynağa-özgü tek fark `fn`+`message(result)` callback'i; match_v5 çok-alanlı mesaj/default sadece farklı argüman. Mevcut güçlü e2e (summary+errors+log) net olarak korudu; 155 desktop test yeşil | **done** |
| ~~B-33~~ | low | `useChampSelect.ts` | 7 kopya fetch-on-signature useEffect → `useSessionDerived` helper (~140 satır tekrar kalktı). ÖNCE derived-state güvenlik-ağı testleri eklendi (gamePlan fetch+clear, puuid-threading, list-fallback), refactor davranış-koruyarak yapıldı; 243 renderer test yeşil + typecheck temiz | **done** |
| ~~B-34~~ | low | `riot/client.ts` | puuid/matchId path segment'leri `encodeURIComponent`'li (4 URL builder) — path/query injection defensive. test | **done** |
| ~~B-35~~ | low | `useToast.ts` | auto-dismiss timer'ları ref'te tutulup unmount'ta temizlenir (sızıntı + unmounted state-update giderildi). test | **done** |

## Tamamlanan / Kapatılan
- **B-01** (done) — Image fallback: `BanIcon` + `CounterItemIcon` onError. 214 test.
- **B-05** (done) — `BuildSummary` 'none' → dürüst "build verisi yok" (+tr/en+test). 217 test.
- **B-09** (done) — DataStatusBadges cap önceliği: aksiyon-alınabilir chip'ler (meta/mastery/Riot-key/stale) cap'ten önce öne alındı (stable sort), diagnostiklerce atılmıyor. Eviction testi eklendi. 218 test.
- **B-16** (done) — `PoolBuilder` loading state: boş puuid/settling sırasında "öneri yok" yerine "Öneriler yükleniyor…" (+tr/en `poolBuilder.loading` + 2 test). 220 test.
- **B-14** (done — deploy bekliyor) — worker `readRates/readMatchups/readBuilds` patch çözümü leksik `patch DESC` → `updated_at DESC` (recency); "16.9">"16.10" bayatlık giderildi. Davranışsal regresyon testi (mock D1 ORDER BY'a saygı duyar). Worker 16 test. **NOT: prod'da etkili olması için `wrangler deploy` gerekir.**
- **B-13** (done) — `syncUgg` u.gg fallback satırlarını canlı patch yerine gerçek kaynak patch'iyle (`uggPatch.replace('_','.')`) etiketler → 1-2 patch eski u.gg verisi 'güncel' sanılıp `patch_fresh`'i yanlış true yapmıyor. Back-level davranışsal test (16.11 canlı → 16_9 servis → satır "16.9"). desktop 16 sources test.
- **B-12** (done — br1) — `routingForRegion`'a `br1→americas` eklendi (account-v1 + Match-V5 doğru bölgesel host; eskiden 'europe' default'una düşüp BR maç/öneri verisini sessizce 404'lüyordu). Test eklendi. oc1 → B-12b (routing-split gerekir).
- **B-18** (done) — `StatsView` WR bölümü, havuzdaki tüm şampiyonlar <3 maçsa sessizce gizlenmek yerine "≥3 maçlık şampiyon yok" ince-veri notu gösterir (+tr/en `stats.winRateThin` + 2 test). renderer 222 test.
- **B-20** (done) — `OutcomeTracker.onGameflowPhase`: `pickRecorded=true` koşulsuz set ediliyordu; pick-record INSERT throw ederse o maçın öneri→pick eğitim etiketi kalıcı kaybolurdu. Flag artık yalnız başarılı INSERT sonrası set edilir → sonraki IN_GAME event'i retry eder. Davranışsal retry testi (throwing-db → boş; gerçek db → kayıt). desktop 15 outcomes test.
- **B-22** (done, 6c5024c) — `roleSource` kalıcı tercihten gelen rolü 'manual' yerine yeni 'preferred' provenance'ıyla etiketler; `RoleSelector` "Rolü sen seçtin" yerine nötr "Geçen oyundan hatırlandı" gösterir (+tr/en `rolePreferredHint` + test assertion). renderer 222 test.
- **B-11** (done) — `useChampSelect`: puuid mount'ta asenkron çözüldüğünden ilk öneriler boş-puuid (kişiselleştirmesiz) hesaplanıyordu; ayrı bir effect puuid çözülünce aktif session için recs'i yeni puuid'le yeniden çeker. Davranışsal test (puuid '' → 'puuid-9' rerender → refetch). renderer 223 test.
- **B-15** (done) — Onboarding `handleDone`, LCU mastery/maç sync'inden ÖNCE `sync_ddragon_champions` (best-effort) çağırır → şampiyon tablosu gerçek anahtarlarla dolar, kullanıcının kendi şampiyonları için numeric placeholder key (ikon 404→"26" baş-harf) yazılmaz. Sıra testi (ddragon < lcu). renderer 222 test. (B-04 reopen kapandı)
- **B-07** (done — renderer) — cold-start dürüst-UI `DataStatusBadges.test.tsx`'te kapsanmış; host-tarafı e2e boşluğu B-24'e taşındı.
- ~~**B-04** wontfix~~ → **B-15 reopen**: workflow placeholder-key'in kullanıcının kendi mastery ikonlarını bozduğunu doğruladı (kendi-iyileşse de pencere gerçek).
