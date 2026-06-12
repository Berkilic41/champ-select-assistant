# Changelog

Bu projedeki kayda değer tüm değişiklikler bu dosyada belgelenir.
Format [Keep a Changelog](https://keepachangelog.com/), versiyonlama
[Semantic Versioning](https://semver.org/) temellidir.

## [Unreleased]

### Eklendi
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
