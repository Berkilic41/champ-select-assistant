# Changelog

Bu projedeki kayda değer tüm değişiklikler bu dosyada belgelenir.
Format [Keep a Changelog](https://keepachangelog.com/), versiyonlama
[Semantic Versioning](https://semver.org/) temellidir.

## [Unreleased]

### Eklendi
- **Sürekli otonom geliştirme sistemi** — repo kökünde yönetim dosyaları
  (`AGENTS`, `PROJECT_STATE`, `BACKLOG`, `TASKS`, `DECISIONS`, `QUALITY_CHECKS`)
  ve Inspect→Discover→Prioritize→Delegate→Implement→Verify→Document→Continue
  döngüsü kuruldu. Her iterasyon küçük, test-geçen, geri-alınabilir bir
  iyileştirme üretir; otomatik commit yok.

### Değişti
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
