# Changelog

Bu projedeki kayda değer tüm değişiklikler bu dosyada belgelenir.
Format [Keep a Changelog](https://keepachangelog.com/), versiyonlama
[Semantic Versioning](https://semver.org/) temellidir.

## [Unreleased]

### Eklendi
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
