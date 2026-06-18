# TASKS — aktif iterasyon

> Tek seferde tek küçük görev. Tarih: 2026-06-16.

## HARDENING — geniş kalite/test sağlamlaştırma (2026-06-18, kullanıcı seçti)
> Olgun kod tabanı → odaklı Explore + koddan-teyit ile gerçek edge-case guard'ı. Ajan 2 marjinal aday buldu; ikisi de gerçek sınır-kırılganlığı.
- **Iter H-01 (done)** — buildCoachUserPrompt sınır-guard. KODDAN doğrulandı: `win_prob.sample_size`/`combo_history.wins`
  TS-zorunlu ama get_coach_narrative IPC sınırında input `as unknown as CoachNarrativeInput` cast'li (runtime doğrulanmaz);
  renderer pratikte yapısal rec geçirse de malformed input prompt'a "undefined maç" yazabilirdi. `Number.isFinite` guard'ı
  (sample_size+wins; 0 geçerli kalır, undefined/null/NaN reddedilir). Pür-defensive → davranış değişmez (CHANGELOG'a yazılmaz,
  B-47 emsali). Host-only (core el değmedi). Test: eksik-sample_size/wins → prompt'ta "undefined" yok + fact eklenmez. ✅ host 180.
- **Sıradaki:** ek hardening adayları (odaklı tarama "olgun, az-bulgu" dedi) ya da yön sorusu. Anti-churn: marjinal guard'ları zorlamadan.

## EPIC: Overlay HUD Görselleri (2026-06-18, kullanıcı AskUserQuestion ile seçti)
> Keşif: in-game ana pencerede ama overlay-modu (sağ-üst 400px+always-on-top) zaten var. Gerçek şeffaf pencere ELE (ToS, ADR-011).
- **Iter Slice-1 (done, ADR-011)** — IngameView "Kompakt HUD" toggle. KODDAN doğrulandı: HUD görselleri (PowerCurveBar,
  objective timer'lar, faz, KDA) zaten IngameView'da ama yoğun plan-metni (win/role/damage/spike/lane/wave/matchup/mid/late)
  ile 400px overlay-pencerede sıkışık. Header'a "Kompakt/Detaylı" toggle (`compact` local state); kompaktta plan-text
  satırları `{!compact && <>...</>}` ile gizli, görseller + macro kalır. `.ingame-compact-toggle` CSS (margin-left:auto →
  title sola, buton+minimize sağa), i18n overlay.compact/detailed (tr/en). Saf renderer (core/host el değmedi — windowing var).
  Test: integration (mock get_settings/get_ingame_plan/get_macro_state → detay metni görünür → Kompakt tıkla → metin gizli,
  .overlay-power kalır → Detaylı tıkla → geri). GOTCHA: useSettings get_settings mock'lanmalı (yoksa undefined.then). ✅ renderer 288.
- **Iter Slice-2 (done)** — kompakt tercihi kalıcı. KODDAN doğrulandı: ayarlar JSON blob (`app_config` key='settings'),
  kolon-migration YOK → `compact_overlay`'i host settings.ts (AppSettings+DEFAULT+getSettings optional-default, coach_llm
  deseni) + renderer useSettings.ts (AppSettings+DEFAULT_SETTINGS) + SettingsPanel checkbox (sounds_enabled deseni) +
  IngameView `useState(settings.compact_overlay)` + `useEffect([settings.compact_overlay])` senkron. Ayar açıksa in-game
  doğrudan kompakt; kullanıcı maç-içi toggle korunur. i18n settings.compactOverlay (tr/en). Host+renderer (core el değmedi).
  Test: yeni settings.test.ts (round-trip + eski-ayar optional-default, diğerleri korunur) + IngameView init-compact
  (get_settings compact_overlay:true → "Detaylı" toggle + plan metni gizli). ✅ host 178 + renderer 289 + typecheck 0.
  GOTCHA: testler DEFAULT_SETTINGS spread kullanıyor → AppSettings'e alan eklemek mevcut mock'ları bozmadı.
- **Iter Slice-3 (done)** — kompakt-HUD cilası. Kullanıcı AskUserQuestion'da "Overlay HUD'u cilala" seçti (düşük-değer
  flag'lendi; kullanıcı talebi anti-churn'ü geçer). Kompaktta plan-head rol/"vs" metin etiketleri `{!compact && ...}` ile
  gizli (ikon/isim/KDA kalır — ikonlar bağlam taşır) + `.ingame-view--compact` sıkı gap/padding (token'lı, öğe/renk aynı).
  Saf renderer/CSS (core/host el değmedi). Test: compact toggle testi rol-etiketi ("Üst") detayda görünür/kompaktta gizli.
  ✅ renderer 289 + typecheck 0.
- **Sıradaki:** Overlay HUD ToS-güvenli yüzeyi TÜKENDİ (sonrası kozmetik). Yön sorusu: canlı-veri / kalite-sağlamlaştırma / başka.

## EPIC: ML/LLM Koçluk Fazı (2026-06-18, kullanıcı AskUserQuestion ile seçti)
> Yön soruldu (güvenli küçük-yüzey tükendi) → ML/LLM seçildi. Keşif: pipeline ZATEN tam bağlı+test'li.
- **Iter Slice-1 (done, ADR-010)** — koç-notu kaynak şeffaflığı rozeti. KODDAN doğrulandı: LLM koçluk pipeline'ı
  zaten TAM (core coach_narrator+validate_external audit; host llm-narrator.ts OpenAI-uyumlu fetch + coach-narrative.ts;
  settings coach_llm_endpoint/model + UI; DeepDiveTab render; 6 test) AMA `CoachNarrative.source`/`external_rejected`
  render edilmiyordu. DeepDiveTab koç-notu başlığına rozet: external→"LLM", rejected→"LLM reddedildi"(tooltip), düz
  deterministik→yok. i18n heroCard.coachNoteLlm/coachNoteRejected/Hint (tr/en). `.hero-detail-coach-badge` CSS. Saf
  renderer (motor purity korunur). +3 test (external/rejected/düz). ✅ renderer 285 + typecheck 0 + i18n parite.
- **Iter Slice-2 (done)** — Settings "Bağlantı test et" butonu. KODDAN doğrulandı: host `fetchLlmCandidate`/`FetchFn`
  seam zaten var. Yeni host `testCoachLlm` (minimal "ping" + max_tokens 1 → kullanıcı/oyun verisi YOK; OpenAI-uyumlu
  `choices` array doğrulaması; `{ok, reason: empty|http|bad_response|network}`; default fetch=globalThis.fetch) +
  `test_coach_llm` ipc kaydı + SettingsPanel buton (mevcut endpoint/model input'larının altına; dirty-state'i etkilemez,
  draft endpoint'i test eder) + ✓/✗ durum + `.sp-llm-*` CSS + i18n (coachLlmTest/Testing/Ok/Fail tr/en). Engine purity
  (core el değmedi). Test: coach-narrative.test.ts'e 5 testCoachLlm testi (ok/http/bad/network/boş-endpoint, mock FetchFn).
  ✅ host 173 + renderer 285 + typecheck 0 + i18n parite. GOTCHA: cwd kaymışsa pnpm/git MUTLAK `-C` yol.
- **Iter Slice-3 (done)** — zengin + güvenli grounded promptlar. KODDAN doğrulandı: ChampSelectWrapper:181 tam `rec`
  geçiriyor → enemy_team_summary/phase_matchup/missing_signals prompt'a ulaşır. CoachRecFacts genişletildi +
  buildCoachUserPrompt'a 3 fact: "Rakip kompo: …", "Faz avantajın: erken/orta/geç ~%…", "Veri boşluğu (bunlar hakkında
  iddia ETME): …" (anti-halüsinasyon). Prose plan'lar (mid/late_plan) EKLENMEDİ — zaten koçluk metni, LLM'e vermek dairesel.
  Host-only (core/renderer el değmedi — engine purity). Test: llm-narrator.test.ts'e Slice-3 fact testi + omit-absence.
  ✅ host 174. GOTCHA: cwd kaymışsa pnpm/git MUTLAK `-C`.
- **Iter Slice-4 (done)** — koç-notu "Yeniden üret" (kullanıcı AskUserQuestion'da "ML/LLM'i derinleştir" seçti). DeepDiveTab'a
  local `regenerated` override state + `regenerate` handler (`invoke('get_coach_narrative', {recommendation: rec, win_prob,
  combo_history})` → taze not) + buton; `canRegenerate = source==='external' || external_rejected` (düz deterministik'te gizli —
  yeniden-üretmek aynı sonucu verir); şampiyon değişince override sıfırlanır (useEffect [champion_id]). i18n coachNoteRegenerate/
  Regenerating + `.hero-detail-coach-regen` CSS. Renderer-only (core/host el değmedi — engine purity; get_coach_narrative idempotent
  re-invoke). Test: regenerate tıkla→yeni-not + düz-deterministik'te buton-yok (global host-mock src/test/setup.ts). ✅ renderer 287.
- **Iter Slice-5 (done)** — "Yeniden üret" gerçekten farklı not üretir. `vary` bayrağı uçtan uca: DeepDiveTab regenerate
  invoke `vary:true` → CoachNarrativeInput.vary → getCoachNarrative `fetchLlmCandidate(...,6000,input.vary)` →
  fetchLlmCandidate `buildCoachUserPrompt(rec,faz3,vary)` → vary ise closing "öncekinden FARKLI bir açıdan, farklı kelimelerle".
  Örtük "beğenmedim"i TÜKETEN en küçük gerçek loop. Host+renderer (core el değmedi — engine purity; mevcut çağrılar vary=false
  default ile bozulmadı). Test: buildCoachUserPrompt vary hint + fetchLlmCandidate body pass-through (capture FetchFn) +
  DeepDiveTab vary:true assertion. ✅ host 176 + renderer 287 + typecheck 0.
- **Sıradaki:** ML/LLM derinleştirme yüzeyi büyük ölçüde işlendi (Slice 1-5). Slice 6 (model-preset/açık-feedback) düşük-orta.
  Muhtemel: dur + AskUserQuestion (ML/LLM-devam mı / canlı-veri prod-key / overlay HUD).

## Discovery-6 — gerçek yeniden-keşif #2 (2026-06-18)
> "Hazırlanmış-ama-bağlanmamış" damarı sürdü (Explore + koddan-teyit).
- **Iter (done)** — ChampionDetailCard "Hareketlilik + Takım katkısı": KODDAN doğrulandı — `ChampionDetail.mobility`
  (high/medium/low/none) + `utility_tags` (16 sabit tag) core'da arketipten hesaplanıp payload'da AMA hiçbir
  bileşende render edilmiyordu (recommendation.ts:363,366 tipte var; yalnız test mock'unda). Karta mobility rozeti
  (.cdc-badges'e) + "Takım katkısı" bölümü (.cdc-badge reuse). Etiketler WIN_LABELS desenli modül-map
  (MOBILITY_LABELS/UTILITY_LABELS, locale-agnostic LoL-jargon + birkaç TR) → ağır i18n yok; +2 bölüm-etiketi
  (champDetail.mobility/utility tr/en). Saf renderer (core/host/CSS değişmedi). +1 test (mobility 'high' + engage/
  frontline). ✅ renderer 282 + typecheck 0 + i18n parite. Sıfır fabrikasyon (KB deterministik). GOTCHA: Bash `cd`
  cwd'yi değiştirdi → pnpm/git için MUTLAK `-C` yol kullan (yoksa champ-select-assistant/champ-select-assistant ENOENT).
- **Sıradaki:** yüzey gerçekten tükendiyse churn üretme — kullanıcıya yön sorusu (ML/LLM, canlı-veri prod-key, overlay HUD).

## Discovery-5 — gerçek yeniden-keşif (2026-06-18)
> Bilinen thin-aday listesi tükendi → 2 Explore ajanı + koddan-teyit ile TÜM kod tabanı tarandı.
- **Iter (done)** — DeepDiveTab "Pick profili": KODDAN doğrulandı — `DraftPlan.blind_pick_safety` (0-1, KB) +
  `execution_difficulty` (1-5, KB) core'da hesaplanıp payload'da AMA hiçbir champ-select bileşeninde render
  edilmiyordu (yalnız test mock'larında); `draftPlan.*` bant i18n anahtarları DA hazır-ama-kullanılmıyordu →
  hazırlanmış özelliğin render'ı tamamlandı. DeepDiveTab'a bantlı etiket bölümü (blindSafetyLabel eşik 0.6=core
  BLIND_SAFE_THRESHOLD/compute_blind_unsafety; execDifficultyLabel 1-2 kolay/3 orta/4-5 zor). Saf renderer
  (core/host/CSS değişmedi; hero-card__quick-tags reuse). +1 i18n (pickProfile) + 2 test (riskli/zor + güvenli/kolay).
  ✅ renderer 281 + typecheck 0 + i18n parite. Sıfır fabrikasyon (KB deterministik). → CHANGELOG/BACKLOG Discovery-5.
- **Sıradaki:** yüzey gerçekten tükendiyse churn üretme — kullanıcıya yön sorusu (ML/LLM, canlı-veri prod-key, overlay HUD).

## Büyük geliştirme modu — EPIC #4: Havuz Gelişim Sistemi (2026-06-17)
> Mesaj-beklemeden otonom devam. In-game Makro S1 bitti → priority-#4 havuz gelişim.
- **Iter Slice-1 (done)** — öğrenme-hedefi ilerleme kartı. Host `getLearningProgress` (player.ts; user_preferences
  preference='learning' ⋈ mastery_snapshots, gained>=0 dahil, championKeyMap key, gain DESC) + `get_learning_progress`
  kaydı (ipc.ts). PoolBuilder: `learning` state + fetch (mastery effect'ine eklendi) + "Öğrenme hedeflerin" bölümü
  (`.pool-progress` deseni yeniden kullanıldı → yeni CSS yok; gain>0 "+N puan·Sv X", gain=0 "işaretli—hareket yok";
  boşsa gizli). i18n poolCoach.learningTitle/learningGain/learningNoMove (tr/en). Engine purity (core el değmedi).
  Test: yeni player.test.ts getLearningProgress (learning-filtre/0-gain/never+tercihsiz-hariç/boş) + PoolBuilder render.
  ✅ renderer 273 + host 164 + typecheck 0 + i18n parite. (commit hazırlanıyor)
- **Iter Slice-2 (done)** — öğrenme hedefine gerçek maç-sonucu (games/WR). Host `getLearningProgress` aynı `days`
  penceresinde `matches`'ten ikinci GROUP BY ile `games_played`+`wins` ekler (JS merge; mastery-snapshot'lı hedef
  seti korunur — additive). PoolBuilder: ikinci nötr alt-satır (`.pool-progress__sub`) — games≥3 "N maç · %WR",
  1–2 maç "N maç" (ince-örneklem dürüstlüğü, WR uydurulmaz), games=0 gizli. i18n poolCoach.learningGames/
  learningWinRate (tr/en, `{{n}}`/`{{wr}}`). Engine purity (core el değmedi). Test: player.test.ts'e matches seed +
  games/wins + pencere-dışı-hariç assert'leri; PoolBuilder.test.tsx WR/sayı/gizleme assert'leri. ADR-007.
  ✅ renderer 273 + host 164 + typecheck 0 + i18n parite. (commit hazırlanıyor)
- **Sıradaki:** Epic #4 Slice 3 (havuz-derinlik zaman-trendi — büyük) ya da kalan Epic Slice-2'leri (Lane #2 ölçülen-plumbing / In-game #2 teamfight-note).

## Büyük geliştirme modu — EPIC #5: In-game Overlay Makro/Objective (2026-06-17)
> Mesaj-beklemeden otonom devam. Post-game S1 bitti → koddan-doğrulanmış temiz in-game adayı.
- **Iter Slice-1 (done)** — objective satırına mutlak doğuş saati (@mm:ss). Koddan doğrulandı: `next_spawn_secs`
  zaten ObjectiveTimer payload'ında (macro_timers.rs:56) ama IngameView:219 yalnız `countdown(seconds_until)`
  gösteriyordu. `gameClock(secs)` modül-export (mm:ss + negatif→0:00) + IngameView render (state≠up && seconds_until>0)
  + `.overlay-objective-timing` grid hücresi (3-kolon korundu) + i18n `overlay.spawnAtHint` (tr/en) + gameClock testi.
  Renderer+i18n-only (core/host el değmedi). ✅ renderer 272 + typecheck 0 + i18n parite. (commit hazırlanıyor)
- **Lider seçimi:** geniş kitle (tüm in-game) + minik + sıfır-risk → priority-#4 öğrenme-progress'ten önce (dar kitle/orta efor).
- **Sıradaki:** Epic #4 Slice 1 (öğrenme-hedefi ilerleme kartı — koddan doğrulandı, getLearningProgress host + PoolBuilder bölümü).

## Büyük geliştirme modu — EPIC #3: Post-game Gelişim Koçluğu (2026-06-17)
> Mesaj-beklemeden otonom devam. Lane-Matchup Slice 1 bitti → öncelik #3 post-game gelişim koçluğu.
- **Keşif** — 1 Explore ajanı (post-game envanter) + lider koddan-doğrulama. 7 boşluk önerildi; ELENENLER:
  form-per-metric (TrendPanel'de zaten), CS@10 lesson (postgame.rs bilinçli kapalı — jungle/support honest),
  off-role/combo/macro (core/timeline, büyük). SEÇİLEN: hedef-tutturma serisi görseli (veri hazır, küçük, core'suz).
- **Iter Slice-1 (done)** — GameReviewCard'a hedef-tutturma serisi (✓/✗ dot). Host `getFocusHistory` (focus_goals
  met/missed, en yeni önce, superseded hariç) + `get_focus_history` kaydı. GameReviewCard: `focusHistory` state +
  fetch + dot satırı (reverse → eski→yeni; role=img+aria, dot=aria-hidden+tooltip label). CSS `.grc-goal-dot`.
  i18n review.focusHistoryTitle/Aria (tr/en). Core DEĞİŞMEDİ (engine purity). Testler: host getFocusHistory
  (sıralama/superseded-hariç/limit/grup) + GameReviewCard ilk testi (dot sayısı/met-missed). ✅ renderer 270 +
  host 162 + typecheck 0 + i18n parite. (commit hazırlanıyor)
- **Iter Slice-2 (done)** — off-rol zayıflık kartı. KODDAN doğrulandı: Lane S2 (ölçülen plumbing) ELENDİ — tek genel
  win_rate'i 3 faz-barına bölmek fabrikasyon (ADR-008); In-game S2 zaten render'lı (iş yok); Havuz S3 soyut. SEÇİLEN:
  off-rol zayıflık — tamamen ölçülen veri, host-query+renderer (ajan "core" dedi, koddan core'suz yapıldı). Host
  `getOffRolePerformance` (matches GROUP BY LOWER(position); ana rol=en-çok-oynanan ≥3 maç; off=ana-dışı ≥3 maç +
  WR<ana, en zayıf önce; yoksa null) + `get_off_role_performance` kaydı. Renderer `OffRoleCard` (`.grc-card` reuse)
  StatsView'da TrendPanel'den sonra. i18n offRole.title/mainRole/hint/roleStat (tr/en). Engine purity (core el değmedi).
  Test: player.test.ts 4 off-role testi (zayıf-flag/ince-hariç/güçlü-hariç/main-ince-null/ARAM-null) + OffRoleCard.test.tsx
  (render/dürüst-gizle). ✅ host 168 + renderer 275 + typecheck 0 + i18n parite. (commit hazırlanıyor)
- **Sıradaki:** kalan Slice-2/3'ler — Lane S2 (dürüst ölçülen-WR ayrı satır, ADR-008) / Havuz S3 (derinlik-trendi) / combo-outcome feedback.

## Büyük geliştirme modu — EPIC #2: Lane-Matchup Veri-Dürüstlüğü (2026-06-17)
> Mesaj-beklemeden otonom devam. Match-History Epic bitti → öncelik #2 lane-matchup dürüstlük.
- **Iter Slice-1 (done)** — LaneMatchup barlarına "KB tahmini" kaynak etiketi. Koddan doğrulandı: phase_advantage
  yalnız arketip power_curve'den (`adv()`), ölçülen matchup'a bakmıyor → hep heuristic. core `LaneMatchup.source`
  ("kb_estimate") + recommendation.ts `source?` + LaneMatchupPanel rozeti (tooltip) + i18n kbEstimate/Hint (tr/en)
  + core/renderer testleri. Engine purity korundu (yalnız read etiketi; engine.rs/scoring.rs el değmedi). WASM rebuild
  (core/pkg gitignore). ✅ core 570 + renderer 268 + host 161 + clippy + typecheck 0 + i18n parite. (commit hazırlanıyor)
- **Iter Slice-2 (done, ADR-009)** — ölçülen genel WR AYRI dürüst satır. KODDAN doğrulandı: ADR-008'in faz-fabrikasyon
  tuzağından kaçınıldı — tek genel win_rate faza bölünmez, ayrı "Ölçülen: %X · N maç" satırı; faz barları "KB tahmini"
  kalır. Plumbing parçaları hazırdı (host `matchupsForPosition`, core `MatchupKeyEntry`/`MatchupEntry`). Core:
  `TeamContextInput.matchups` (serde default) + `LaneMatchup.measured_win_rate/games` (skip-if-none) + lane_matchup_from_json
  lookup (my_id/opp_id eşleşmesi + games≥20). Host: getLaneMatchup context'e `matchupsForPosition(db,myPos)`. Renderer:
  recommendation.ts interface + LaneMatchupPanel tone'lu satır + `.lane-matchup__measured` CSS. i18n laneMatchup.measured/
  measuredHint (tr/en). **Engine purity** (json_api presentation; scoring/engine.rs/recommendation.rs el değmedi). Test:
  kontrollü core (Garen vs Darius; 2200→görünür/5→gizli/yok→gizli) + LaneMatchupPanel 2 test. ✅ core 571 + clippy + WASM +
  host 168 + renderer 277 + typecheck 0 + i18n parite. (commit hazırlanıyor)
- **Sıradaki:** kalan Slice-3'ler — Havuz S3 (derinlik-trendi) / combo-outcome post-game feedback / Lane S3 (matchup tips zenginleştirme).

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
- **Iter Slice-4 (done)** — özet başlığı: MatchHistoryView'da gösterilen (filtrelenmiş) maçların rekor+WR+toplam
  KDA'sı (`{{wins}}G {{losses}}M · %{{wr}} · {{kda}} KDA`). Saf renderer — `filtered`'tan hesaplanır (yeni host
  çağrısı yok); filtrelerle birleşince "bu şampiyon/rolde rekorum". i18n matchHistory.summary (tr/en). Test: özet
  hem genel hem filtre-sonrası doğrulanır (locale-bağımsız: KDA toFixed(2)). ✅ renderer 278 + typecheck 0 + i18n parite.
- **Iter Slice-5 (done)** — "daha fazla yükle": MatchHistoryView'a `limit`/`hasMore` state + buton; host
  `get_match_history` limit param zaten vardı → saf renderer. `fetching && matches=0` yalnız ilk yüklemede tam-ekran
  (load-more sırasında liste kalır); `hasMore = rows.length >= limit` (tam sayfa → daha var). i18n matchHistory.loadMore
  (tr/en). Test: ilk sayfa 20→buton+20 satır, tıkla→25 satır+buton kaybolur. ✅ renderer 279 + typecheck 0 + i18n parite.
- **EPIC MVP++ TAMAM (Match-History):** liste + detay + filtre + özet + sayfalama. Lane-matchup #2 de S2'ye kadar bitti (ADR-009).

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
