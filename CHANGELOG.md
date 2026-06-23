# Changelog

Bu projedeki kayda değer tüm değişiklikler bu dosyada belgelenir.
Format [Keep a Changelog](https://keepachangelog.com/), versiyonlama
[Semantic Versioning](https://semver.org/) temellidir.

## [Unreleased]

### Eklendi
- **Ayarlar → "Hakkında" bölümü (kalıcı Riot disclaimer)** — Ayarlar paneline kalıcı bir "Hakkında" bölümü eklendi:
  uygulama adı + Riot Games'in zorunlu kıldığı üçüncü-taraf disclaimer'ı ("Riot Games tarafından onaylanmamıştır…") +
  veri-kaynağı atfı (Data Dragon / Community Dragon). Disclaimer önceden YALNIZ onboarding sihirbazında bir kez görünüp
  kayboluyordu; artık uygulama içinde her zaman erişilebilir (yasal gereklilik + Riot uygulama-incelemesiyle hizalı).
  Disclaimer metni tek kaynaktan (`onboarding.disclaimer`) okunur → legal metin iki kopya arasında sürüklenmez. Saf
  renderer (core/host/worker değişmedi — engine purity); +2 i18n anahtarı (tr/en parite) + disclaimer'ı kilitleyen test.
- **"Hakkında"da uygulama sürümü** — "Hakkında" bölümü artık dağıtılan build'in sürümünü gösteriyor
  ("Champ Select Assistant · v{x}"). Pending-Review bir uygulamada hangi build'in çalıştığını/incelendiğini ve
  güncel olup olmadığını görmek için yararlı. Yeni host komutu `get_app_version` (`app.getVersion()` sarmalı) +
  renderer mount-fetch; sürüm çözülemezse satır yalnız uygulama adını gösterir (graceful). Host+renderer (core/worker
  değişmedi — engine purity); +2 test (ipc-contract handler + renderer sürüm-render).

### Geliştirildi
- **Kompakt HUD daha sıkı/glanceable** (Overlay HUD Epic — Slice 3) — Kompakt modda artık plan başlığındaki metin
  etiketleri (rol "Üst" + "vs Rakip") gizleniyor — şampiyon/rakip ikonları bağlamı zaten taşıyor — ve layout boşlukları
  sıkılaşıyor (`.ingame-view--compact` daha küçük gap/padding). 400px'te yüzen pencere daha yoğun, daha hızlı okunan bir
  bakış-at HUD oluyor. Saf renderer/CSS (core/host değişmedi); kompakt-toggle testi rol etiketinin gizlenmesini de doğrular.
- **Kompakt HUD tercihi artık kalıcı** (Overlay HUD Epic — Slice 2) — Ayarlar'a "Oyun-içi kompakt HUD (yalnız
  görseller)" seçeneği eklendi. Açıkken in-game overlay her maça doğrudan kompakt modda (yalnız glanceable görseller)
  başlıyor — artık her maç elle toggle'lamaya gerek yok. Kullanıcı maç-içinde yine "Detaylı"ya geçebiliyor. Ayar JSON
  blob'una eklendi (kolon-migration yok; eski ayarlar resetlenmeden default'lanır). Host+renderer (core değişmedi —
  engine purity); tr/en parite + host (getSettings round-trip/optional-default) ve renderer (IngameView başlangıç-kompakt) testleri.
- **Oyun-içi "Kompakt HUD" modu** (Overlay HUD Epic — Slice 1) — Oyun-içi overlay'e bir "Kompakt/Detaylı" toggle'ı
  eklendi. Maça girince pencere zaten sağ-üstte küçük (400px) ve always-on-top yüzüyordu, ama yoğun plan metniyle
  (win-condition/rol/spike/lane/wave/matchup/mid-late) sıkışıktı. "Kompakt"a geçince bu metin gizleniyor; yalnız
  **glanceable görseller** kalıyor: şampiyon/KDA başlığı, **güç eğrisi çubuğu**, **objective doğuş saatleri** ve
  faz — sağ-üstteki yüzen pencere gerçek bir bakış-at HUD'ı oluyor. "Detaylı"ya dönünce tam plan geri geliyor.
  Saf renderer (mevcut overlay-mode windowing kullanılır; core/host değişmedi); ToS-güvenli (yeni pencere/şeffaflık
  yok — bkz ADR-011: gerçek şeffaf overlay penceresi ToS riski nedeniyle elendi); tr/en parite + test.

### Geliştirildi
- **"Yeniden üret" artık gerçekten farklı bir not üretiyor** (ML/LLM Koçluk Epic — Slice 5) — Koç notu "Yeniden üret"
  butonu artık LLM'e açıkça **"öncekinden farklı bir açıdan, farklı kelimelerle yaz"** talimatı geçiriyor (yalnız
  temperature rastgeleliğine güvenmek yerine). Böylece kullanıcının örtük "beğenmedim" sinyali TÜKETİLİYOR: yeniden-üret
  anlamlı şekilde farklı bir not getiriyor. `vary` bayrağı uçtan uca geçirildi (DeepDiveTab → `get_coach_narrative` →
  `getCoachNarrative` → `fetchLlmCandidate` → `buildCoachUserPrompt`). Host+renderer (core değişmedi — engine purity);
  +2 host testi (prompt hint + request-body pass-through) + DeepDiveTab vary:true assertion.

### Eklendi
- **Koç notu "Yeniden üret"** (ML/LLM Koçluk Epic — Slice 4) — LLM kaynaklı koç notunun başlığına bir "Yeniden üret"
  butonu eklendi: kullanıcı üretilen notu beğenmezse (ya da audit'i geçemeyip deterministik'e düşmüşse) tek tıkla
  taze bir LLM notu çekebiliyor (LLM doğası gereği her seferinde biraz farklı). Memnuniyetsizliği TÜKETEN gerçek bir
  etkileşim — sadece veri toplama değil. Buton yalnız LLM süreçte olduğunda görünür (düz deterministik notta gizli,
  çünkü yeniden-üretmek aynı sonucu verir). Renderer-only (mevcut `get_coach_narrative` re-invoke; core/host değişmedi
  — engine purity); şampiyon değişince sıfırlanır; tr/en parite + 2 test.

### Geliştirildi
- **LLM koçluk notu daha zengin + güvenli temellendirildi** (ML/LLM Koçluk Epic — Slice 3) — Opsiyonel LLM koç
  notu prompt'una üç grounded fact daha eklendi: **rakip kompo özeti** (ör. "AP ağırlıklı · frontline yok"),
  **faz avantajı** (erken/orta/geç ~%X) ve **veri boşluğu uyarısı** (gerçek verisi olmayan sinyaller — meta/matchup/
  build — LLM'e "bunlar hakkında iddia ETME" olarak verilir → anti-halüsinasyon/grounding). Daha bağlamlı + daha az
  uydurma riskli LLM notu (core audit zaten abartıyı keser; bu, üretilen metni baştan daha sağlam temellendirir).
  Host-only (`llm-narrator.ts` buildCoachUserPrompt; core/renderer değişmedi — engine purity); +1 birim testi.

### Eklendi
- **LLM koçluk için "Bağlantı test et"** (ML/LLM Koçluk Epic — Slice 2) — Ayarlar'daki LLM koçluk bölümüne, girilen
  endpoint'in (ve model'in) çalışıp çalışmadığını champ-select'i beklemeden doğrulayan bir buton eklendi: tıklayınca
  endpoint'e **minimal bir "ping" isteği** atılır (kullanıcı/oyun verisi GÖNDERİLMEZ, yalnız "ping" + max_tokens 1)
  ve dürüst sonuç gösterilir ("✓ Bağlandı" / "Bağlanılamadı — endpoint ve model'i kontrol et"). Kurulum sürtünmesini
  azaltır (önceden endpoint'in doğru olup olmadığı ancak champ-select'te belli oluyordu). Yeni host `test_coach_llm`
  (mevcut `FetchFn` seam'iyle test edilebilir; OpenAI-uyumlu yanıt biçimini doğrular). Engine purity (core değişmedi);
  host+renderer; tr/en parite + 5 host testi (ok/http/bad-response/network/boş-endpoint).
- **Koç notunda kaynak şeffaflığı** (ML/LLM Koçluk Epic — Slice 1) — Öneri detayındaki "Koç notu" artık notun
  **kaynağını** dürüstçe gösteriyor: LLM ürettiyse **"LLM"** rozeti; LLM önerisi audit'i geçemeyip deterministik
  nota düşüldüyse **"LLM reddedildi"** rozeti (tooltip açıklamalı); düz deterministik (varsayılan) notta rozet yok.
  Kullanıcı bir koçluk notunun yapay-zekâ mı yoksa deterministik motor mu olduğunu artık görebiliyor (veri-dürüstlüğü
  DNA'sı + LLM fazının güven temeli). `CoachNarrative.source`/`external_rejected` zaten core audit'inde hesaplanıyordu
  ama render edilmiyordu. Saf renderer (core/host değişmedi; motor purity korunur — LLM yalnız anlatım seam'i);
  tr/en parite + 3 test. NOT: LLM koçluk pipeline'ı zaten tam bağlıydı (Settings'e OpenAI-uyumlu endpoint gir → audit'li
  LLM notu + deterministik fallback) → bkz ADR-010.
- **Şampiyon detayında hareketlilik + takım katkısı** (champ-select) — Şampiyon detay kartı artık KB'den
  gelen **mobility** (Hareketlilik: High/Medium/Low/None) rozetini ve **utility_tags** (Takım katkısı:
  Engage/Peel/Ön saf/Alan kontrolü/… ) bölümünü gösteriyor — "bu şampiyon takımın engage/peel/disengage
  ihtiyacını karşılar mı, ne kadar hareketli?" sorusunu draft'ta yanıtlar. Veri zaten core'da hesaplanıp
  payload'da (`ChampionDetail.mobility` + `utility_tags`, arketipten) AMA hiçbir yerde render edilmiyordu.
  Saf renderer (core/host/CSS değişmedi; `.cdc-badge` reuse, etiketler WIN_LABELS desenli modül-map); +2 i18n
  bölüm-etiketi (tr/en parite) + test. Sıfır fabrikasyon (KB-türevli).
- **Draft derin-incelemede "Pick profili"** (champ-select) — Öneri detay sekmesi (DeepDiveTab) artık seçilen
  şampiyonun **blind-pick güvenliğini** ("Blind pick güvenli / Orta / Güvenli değil") ve **execution zorluğunu**
  ("Kolay / Orta / Zor (n/5)") bantlı etiketlerle gösteriyor — pick-anında "bunu körlemesine seçmek güvenli mi,
  mekanik olarak ne kadar zor?" sorusunu yanıtlar. Veri zaten core'da hesaplanıyordu (`DraftPlan.blind_pick_safety`
  + `execution_difficulty`, KB arketipinden — mekanik, comfort değil) ama hiçbir champ-select yüzeyinde render
  edilmiyordu; i18n bantları da hazırdı (kullanılmıyordu). Eşik 0.6 = core BLIND_SAFE_THRESHOLD ile hizalı.
  Saf renderer (core/host değişmedi); tr/en parite + 2 test. Sıfır fabrikasyon (KB-türevli deterministik).
- **Maç Geçmişinde "daha fazla yükle"** (Match-History Epic — Slice 5) — Maç Geçmişi artık ilk 20 maçla sınırlı
  değil: liste sonundaki "Daha fazla yükle" butonu sonraki 20'şerlik sayfayı getiriyor (host `get_match_history`
  limit parametresi zaten vardı). Tam sayfa döndüğünde buton görünür, daha az dönünce (hepsi yüklendi) gizlenir.
  Saf renderer + mevcut host komutu (yeni Riot çağrısı/cloud yok); özet ve filtreler büyüyen liste üzerinde
  yeniden hesaplanır. tr/en parite (`matchHistory.loadMore`) + renderer testi.
- **Maç Geçmişinde özet başlığı** (Match-History Epic — Slice 4) — Maç Geçmişi listesi artık gösterilen
  (filtrelenmiş) maçların **galibiyet/mağlubiyet rekorunu + kazanma oranını + toplam KDA'sını** bir bakışta
  veriyor (ör. "12G 8M · %60 · 2.85 KDA"). Filtrelerle birleşince doğrudan değerli: bir şampiyon/rol seçince
  "o şampiyonda/rolde rekorum" anında görünür. Saf renderer — zaten çekilmiş maçlardan hesaplanır (yeni Riot
  çağrısı/host sorgusu yok). tr/en parite (`matchHistory.summary`) + renderer testi.
- **Lane eşleşmesinde ölçülen kazanma oranı satırı** (Lane-Matchup Epic — Slice 2) — Lane eşleşme paneli artık
  (veri varsa) rakibe karşı **ölçülen genel kazanma oranını** ayrı dürüst bir satırda gösteriyor (ör. "Ölçülen:
  %48 · 2.200 maç"), `champion_matchups` verisinden. Bu, tahmini faz barlarından (KB tahmini) **ayrı** tutulur:
  tek genel oran 3 faza BÖLÜNMEZ (faz-bazlı ölçüm yok → bölmek fabrikasyon olurdu). Yalnız yeterli örneklemde
  (≥20 maç) gösterilir; altında gürültü olduğu için gizli. Core'a `LaneMatchup.measured_win_rate`/`measured_games`
  + `TeamContextInput.matchups` eklendi; host `getLaneMatchup` matchup verisini geçirir. **Scoring DEĞİŞMEDİ**
  (engine purity — yalnız json_api presentation read-side); tr/en parite (`laneMatchup.measured`) + core/renderer testleri.
- **Off-rol zayıflık kartı** (Post-game Koçluk Epic — Slice 2) — İstatistik sekmesi artık oyuncunun **ana rolünden İstatistik sekmesi artık oyuncunun **ana rolünden
  daha düşük kazanma oranlı off-rollerini** dürüstçe gösteriyor (ör. ana rol Orta %70 iken "Üst: %20 · 5 maç").
  En çok oynanan rol "ana rol" sayılır; ondan düşük WR'li ve ≥3 maçlık off-roller en zayıf önce listelenir —
  "hangi rol seni aşağı çekiyor?" sorusunu tek bakışta yanıtlar. Tamamen ölçülen veri (`matches.position`+`win`),
  uydurma yok; anlamlı zayıflık yoksa kart gizli; ARAM/Arena (rolsüz) doğal olarak hariç. Yeni host
  `get_off_role_performance` (matches GROUP BY position). Core değişmedi (engine purity); tr/en parite
  (`offRole.*`) + host/renderer testleri. (Not: Lane-Matchup S2 ölçülen-veri faz-fabrikasyon riski nedeniyle elendi → ADR-008.)
- **Öğrenme hedefinde gerçek maç sonucu (maç sayısı + WR)** (Havuz Gelişim Epic — Slice 2) — "Öğrenme
  hedeflerin" kartı artık mastery-puanı kazancının yanında o hedefte son 30 günde oynanan **gerçek maç sayısı
  ve kazanma oranını** da gösteriyor (ör. "4 maç · %75") — mastery sadece "grind"i ölçer; WR pratiğin işe
  yarayıp yaramadığını söyler, böylece recommend→işaretle→pratik→**sonuç** döngüsü kapanır. İnce-örneklem
  dürüstlüğü: 1–2 maçta yalnız sayı (gürültülü WR uydurulmaz), 0 maçta alt-satır gizli. Host
  `get_learning_progress` aynı pencerede `matches`'ten games/wins ekler. Core değişmedi (engine purity);
  tr/en parite (`poolCoach.learningGames`/`learningWinRate`) + host/renderer testleri.
- **Havuz koçunda öğrenme-hedefi ilerleme kartı** (Havuz Gelişim Epic — Slice 1) — Havuz koçu artık
  kullanıcının "Öğreniyorum" işaretlediği (ChampionDetailCard) şampiyonların son 30 günlük mastery
  ilerlemesini ayrı "Öğrenme hedeflerin" bölümünde gösteriyor ("+N puan · Sv X" ya da işaretli-ama-hareket-yok).
  recommend→işaretle→ilerleme döngüsünü kapatır. Yeni host `get_learning_progress` (user_preferences.learning ⋈
  mastery_snapshots; mevcut mastery-progress'ten ayrı, learning-filtreli). Core değişmedi (engine purity);
  tr/en parite + host/renderer testleri (player komutlarının ilk test dosyası).
- **In-game objective mutlak doğuş saati** (Overlay Makro Epic — Slice 1) — Oyun-içi overlay'de obje
  zamanlayıcıları (ejder/baron vb.) artık geri sayımın yanında mutlak doğuş oyun-saatini de gösteriyor
  (ör. "1:30" altında "@24:00") — oyuncu kendi oyun-saatiyle toplama yapmadan "ne zaman doğacak"ı bir bakışta
  okur. Veri (`next_spawn_secs`) zaten payload'daydı ama render edilmiyordu; yalnız doğmamış objelerde
  (state≠up). Renderer+i18n (core/host DEĞİŞMEDİ); `overlay.spawnAtHint` tr/en parite + `gameClock` birim testi.
- **Maç sonu karnesinde hedef-tutturma serisi görseli** (Post-game Koçluk Epic — Slice 1) — Maç Sonu Karnesi
  artık son odak hedeflerinin (focus_goal) tutturma paternini küçük ✓/✗ noktalarıyla gösteriyor (yeşil=tuttu,
  kırmızı=kaçtı, tooltip'te hedef metni). Önceden yalnız streak SAYISI vardı; artık serinin ŞEKLİ (ör. ✓✓✗✓)
  görünür — oyuncu hedef-döngüsünün gerçek etkisini bir bakışta okur. Yeni host `get_focus_history`
  (focus_goals'tan met/missed, en yeni önce; superseded/no_data hariç). Core değişmedi (engine purity);
  tr/en parite + host/renderer testleri (GameReviewCard'ın ilk testi).
- **Lane eşleşmesinde "KB tahmini" dürüstlük rozeti** (Lane-Matchup Epic — Slice 1) — Lane eşleşme panelindeki
  faz-avantaj barları (erken/orta/geç) artık dürüstçe "KB tahmini" rozetiyle etiketli: bu barlar ölçülen kazanma
  oranı DEĞİL, arketip güç-eğrisinden türetilen tahmin (`lane_matchup_from_json` ölçülen matchup verisine hiç
  bakmıyor — her zaman heuristic). Core `LaneMatchup`'a `source` alanı ("kb_estimate") eklendi; tooltip mekaniği
  açıklıyor. Scoring/engine DEĞİŞMEDİ (engine purity korundu — yalnız read etiketi). tr/en parite + core/renderer testleri.
- **Maç Geçmişi sekmesi** (Match-History Browser Epic — Slice 1) — Lobby'ye 4. sekme
  ("Maç Geçmişi"): yerel DB'deki son 20 maç şampiyon ikonu, rol, galibiyet/mağlubiyet,
  relatif tarih, KDA, CS/dk ve vision ile listeleniyor; karnesi olan maçlar "İncelendi"
  rozetiyle işaretli. Yeni host komutu `get_match_history` (mevcut `recentMatches` JOIN'i
  + `game_reviews` EXISTS işareti — **yeni Riot çağrısı / cloud YOK, sadece local DB**).
  Dürüst durumlar (P-07 deseni): yükleniyor / veri alınamadı / boş ayrı. CS/dk hesaplanır,
  eski (cs null) maçlarda "—". Saf host+renderer (core/ts-rs/WASM değişmedi); tr/en parite,
  +4 renderer testi + 1 host testi (sıralama/JOIN/has_review).
- **Maç Geçmişi — karne detay paneli** (Match-History Epic — Slice 2) — "İncelendi" satırına tıklayınca
  (klavye erişilebilir, `role="button"`) o maçın tam karnesi mevcut `GameReviewCard` ile detay panelinde
  açılıyor; "← Maçlara dön" ile listeye dönülüyor. `GameReviewCard` opsiyonel `matchId` prop'uyla belirli
  maçı çeker (yeni host `get_game_review` by match_id; karnesiz maçlar tıklanamaz; StatsView'daki prop'suz
  "en yeni" davranışı korunur). +1 host (review-by-id) + 2 renderer test.
- **Maç Geçmişi — filtreler** (Match-History Epic — Slice 3) — liste üstüne rol, şampiyon ve sonuç
  (galibiyet/mağlubiyet) için üç açılır filtre (yalnız listede VAR olan rol/şampiyonlar seçenek olur);
  filtreleme tamamen client-side (yeni fetch yok). Hiçbir maç eşleşmezse dürüst "Bu filtreye uygun maç
  yok" mesajı. tr/en parite + 2 renderer test. **Match-History MVP (liste + detay + filtre) tamamlandı.**
- **Havuz koçunda dürüst veri-hatası durumu** — `PoolBuilder` öneri fetch'i
  (`get_pool_suggestions`) reddedildiğinde artık sessizce "bu rol için öneri yok"
  demiyor; backend/DB hatasını kardeş kartlarla (RankCard/TrendPanel/WeeklySummaryCard)
  tutarlı biçimde dürüstçe `app.dataError` ("Veri alınamadı") olarak gösteriyor.
  Yükleniyor / hata / boş üç durumu net ayrıldı (sessiz hata yutma giderildi —
  bir kullanıcı backend çökmesini "havuzun zayıf" sanmaz). Renderer-only, sıfır yeni
  i18n (mevcut `app.dataError`), +1 test. (P-07)
- **Ayarlarda temalı "değişiklikleri at" onayı** — kaydedilmemiş değişikliklerle ayar
  panelini kapatma denemesi (X düğmesi / Escape / arka-plana tık) artık native
  `window.confirm` yerine uygulamanın koyu temasıyla uyumlu, odaklanabilir bir
  `role="alertdialog"` gösteriyor ("Düzenlemeye dön" / "Değişiklikleri at"). Native
  tarayıcı dialog'unun görsel kopukluğu giderildi (daha profesyonel). Escape önce
  onayı kapatır; footer "İptal" hâlâ doğrudan atar. Renderer-only, tr/en parite + test. (P-06)
- **Combo panosunda gerçek track-record** — `ComboBoard` artık her müttefik combo'su için
  oyuncunun o eşle **gerçek co-pick geçmişini** (≥2 maç → "Geçmişin: N maç · %WR") teorik
  güç çubuğunun yanında gösteriyor. Veri `get_combo_outcomes`'tan (zaten HeroCard'ın yalnız
  BİRİNCİL combo'su için çekiliyordu); my-key locked analizden, eşleşmezse satır gizli
  (yeni oyuncuda boş — sahte istatistik yok). Renderer-only, tr/en parite + 3 yeni test. (P-03)
- **Draft simülatöründe daha derin koçluk** — `DraftSimulatorPanel` artık core'un
  çoktan hesapladığı ama gösterilmeyen iki alanı yüzeye çıkarıyor: (1) **`why_this_move`**
  — her aday pick için "Neden bu?" stratejik gerekçesi (önceden yalnız rank-0'ın
  "Neden alternatif?"i vardı), (2) **faktör sayısal delta'ları** — improved/worsened
  faktör chip'leri artık büyüklüğü de gösteriyor (ör. "Engage +0.17"), `result.deltas`'tan;
  delta yoksa salt-isim (geriye uyumlu). Renderer-only, sıfır core değişikliği; tr/en
  parite + 3 yeni birim testi. (P-01)
- **In-game güç eğrisi görsel çubuğu** — overlay oyun-plan kartına 3 segmentli
  (erken/orta/geç) bir HUD çubuğu eklendi; her segment, oynanan şampiyonun arketip
  `power_curve` değeriyle (0..1) orantılı yükseklikte ve zirve faz teal ile
  vurgulanır. Alt-tab'da "şu an güçlü müyüm?" sorusunu bir bakışta yanıtlar ve
  metinsel `spike_note`'u görsel olarak tamamlar. Core `IngamePlan`'a `power_early/
  mid/late` alanları yüzeye çıkarıldı (arketipten birebir, e2e testle kilitli);
  PowerCurveBar a11y için tek `role=img` + yüzdeli `aria-label` (çubuklar
  dekoratif), tr/en parite, izole birim testleri. Ayrıca **canlı oyun fazı
  işaretçisi**: `macro.phase` (erken/orta/geç) o kolonu "şu an buradasın" (▾) +
  teal-vurgu ile işaretler → statik referans canlı "neredeyim"e döner; aria-label
  fazı da içerir. (WS3 — overlay polish; W-01 + W-02)
- **Overlay plan kartı: hesaplanan-ama-gizli alanlar** — core'un ürettiği ama UI'ın
  düşürdüğü iki alan artık gösteriliyor: `damage_profile` (hasar tipi) takım-rolünden
  sonra bir satır, `level` (oyuncu seviyesi) KDA önekinde (Sv/Lv). (WS3 — W-03)
- **Sürekli otonom geliştirme sistemi** — repo kökünde yönetim dosyaları
  (`AGENTS`, `PROJECT_STATE`, `BACKLOG`, `TASKS`, `DECISIONS`, `QUALITY_CHECKS`)
  ve Inspect→Discover→Prioritize→Delegate→Implement→Verify→Document→Continue
  döngüsü kuruldu. Her iterasyon küçük, test-geçen, geri-alınabilir bir
  iyileştirme üretir; otomatik commit yok.

### Değişti
- **Worker okuma-yolu patch çözümlemesi DRY** — `readRates`/`readMatchups`/`readBuilds`
  aynı "en taze patch'i recency ile seç" bloğunu (B-14 recency yorumu dâhil) birebir
  3× tekrarlıyordu; ortak `resolveLatestPatch` helper'ına indirildi. Tek bakım noktası,
  davranış (açık boş-string patch dâhil) korundu. (B-36)
- **`syncDataPipelineInner` DRY** — manuel veri-pipeline'ındaki beş kaynak (ddragon,
  meraki, build/matchup seed, match-v5) birebir aynı try/catch/fetch-log/error-push
  bloğunu tekrarlıyordu (~140 satır). Ortak `runSource<T>` helper'ına indirildi;
  kaynağa özgü tek fark sync fonksiyonu + başarı mesajı callback'i. Davranış,
  fetch-log'lar ve hata-dizisi korundu (mevcut uçtan-uca pipeline testi doğrular). (B-32)
- **`useChampSelect` türev-state'leri DRY** — yedi koçluk çıktısı (game plan,
  counter-pick, team comp, combo board, draft verdict, counter-item, lane matchup)
  birebir aynı "session imzasından türet" effect'ini tekrarlıyordu (~140 satır).
  Ortak `useSessionDerived` helper'ına indirildi (iptal-edilebilir, en-güncel-kazanır,
  no-session'da temizler). Davranış korundu; önce türev-state'lere güvenlik-ağı
  testleri eklendi. (B-33)

### Eklendi
- **Cold-start seed priming** — arka plan scheduler'ı, DDragon şampiyon sync'inden
  hemen sonra (FK-valid) bundled offline build/matchup seed'lerini bir kez içe aktarır
  (`primeColdStartSeeds`; yalnız ilgili tablo boşken, atomik transaction ile). Böylece
  otomatik yol, kullanıcının manuel "Settings → senkronize et" butonuna basmasını
  beklemeden ilk açılışta offline kapsamaya kavuşur. Best-effort: seed hatası tick'i
  veya DDragon başarısını düşürmez. (B-02)

### Düzeltildi
- **Bayat edge matchup/build verisi de "düşük güven" işaretlenir** — `syncEdgeRates`'in
  tazelik kontrolü (worker `updated_at` >48s eskiyse `confidence='low'`) yalnız **rates**'e
  uygulanıyordu; aynı bayat ingestion'dan gelen **matchups** ve **builds** örnek-bazlı
  confidence'larını koruyordu (potansiyel `medium`/`high`). Durmuş ingestion / dev-key
  expiry'de bu, bayat counter-pick/build verisini "taze yüksek-güven" gösteriyordu — tam da
  staleness kontrolünün engellemeye çalıştığı sahte-tazelik. Rates yanıtının kanonik
  `updated_at`'i (worker `ingest_meta`) üç tabloyu da kapsar; `stale` downgrade'i artık
  matchups+builds'e de uygulanıyor. TDD (RED→GREEN), yeni regresyon testi. (L-01)
- **Bozuk `wins > games` matchup satırları ingestion'da elenir** — dış kaynaklardan
  (u.gg `parseUggMatchups`, edge worker `syncEdgeRates`) gelen matchup satırları yalnız
  `games > 0` / geçerli-id için filtreleniyordu; `wins > games` (win_rate >1.0) bozuk
  satırlar `champion_matchups`'a sızabiliyordu ve matchup skorunu şişirebiliyordu. İki
  yola da defensive guard eklendi (u.gg `|| wins > games` continue; edge filter
  `Number(m.wins) <= Number(m.games)`). Bu, B-38'in (motor risk-notu `saturating_sub`)
  **upstream tamamlayıcısı** — kötü veri DB'ye hiç girmez. Geçerli veri (wins ≤ games)
  etkilenmez. 2 regresyon testi (u.gg birim + edge fixture). (B-41)
- **`docs/api-key-policy.md` Tauri→Electron güncellendi** — doküman geliştiriciyi var
  olmayan bir kuruluma yönlendiriyordu: `src-tauri/.env`, `dotenvy::dotenv()`,
  `tauri.conf.json` checklist'i ve `target/release/*.exe` binary-tarama — hepsi Tauri/Rust
  dönemine ait. Gerçek mekanizma: anahtar Node host'ta `process.env.RIOT_API_KEY`
  (+ en yakın `.env`, process.env öncelikli) `desktop/src/main/riot/client.ts`
  `runtimeEnv()` ile okunur; Rust/WASM core hiç görmez. LCU hover compliance notundaki
  stale `commands/champ_select.rs` referansı da gerçek konuma (`desktop/src/main/commands/lcu.ts`
  `hoverChampion`) çekildi. (B-40)
- **Arena (queue 1700) artık laneless sayılıyor** — `json_api.rs` `my_pos()` yalnız
  ARAM'ı (450) sentetik "aram" lane'ine eşliyordu; Arena (1700) `else` dalında atanan
  LCU pozisyonunu döndürüyordu. LCU Arena'da pozisyonu boş bırakır ama renderer kalıcı
  tercih-rolünü (örn. "middle") queue'dan bağımsız enjekte ettiğinden bu rol Arena
  session'a sızıp önerilere anlamsız "lane_performance eksik" rozeti bastırabiliyordu
  (Arena'da lane yok). `my_pos()` artık `matches!(queue_id, 450 | 1700)` ile her iki
  brawl modunu da laneless sayar (engine.rs `is_aram` ile hizalı). Regresyon testi
  (queue 1700 fixture → hiçbir öneride lane_performance sinyali yok). (B-39)
- **Stretch-pick risk notunda u32 underflow koruması** — `engine.rs`'in düşük-deneyim
  stretch önerisi için ürettiği risk notu `losses = games - wins` ile korumasız
  çıkarma yapıyordu. `wins`/`games` host SQLite'tan (`COUNT(*) AS games, SUM(win) AS wins`)
  `wins <= games` invariant'ı zorlanmadan gelir; bozuk tek bir satır `wins > games`
  yapabilir. `[profile.release]`'de `overflow-checks` kapalı olduğundan release/WASM
  build'inde bu sessizce underflow'la sarıp kullanıcıya "…4294967290L…" gibi çöp not
  gösterir (debug'da panik). Not-üretimi saf `stretch_risk_note` yardımcısına çıkarıldı
  ve mağlubiyet `saturating_sub` ile (crate konvansiyonu) hesaplanıyor. 3 birim testi
  (sıfır maç / normal / bozuk wins>games). Geçerli veride çıktı birebir aynı. (B-38)
- **Cron ingestion hatası artık görünür** — worker'ın `scheduled` (cron) yolu, production'daki
  birincil ingestion sürücüsü olmasına rağmen `runIngestion` reddini bağlamsız bırakıyordu
  (manuel `/v1/ingest` yolu logluyordu). Bağlamlı `console.error("scheduled ingest failed", e)`
  eklendi → durmuş cron (dev-key expiry vb.) `wrangler tail`'de görünür. Regresyon-kilidi testi
  reddin yutulduğunu doğrular. (B-37)
- **"Meta yok" rozeti yapısal sinyale bağlandı** — `DataStatusBadges` artık meta
  eksikliğini core'un yapısal `missing_signals` ('meta') alanından okur (kesin sinyal:
  meta-rate satırı yok). Önceki `meta_score==0.3` sihirli-sabit tespiti, ~%50.1
  kazanma oranına sahip gerçek-meta şampiyonu yanlışlıkla "meta yok" sayabiliyordu;
  yapısal alan bu yanlış-pozitifi de giderir. (B-10)
- **OCE (oc1) Match-V5 yönlendirmesi** — Match-V5, OCE maçlarını SEA kümesinden
  sunar; account-v1 ise yalnız americas/asia/europe sunar. Eskiden her iki çağrı da
  `routingForRegion`→`americas` kullandığından OCE oyuncularının maç geçmişi
  Match-V5'te sessizce 404'lenip hiç eşitlenmiyordu. Yeni `matchRoutingForRegion`
  (oc1→`sea`) eklendi; `syncMatchHistory` ve Match-V5 ingestion ona geçti, account-v1
  yolu americas'ta kaldı. (B-12c)
- **Kırık görsel yedekleri (ilk-açılış/paketli)** — ban ikonları ve counter-item
  ikonları görsel yüklenemediğinde (404/403, ör. DDragon sync öncesi) kırık-görsel
  yerine dürüst yedek kutusu gösterir. Ban ikonu `BanIcon` bileşenine çıkarıldı
  (`onError` + iki ban bloğu DRY); counter-item ikonu `onError` yedeği kazandı. (B-01)
- **Dürüst "build verisi yok" durumu** — bir öneri geldiğinde ama o şampiyon için
  build verisi olmadığında (`build_source = "none"`) build kartı yanıltıcı
  "Build verisi yükleniyor…" yerine dürüstçe "Bu şampiyon için build verisi yok"
  gösterir. (B-05)
- **Veri durumu rozetlerinde öncelik** — düşük-veri/ilk-açılış durumunda en fazla 3
  rozet gösterilirken, kullanıcının aksiyon alabileceği uyarılar (meta yok, mastery
  yok, Riot anahtarı yok, canlı veri bayat) artık tanılayıcı rozetlerce (paket/kayıt/
  pipeline) ekrandan atılmıyor; aksiyon-alınabilir rozetler önceliklendiriliyor. (B-09)
- **Havuz önerilerinde dürüst yükleniyor durumu** — Lobi'deki havuz oluşturucu,
  oturum/öneriler henüz çözülürken "Bu rol için öneri yok" yerine "Öneriler
  yükleniyor…" gösterir; ilk açılışta yanıltıcı boş-durum kalkar. (B-16)
- **Edge worker: en taze patch seçimi** — toplu meta okunurken "en son patch"
  artık leksik string yerine ingest tazeliğine (`updated_at`) göre seçilir;
  "16.9" > "16.10" yanlış sıralaması nedeniyle bayat metanın taze sunulması
  giderildi. (B-14 — etkili olması için worker yeniden deploy edilmeli)
- **u.gg verisi doğru patch ile etiketlenir** — u.gg canlı patch'in 1-2 gerisinde
  veri sunduğunda, çekilen satırlar artık canlı patch yerine gerçek kaynak patch'iyle
  saklanır; böylece bayat u.gg verisi "güncel patch" sanılıp veri-tazeliği uyarısını
  yanlışlıkla bastırmıyor. (B-13)
- **Brezilya (BR) bölgesi için doğru Riot yönlendirmesi** — BR hesap/maç sorguları
  artık yanlış 'europe' yerine 'americas' bölgesel sunucusuna gider; eskiden BR
  kullanıcılarının maç geçmişi/öneri verisi sessizce boş dönüyordu. (B-12) (OCE için
  benzer düzeltme ayrı izleniyor.)
- **İstatistiklerde dürüst ince-veri durumu** — oyuncunun hiçbir şampiyonu en az 3
  maça ulaşmadığında, galibiyet oranı bölümü sessizce kaybolmak yerine "≥3 maçlık
  şampiyon yok" notu gösterir. (B-18)
- **Öğrenme verisi kaybı önlendi** — bir maç başlarken öneri→pick kaydı geçici bir
  veritabanı hatasıyla başarısız olursa, kayıt artık "yapıldı" sayılmayıp bir sonraki
  oyun-içi olayda yeniden denenir; o maçın yerel öğrenme etiketi sessizce kaybolmaz. (B-20)
- **Rol kaynağı etiketi dürüstleşti** — rol bir önceki oyundan hatırlanan tercihten
  geldiğinde, rol seçici artık yanıltıcı "Rolü sen seçtin" yerine "Geçen oyundan
  hatırlandı" der. (B-22)
- **İlk açılışta kendi şampiyon ikonların** — onboarding tamamlanırken şampiyon
  verisi (DDragon), maç geçmişi çekilmeden önce yüklenir; böylece yeni kullanıcının
  kendi uzmanlık şampiyonlarının ikonları ilk açılışta kırık (404) görünmez. (B-15)
- **İlk öneriler artık kişiselleştirilir** — uygulamayı champ-select açıkken başlatıp
  oyuncu kimliği henüz çözülmeden gelen ilk öneriler, kimlik çözülür çözülmez mevcut
  draft için otomatik olarak yeniden hesaplanır (mastery/konfor dahil) — bir sonraki
  hover/lock'a kadar kişiselleştirmesiz kalmaz. (B-11)
- **Öneri yarış-koşulu giderildi** — hızlı pick/ban akışında geç gelen eski bir öneri
  yanıtı artık yeni öneriyi ezmiyor; champ-select bittikten sonra geç gelen yanıt da
  bayat öneri yazmıyor. (B-26)
- **Erişilebilirlik: bildirimler ekran okuyucuya duyurulur** — toast bildirimleri
  artık ekran okuyucu tarafından okunur (hata/uyarı acil, bilgi/başarı kibar). (B-25)
- **Erişilebilirlik: sayaç ve bağlantı durumu** — champ-select geri sayım sayacı
  ekran okuyucuya "{{n}} saniye kaldı" olarak okunur ve bağlantı durumu (bağlanıyor/
  bağlantı yok/bağlı) değişimleri otomatik duyurulur. (B-29, B-31)
- **Bayat meta dürüstçe işaretlenir** — edge sunucusunun toplu meta verisi 48 saatten
  eskiyse (örn. veri toplama duraklamışsa) artık "düşük güven" olarak kaydedilir ve
  öneri/veri-durumu rozetlerine yansır; eskiden bayat veri sessizce "taze" sunuluyordu.
  (B-03 — worker tarafının etkili olması için yeniden deploy gerekir)
- **İstatistik kartları ilk açılışta dolar** — rank, trend ve haftalık özet kartları,
  oyuncu kimliği henüz çözülmeden açıldığında artık kalıcı boş kalmıyor; kimlik
  çözülür çözülmez verilerini otomatik çekiyor. (B-17)
- **Erişilebilirlik: modal odak yönetimi** — Ayarlar ve Şampiyon Detayı pencereleri
  açıldığında klavye odağı pencere içine taşınır, kapandığında onu açan öğeye geri
  döner (klavye/ekran-okuyucu kullanıcıları için). (B-28)

## [0.10.0-beta.6] — 2026-06-16

### Düzeltildi
- **Şampiyon ikonları görünmüyordu (paketli kurulum)** — ilk açılışta DDragon
  sürümü henüz sync olmadan `"unknown"` sentinel'i ikon URL'lerine gömülüyordu
  (`.../cdn/unknown/img/...` → 403), ikonlar baş-harf yedeğine düşüyordu. Artık
  sync öncesi servable bir fallback patch kullanılır ve sentinel reddedilir;
  ikonlar ilk açılıştan itibaren görünür, sync bitince canlı patch'e geçer.
  (Splash art sürüm içermediği için zaten etkilenmiyordu.)

## [0.10.0-beta.5] — 2026-06-16

beta.4'ten bu yana biriken büyük güncelleme: yapay zekâ koçluk, kazanma
olasılığı kalibrasyonu, yerel öğrenme, genişletilmiş combo bilgisi ve uçtan
uca doğrulama kalkanları.

### Eklendi
- **Yapay zekâ koç notu (opsiyonel, yerel)** — DeepDive'da, OpenAI-uyumlu bir
  yerel LLM (ör. Ollama) yapılandırılırsa gerekçeli koçluk notu üretir; varsayılan
  KAPALI ve veri makineden çıkmaz. Aday her zaman deterministik audit'ten geçer,
  geçemezse yerleşik koç notuna düşer.
- **Kazanma olasılığı rozeti** — öneri skorları, geçmiş sonuçlardan kalibre
  edilmiş bir kazanma olasılığına çevrilir (yeterli örnek altında gösterilmez).
- **Co-pick combo geçmişin** — bir combo ipucunda o eşli geçmişin (oynanan /
  kazanılan) görüntülenir.
- **Yerel öğrenme** — maç sonuçları yerelde etiketlenir (sunucuya GİTMEZ); yeterli
  örnek birikince öneri ağırlıkları muhafazakâr biçimde öğrenilir, deterministik
  motor hâlâ ana referans.
- **Genişletilmiş combo bilgisi** — denetlenmiş sinerji combo'ları 123 → 544.
- **Rank bağlamı** — soloQ ve flex rank'in (tier/division/LP + split rekoru ve
  win-rate) League Client'tan anahtar gerekmeden okunup istatistik sekmesinde
  gösterilir. Yalnız görüntü; öneri/karne motorunu etkilemez.
- Anonim geri bildirim yüklemesi için açık **rıza kapısı** (varsayılan KAPALI).

### İç / kalite
- Uçtan uca **app-launch + IPC smoke testi** (E2E), canlı LCU/Live-Client
  **wire-şekil testleri**, **IPC komut-kayıt sözleşmesi** ve sürüm-senkron
  kalkanları; boru hattı aşama log'ları (`[pipeline]`).

## [0.10.0-beta.3] — 2026-06-12

Koç sürümü: uygulama draft analistinden gerçek bir solo-q koçuna dönüştü —
maç karnesi + hedef döngüsü, kişisel form sinyali, seans koçu ve daha fazlası.

### Eklendi
- **Veritabanı kurtarma** — açılışta bozuk veritabanı tespit edilirse dosya
  SİLİNMEDEN `.corrupt-*` olarak kenara alınır, taze şema kurulur ve durum
  dürüstçe bildirilir; maç/meta verileri sync'lerle yeniden dolar.
- **İkon yedeği** — Data Dragon CDN'ine erişilemediğinde şampiyon ikonları boş
  kutu yerine baş harflerle görünür.
- **Onboarding netliği** — son adım artık ilk senkronun birkaç dakika
  sürebileceğini, paket veriyle başlanacağını, 1-5 kısayollarını ve League
  kapalıyken çalışmaya devam eden ekranları açıklıyor.
- Canlı duman checklist'i (`docs/live-smoke-checklist.md`) — her release öncesi
  koşulan elle doğrulama akışı (otomasyonun kapsamadığı canlı LCU/oyun yolları).
- **Seans Koçu** — oturum başında ısınma kontrol listesi (devreden maç hedefin
  otomatik madde olur); üst üste 2 kayıpta nazik not, 3+ kayıpta "15 dk ara"
  önerisi (seans W/L ile). Yalnız önerir, hiçbir şeyi engellemez; kapatılabilir.
- **Haftalık Özet** — istatistik sekmesinde son 7 günün hedef isabet oranı,
  W/L ve karne sayısı.
- **ARAM koçluğu derinleşti** — combo ve takım-ihtiyacı analizi artık ARAM'da
  da skora işliyor (sabit 5v5'te tam anlamlı); koridor-merkezli plan metni ve
  blind-pick risk eki ARAM'da bilinçli olarak kapalı.
- **Dalga yönetimi dersleri** — oyun içi plan kartında arketipine ve erken
  baskı durumuna göre wave tavsiyesi (slow-push/freeze/kule altı; 13 arketip ×
  3 durum, ölçülü dil).
- **Güç penceresi hatırlatıcıları** — overlay, 1:30-3:30 arasında Lvl 2-3
  penceresini, 8:00-11:00 arasında "rakip genelde 6'ya basar" uyarısını
  hatırlatır (yalnız public oyun saati; "beklenen" dili).
- **+14 yeni combo** (109→123) — Alistar/Gragas/Nautilus/Zac+Yasuo,
  Jarvan+MF, Amumu+Karthus, Rell+MF, Camille+Orianna, Braum/Nami+Lucian,
  Lulu+KogMaw, Zilean+Yi, Shen+Twitch, Tahm+Senna; tümü mekanik-doğrulanmış
  ability referanslarıyla.
- **Veto & tercihler** — şampiyon detay kartından "Asla önerme" (öneri
  listesinden tamamen çıkar) ve "Öğreniyorum" (sınırlı pozitif boost)
  işaretlenebilir; tamamen yerel.
- **Meta trend çipi** — aktif önerinin u.gg win-rate'i son snapshot'tan beri
  anlamlı oynadıysa pick ekranında ▲/▼ rozeti; yalnız bilgi amaçlı, skora
  etkisi yok.
- **Kişisel form sinyali** — öneriler artık her şampiyonda NASIL oynadığını da
  bilir: bu roldeki kendi CS/dk ve ölüm-oranı medyanına karşı şampiyon-başına
  form okuması (az maçta nötre çekilir), skora sınırlı (±0.05) etki eder ve
  kartta görünür; bu rolde o şampiyonla maçın yoksa dürüstçe "form verisi yok".
- **Trend Panosu** — baskın rol+kuyruktaki son maçların CS/dk, ölüm-oranı ve
  vizyon sparkline'ları + "yükseliyor/sabit/geriliyor" hükümleri (ilk-yarı vs
  ikinci-yarı medyan; 8 maç altında dürüstçe yalnız eğri).
- **Maç Notları** — karne kartında bu maç için serbest not + etiket çipleri
  (tilt/wave/vizyon/makro); yalnız yerel veritabanında kalır.
- **Kuyruk-ayrımlı konfor sinyali** — ARAM draft'ı yalnız ARAM maçlarından,
  Sihirdar Vadisi draft'ı yalnız SR maçlarından kişisel win-rate okur; iki mod
  birbirinin önerilerini artık kirletmez.
- **Koç Döngüsü: Maç Sonu Karnesi + Sonraki Maç Hedefi** — her senkronlanan maç,
  SENİN aynı rol+kuyruk geçmişinin medyanına karşı notlanır (CS/dk, 10 dk başına
  ölüm, KDA, vizyon; timeline metrikleri key'siz dürüstçe "kilitli"); karne "iyi
  giden 1 şey + düzeltilecek 1 şey" der, TEK ölçülebilir hedef bırakır ve SONRAKİ
  maçta hedefi kontrol eder (✓/✗ + ardışık tutturma serisi). İstatistik sekmesinde.
- **Rakip hover uyarısı pick fazında** — rakibin hover'ladığı şampiyonlar artık
  ban fazına ek olarak pick ekranında da görünür (client'ta zaten görünen bilgi).
- **Takım sohbeti yardımcısı** — takım kompozisyonu eksiklerinden (engage/ön saf/
  full AD-AP/peel) kopyalanabilir 1-2 sohbet önerisi; yalnız panoya kopyalar,
  LCU sohbetine asla yazmaz.
- **Sesli makro uyarıları** (varsayılan KAPALI) — dragon/baron/herald penceresine
  60 ve 30 sn kala kısa bip; ayarlardan açılır.
- **Tam skor şeffaflığı** — skor kırılımında 6 sinyalin tamamı + güven temeli
  ("yeterli örneklem" / "sinyaller çelişti") + eksik sinyal listesi.

## [0.10.0-beta.2] — 2026-06-12

İlk otomatik güncelleme turu: beta.1 kurulumları bu sürümü kendiliğinden çekmeli.

### Eklendi
- **u.gg'den tam rune sayfası** — ikincil ağaç + rune'lar, stat shard'ları ve
  skill order (örn. "E→Q→W") artık 170+ şampiyonun tamamı için canlı veriden
  gelir (eskiden yalnız ~40 seed şampiyonunda vardı).
- Seed tazeleme aracı (`scripts/refresh-seeds.mjs`) ve performans baseline
  ölçer (`scripts/benchmark/baseline.mjs`; motor gecikmesi p95 ≈ 4 ms).

### Kaldırıldı
- **Rakip havuzu + lobi scouting** — champ select'te LCU rakip `summonerId`
  vermediği için rakip/takım arkadaşı şampiyon havuzları hiçbir zaman
  dolmuyordu; ölü özellik tüm katmanlardan söküldü (UI kartı, IPC komutları
  `get_enemy_champion_pools`/`get_lobby_scouting`, core `scouting` modülü).
  Ban önerileri aynen çalışmaya devam ediyor.

### Düzeltildi
- **Öneri zenginleştirme Electron'da tamamlandı** — Tauri'den kalan açık
  kapandı: küratörlü seed build'ler (matchup-özel + pozisyon varsayılanı),
  `missing_signals` dürüstlük bayrakları, Leaguepedia pro-presence rozeti ve
  DraftBrain pack yükseltmesi (model skoru, tier, skor dökümü, lane/orta-oyun
  planları, karşılaştırmalı "neden o değil" notları) artık core'da çalışıyor
  ve hem öneri listesinde hem tek-şampiyon analizinde uygulanıyor.

### Değişti
- **Bayesian meta yumuşatması** — düşük örneklemli win-rate'ler 0.50 önseline
  çekilir (prior_n=200); 60 maçlık bir "yükseliş", 10k maçlık kanıtlanmış bir
  seçimi öneri/ban/havuz sıralamasında artık geçemez. Gösterilen win-rate ham
  kalır; yalnız skor kararları yumuşatılır.

## [0.10.0-beta.1] — 2026-06-12

Electron çağı: masaüstü host tamamen değişti, veri tabanı bulut destekli derinleşti.

### Değişti
- **Electron'a tam geçiş** — Tauri host emekli edildi; masaüstü artık
  Electron + Rust/WASM core (csa-core). Tüm komutlar parite ile taşındı,
  öneri motoru aynı saf çekirdekte çalışıyor.

### Eklendi
- **Otomatik güncelleme** — electron-updater + GitHub Releases; paketli
  uygulama açılışta sessizce kontrol eder, indirir, kapanışta kurar.
- **Cloud edge veri kaynağı** — Cloudflare Worker, Match-V5 maçlarından
  win/pick/ban + lane matchup + build agregasyonu toplar; uygulama
  `EDGE_BASE_URL` ayarlıysa bu kaynağı kendiliğinden kullanır.
- **Maç sonrası derin istatistikler** — farm@10, erken ölüm (ilk 14 dk) ve
  vizyon skoru ortalamaları; yeterli örneklemde erken-ölüm dersi.
- **In-game güç penceresi** — senin ve rakip laner'ın güç eğrileri
  karşılaştırılarak baskı/sabır penceresi okuması.
- **i18n tamamlandı** — champ-select / lobi / bağlantı bileşenleri TR/EN.
- Public beta hazırlığı: LICENSE (proprietary), PRIVACY, TERMS, CHANGELOG.

## [0.9.0-beta.1] — 2026-05-22

İlk kapalı beta (closed beta, CB-1) adayı.

### Eklendi
- **LCU-first match history sync** — kullanıcıdan Riot developer API key
  istenmez; veri doğrudan League Client'tan okunur.
- **Champ-select öneri motoru** — comfort (mastery + maç geçmişi), lane/team
  counter, synergy ve meta sinyallerini birleştirerek en iyi 5 öneri.
- **Draft IQ** — her öneri için win condition, combo ve risk açıklaması.
- **Ban önerileri** — en yüksek tehdit mantığıyla ban fazı desteği.
- **Build/rune özeti** — seed veri (20 şampiyon) + Meraki Analytics rate verisi.
- **Onboarding wizard** (4 adım) ve sunucu/dil ayarları (TR/EN).
- **In-game overlay** — oyun başlayınca kompakt pencere, oyun bitince kullanıcı
  tercihine (compact/standard/wide) dönüş.
- Snapshot test altyapısı (insta, 5 fixture), Vitest frontend testleri.

### Güvenlik / Gizlilik
- Telemetry yok; hiçbir analitik servisine veri gönderilmez.
- Uygulama otomatik lock/ban/pick **yapmaz** — yalnızca öneri gösterir.
- API key binary'ye gömülmez; `.env`'de bulunması isteğe bağlıdır.

### Bilinen Sorunlar
- İmzasız build — Windows SmartScreen uyarısı çıkabilir ("Yine de çalıştır").
- Auto-updater kapalı (`active: false`); güncellemeler manuel indirilir.
- Lolalytics meta kaynağı ertelendi (Cloudflare/ToS); seed + Meraki kullanılır.

[Unreleased]: https://github.com/Berkilic41/champ-select-assistant/compare/v0.9.0-beta.1...HEAD
[0.9.0-beta.1]: https://github.com/Berkilic41/champ-select-assistant/releases/tag/v0.9.0-beta.1
