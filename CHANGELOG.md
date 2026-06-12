# Changelog

Bu projedeki kayda değer tüm değişiklikler bu dosyada belgelenir.
Format [Keep a Changelog](https://keepachangelog.com/), versiyonlama
[Semantic Versioning](https://semver.org/) temellidir.

## [Unreleased]

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
