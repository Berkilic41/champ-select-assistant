# DECISIONS — mimari/teknik karar günlüğü (ADR-lite)

> Format: bağlam → karar → gerekçe → sonuç. En yeni üstte.

## ADR-011 — Overlay HUD Epic: gerçek şeffaf pencere ELE (ToS); Slice 1 = kompakt HUD toggle (2026-06-18)
- **Bağlam:** ML/LLM derinleştirme hızlı-kazançları bitince AskUserQuestion → kullanıcı **"Overlay HUD görselleri"**
  seçti. Keşif (Explore + koddan-teyit): in-game görünüm (IngameView) ANA pencerede render ediliyor AMA zaten bir
  "overlay modu" var — `set_overlay_mode` maça girince pencereyi sağ-üst 400×720 + always-on-top yapıyor (window.ts).
  HUD görselleri (PowerCurveBar, objective timer'lar, faz chip, KDA) ZATEN IngameView'da, ama yoğun metin plan-satırları
  (win/role/damage/spike/lane/wave/matchup/mid/late) ile birlikte → 400px'te sıkışık, glance-edilemiyor.
- **Karar:** Gerçek şeffaf/click-through Electron overlay penceresi (transparent BrowserWindow + setIgnoreMouseEvents)
  **ELENDİ**: yüksek sürtünme (ayrı pencere yaşam-döngüsü, state sync, oyun-penceresi takibi) + **ToS/anti-cheat riski**
  (şeffaf always-on-top injection gibi görünebilir). İlk dilim = **kompakt HUD toggle**: IngameView'da yoğun plan metnini
  gizleyip yalnız glanceable görselleri (header/KDA, güç eğrisi, objective'ler, faz) bırakan yerel toggle. Mevcut overlay-mode
  windowing'i (sağ-üst yüzen pencere) gerçek bir HUD'a çevirir.
- **Gerekçe:** Geniş kitle (tüm in-game), saf renderer (core/host EL DEĞMEZ — windowing zaten var), ToS-güvenli (yeni
  pencere/şeffaflık yok; yalnız CSS/koşullu render), mevcut görselleri (PowerCurveBar) yeniden kullanır. Fabrikasyon yok.
- **Sonuç:** IngameView `compact` toggle + `.ingame-compact-toggle` + i18n overlay.compact/detailed (tr/en); plan-text
  satırları `{!compact && …}`. renderer 288 + typecheck 0 + i18n parite. Sonraki dilim adayları: kompakt tercihini
  kalıcılaştır (setting) · in-game'e girince auto-kompakt · daha sıkı kompakt-layout CSS.

## ADR-010 — ML/LLM Koçluk Epic: pipeline ZATEN bağlıymış; Slice 1 = kaynak şeffaflığı (2026-06-18)
- **Bağlam:** Güvenli küçük-geliştirme yüzeyi tükenince kullanıcıya AskUserQuestion ile yön soruldu → **"ML/LLM
  koçluk fazı"** seçildi. Keşif (Explore + koddan-teyit) ŞAŞIRTICI bulgu: LLM koçluk pipeline'ı ZATEN TAM BAĞLI +
  test'li: core `coach_narrator::narrate` + `validate_external` audit (boy/abartı/grounding); host `llm-narrator.ts`
  (OpenAI-uyumlu fetch, 6s timeout) + `coach-narrative.ts`; settings `coach_llm_endpoint/model` + SettingsPanel UI;
  DeepDiveTab "Koç notu" render'ı; 6 host testi. MVP çalışıyor: Settings'e endpoint gir → audit'li LLM notu + hata/
  timeout/red → deterministik fallback. **Motor purity korunuyor by-design** (LLM yalnız anlatım seam'i; scoring DEĞİŞMEZ).
- **Karar:** Epic'in ilk dikey dilimi = **kaynak ŞEFFAFLIĞI**. `CoachNarrative.source` ("external"/"deterministic")
  + `external_rejected` HESAPLANIYOR ama hiçbir yerde render edilmiyordu (computed-but-unrendered). DeepDiveTab koç-notu
  başlığına rozet: source="external" → "LLM"; external_rejected → "LLM reddedildi" (tooltip: audit'i geçemedi→deterministik);
  düz deterministik (varsayılan) → ROZET YOK (gürültü değil). Renderer+i18n+CSS; core/host EL DEĞMEDİ.
- **Gerekçe:** ML/LLM fazının GÜVEN temeli = kullanıcı notun AI mı, deterministik mi, yoksa LLM-reddedilip-fallback mı
  olduğunu görmeli (veri-dürüstlüğü DNA'sı: measured/estimate, missing_signals hattı). Pipeline zaten çalıştığından en
  yüksek değer ŞEFFAFLIK; sahte-değer/scaffolding-churn değil. Rozet yalnız LLM dahil olduğunda görünür (varsayım: düz
  deterministik notta rozet basmak gürültü olur — çoğu kullanıcının LLM'i yok).
- **Sonuç:** renderer 285 + typecheck 0 + i18n parite. Sonraki Slice adayları: (2) Settings "Bağlantı test et" butonu
  (P-02; kurulum sürtünmesi), (3) daha zengin grounded promptlar / ek fact'ler, (4) LLM-notu için kullanıcı geri-bildirimi.

## ADR-009 — Lane-Matchup Epic #2 Slice 2: ölçülen WR AYRI dürüst satır (faz barları tahmin kalır) (2026-06-17)
- **Bağlam:** ADR-008 Lane S2'nin doğru yolunu reçete etti: ölçülen tek genel win_rate'i faz barlarına BÖLME
  (fabrikasyon); ayrı dürüst satır göster. Plumbing parçaları zaten vardı: host `matchupsForPosition` (recommendations
  bunu kullanıyor), core `MatchupKeyEntry`/`MatchupEntry {win_rate, games}`. `lane_matchup_from_json` zaten my_id/
  opp_id/my_pos hesaplıyordu — eksik tek şey input'a matchups'ı geçirmek + lookup.
- **Karar:** `TeamContextInput`'a `matchups: Option<Vec<MatchupKeyEntry>>` (serde default — diğer team-context
  uçları None'la geriye-uyumlu); `LaneMatchup`'a `measured_win_rate`/`measured_games` (skip_serializing_if). Lookup:
  (my_id, opp_id) eşleşmesi + `games >= MEASURED_MATCHUP_MIN_GAMES=20` (altı gürültü → gizli). Host getLaneMatchup
  context'e `matchupsForPosition(db, myPos)` ekler. LaneMatchupPanel ayrı `lane-matchup__measured` satırı (tone'lu);
  **faz barları source="kb_estimate" KALIR**.
- **Gerekçe:** Gerçek ölçülen sinyal (öncelik #2 tema), **sıfır fabrikasyon** (genel oran genel olarak gösterilir,
  faza bölünmez), örneklem-eşikli dürüstlük (B-18 hattı), **engine purity korunur**: yalnız `json_api.rs` presentation
  read-side değişti — `scoring.rs`/`engine.rs`/`recommendation.rs` (compute_recommendations) EL DEĞMEDİ, skor değişmez.
- **Sonuç:** core 571 (506 lib + 65 entegrasyon) + clippy temiz + WASM rebuild + host 168 + renderer 277 + typecheck 0
  + i18n parite. Kontrollü core testi (Garen vs Darius fixture, games 2200→görünür / 5→gizli / matchups-yok→gizli).
  recommendation.ts measured alanlarını elle aynalar (LaneMatchup ts-rs DEĞİL — `source?`/`inferred?` deseni).

## ADR-008 — Post-game Epic #3 Slice 2: off-rol zayıflık kartı; Lane S2 dürüstlük tuzağı nedeniyle elendi (2026-06-17)
- **Bağlam:** Kalan Slice-2 adayları koddan-doğrulandı. **Lane-Matchup S2 (ölçülen plumbing) ELENDİ:** ölçülen
  matchup yalnız TEK genel win_rate verir; bunu 3 ayrı faz-barına (erken/orta/geç) bölmek **fabrikasyon** olur
  (faz-bazlı ölçüm yok) → veri-dürüstlüğü DNA'sını bozar (ADR-005 zaten barların "tahmin" olduğunu söylüyor).
  In-game S2 (teamfight-note): tüm IngamePlan alanları zaten render'lı → iş yok. Havuz S3: daha soyut/düşük değer.
- **Karar:** Off-rol zayıflık kartı (Post-game #3 S2). Ajan "core gerekir" dedi ama KODDAN doğrulandı: tamamen
  ölçülen veriyle (`matches.position`+`win`) **host-query+renderer**, core'a hiç dokunmadan yapılabilir (daha temiz).
  Host `getOffRolePerformance`: `matches` GROUP BY LOWER(position); ana rol = en çok oynanan; off-roller = ana-rol
  dışı, ≥3 maç, **ana rolden DÜŞÜK WR'li** (en zayıf önce); anlamlı zayıflık yoksa null. `OffRoleCard` StatsView'da
  (`.grc-card` yeniden kullanıldı, TrendPanel deseni). ARAM/Arena rolsüz → 5 SR rolü filtresiyle doğal hariç.
- **Gerekçe:** Yüksek değer ("hangi rol seni aşağı çekiyor?"), **sıfır fabrikasyon riski** (tüm WR ölçülen),
  **engine purity korunur (core el değmedi)**, host-query+renderer (en düşük risk). Lane S2'nin faz-tuzağının tersi.
- **Sonuç:** host 168 + renderer 275 + typecheck 0 + i18n parite yeşil. core/WASM el değmedi. Lane S2 ileride
  yapılırsa: ölçülen genel WR'yi AYRI dürüst satır olarak göster (faz barlarını "KB tahmini" bırak) — fabrikasyon yok.

## ADR-007 — Havuz Gelişim Epic #4 Slice 2: öğrenme hedefine gerçek maç-sonucu (games/WR) (2026-06-17)
- **Bağlam:** S1 öğrenme-hedefi kartı yalnız mastery-puanı kazancını gösteriyordu. Mastery-puanı "grind"i (oynama
  süresi) ölçer ama pratiğin işe yarayıp yaramadığını söylemez — oyuncu çok oynayıp hâlâ kaybediyor olabilir.
  `matches` tablosu puuid+champion_id+played_at indeksli ve `win IN (0,1) NOT NULL` → gerçek sonuç hazır.
- **Karar:** `getLearningProgress` aynı `days` penceresinde her hedef için `matches`'ten `games_played`+`wins`
  ekler (mastery snapshot'lı hedef seti korunur — additive); PoolBuilder kartında ikinci nötr alt-satır gösterir.
  **İnce-örneklem dürüstlüğü:** `games_played >= 3` → "N maç · %WR"; 1–2 maç → yalnız "N maç" (gürültülü %0/%100
  WR uydurulmaz, B-18 StatsView emsali); `games_played == 0` → alt-satır hiç gösterilmez (dürüst gizleme).
- **Gerekçe:** recommend→işaretle→pratik→**SONUÇ** döngüsünü kapatır (S1 sadece pratik niyetini gösteriyordu);
  host-query + renderer, **core DEĞİŞMEZ (engine purity)**; aynı pencere mastery ile tutarlı; matchup plumbing
  (Lane #2) ya da timeline (Post-game #2) gerektirmeyen en yüksek (değer ÷ risk) Slice-2 adayı.
- **Sonuç:** `LearningProgressEntry`'ye `games_played`+`wins`; ikinci `matches` GROUP BY sorgusu + JS merge;
  i18n `poolCoach.learningGames`/`learningWinRate` (tr/en parite, `{{n}}`/`{{wr}}`); `.pool-progress__sub` nötr
  stil. host 164 + renderer 273 + typecheck 0 + i18n parite yeşil. core/WASM el değmedi.

## ADR-006 — Post-game koçluk Epic #3: hedef-tutturma serisi görseli ilk dilim seçildi (2026-06-17)
- **Bağlam:** Epic #3 için Explore ajanı 7 boşluk önerdi; koddan doğrulandı: form-per-metric ZATEN TrendPanel'de
  (redundant); CS@10/farm lesson'ı postgame.rs'te BİLİNÇLİ kapalı (jungle/support için yanlış-alarm → dürüstlük
  tasarımı, eklersem honest-design bozulur); off-role per-rol stat + combo-outcome + macro-timeline core/timeline
  değişikliği ister (geniş → ertelendi).
- **Karar:** İlk dilim = hedef-tutturma serisi görseli (GameReviewCard'da ✓/✗ dot dizisi). focus_goals met/missed
  geçmişi VAR ama yalnız streak SAYISI gösteriliyordu (GameReviewCard streak span). Yeni host `get_focus_history`.
- **Gerekçe:** Veri hazır, küçük+güvenli, yüksek motivasyon/geri-bildirim değeri, **core değişmez (engine purity)**;
  honest-design'ı (CS lesson'ı kapalı tutma) bozmaz.
- **Sonuç:** `get_focus_history` + GameReviewCard dot satırı; renderer 270 + host 162 yeşil. Sonraki adaylar
  (off-role hedef, combo-outcome, macro-timeline) daha büyük/core → ayrı tur.

## ADR-005 — Lane-matchup faz-avantajı dürüstçe "KB tahmini" etiketlenir (2026-06-17)
- **Bağlam:** `lane_matchup_from_json` phase_advantage'ı YALNIZ arketip `power_curve`'den (`adv()`) hesaplıyor —
  ölçülen matchup verisine hiç bakmıyor — ama panel barları kaynak etiketsiz gösteriyordu → kullanıcı bunları
  ölçülen win-rate sanabilir. `inferred` yalnız rakip KİMLİĞİNİN tahmin olduğunu söyler, avantaj sayılarının değil.
- **Karar:** `LaneMatchup` struct'a `source: String` ekle (şimdilik sabit "kb_estimate"); panelde "KB tahmini"
  rozeti + tooltip göster. Ölçülen veriyi plumb etmek (source="measured") sonraki dilime ertelendi (geniş core değişikliği).
- **Gerekçe:** Dürüstlük DNA'sı (B-03/B-10/B-23 hattı); minimal + güvenli; **scoring/engine DEĞİŞMEZ (engine purity)**.
  Geniş `ctx.matchups` plumbing'i olmadan kullanıcı barların arketip-tahmini olduğunu anlar.
- **Sonuç:** core 570 + renderer 268 + host 161 yeşil; recommendation.ts `source?` (Rust hep emit, TS opsiyonel —
  `inferred?` deseni). WASM rebuild gerekti (host runtime'da alanı emit etsin; core/pkg gitignore'da → commit'lenmez).

## ADR-004 — Match-History Browser Epic: MVP kapsam varsayımları (2026-06-17)
- **Bağlam:** Kullanıcı "büyük geliştirme modu" + öncelik #1 match-history browser; kapsam belirsizse "soru sormadan makul varsay" dedi. Epic MVP'ye bölündü, ilk dikey dilim (liste sekmesi) uygulandı.
- **Karar (varsayımlar):**
  - **A1** Geçmiş aktif summoner puuid'sine kapsamlı (`useActiveSummonerPuuid`); varsayılan limit 20.
  - **A2** Listedeki "review verdict" = dürüst **"İncelendi"** rozeti (karne var mı). Zengin verdict (metric lines/koçluk) Slice-2 detay panelinde — sahte tek-değer aggregate verdict UYDURULMAZ (dürüstlük DNA'sı).
  - **A3** CS/dk = `cs/(süre/60)`; cs/cs_at_10/vision null → "—".
  - **A4** 4. LobbyView sekmesi (yeni App-level status/route YOK).
  - **A5** Tip elle (`src/types/match-history.ts` + eşleşen host şekli); saf host SQL → **core/ts-rs/WASM değişmez**.
  - **A6** Tarih relatif (`time.*` + LobbyView relativeTime deseni; played_at Unix SANİYE → ×1000).
  - **A7** Queue etiketi `review.queue.*` + host `queueGroup` (soloq/flex/aram/normal).
- **Gerekçe:** En az sürtünme + mevcut desen yeniden-kullanımı (`recentMatches` JOIN, RankCard tip deseni, P-07 dürüst loading/error/empty); core'a dokunmadan additive. Yeni Riot çağrısı/cloud yok.
- **Sonuç:** Slice 1 (liste) teslim — renderer 262 + host 160 + typecheck 0. Slice 2 (detay paneli: `GameReviewCard` `matchId`/`review` prop refactor) + Slice 3 (champion/rol/win-loss filtreleri) sıradaki turlar.

## ADR-003 — Ban ikonu küçük bileşene çıkarılır (2026-06-16)
- **Bağlam:** `ChampSelectScreen.tsx`'te ban `<img>` iki yerde inline; URL 404'te `onError` yedeği yok (icon-bug sınıfı). React'te `.map()` içinde inline `useState` tutulamaz.
- **Karar:** `BanIcon.tsx` adlı küçük presentational bileşen oluştur; her iki ban bloğu onu kullansın.
- **Gerekçe:** onError state'i için bileşen ZORUNLU; ayrıca iki özdeş ternary'yi DRY yapar ve izole test edilebilir kılar. Mevcut `ItemIcon`/`ChampionIcon` desenine uyumlu.
- **Sonuç:** İki ban bloğu tek satıra iner; `BanIcon.test.tsx` ile davranış kilitlenir.

## ADR-002 — Otomatik commit/push YOK (2026-06-16)
- **Bağlam:** Otonom döngü süreklilik istiyor; ama kullanıcının değişmez kuralı "commit/push yalnız açıkça isteyince".
- **Karar:** Döngü hiçbir iterasyonda otomatik commit/push yapmaz; değişiklikleri çalışma ağacında bırakır.
- **Gerekçe:** Kullanıcı diff'leri biriktikçe gözden geçirir; geri-alınabilirlik + güven korunur.
- **Sonuç:** Kalite kapıları yeşil olsa bile commit edilmez; kullanıcı commit'ler.

## ADR-001 — Sürekli otonom geliştirme sistemi kuruldu (2026-06-16)
- **Bağlam:** Kullanıcı tek-seferlik görev yerine kendi backlog'unu üreten, önceliklendiren, uygulayan, test eden ve dokümante eden sürekli bir mühendislik döngüsü istedi.
- **Karar:** Repo kökünde 7 yönetim dosyası (`AGENTS/PROJECT_STATE/BACKLOG/TASKS/DECISIONS/CHANGELOG/QUALITY_CHECKS`) + Inspect→…→Continue döngüsü; kullanıcı rolleri gerçek `Agent` tiplerine eşlendi.
- **Gerekçe:** İzlenebilir + güvenli + tekrarsız iterasyon; mevcut araç envanteriyle çalışır (simülasyon değil).
- **Sonuç:** İlk iş B-01 (image fallback); döngü "dur" denene dek devam eder, riskli/büyük iş onay-kapılı.
