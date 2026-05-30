# Sprint 6 — Lolalytics Meta Source Spike

**Tarih:** 2026-05-28  
**Karar:** DEFERRED

---

## Spike Soruları ve Bulgular

### 1. Public JSON endpoint var mı?

**Hayır.** Lolalytics, verileri Cloudflare koruması altındaki HTML sayfaları üzerinden sunar.
Mevcut `src-tauri/src/meta/lolalytics.rs` stub'u bunu zaten not etmiştir:

> "HTML sayfa döner, JSON extract etmek gerekir"
> "Cloudflare koruma varsa Err döner — CDragon fallback kullanılıyor"

Belgelenmiş ve genel erişime açık bir JSON API uç noktası bulunmuyor.
İç API uç noktaları belgelenmemiş ve deploy'lar arasında kırılmaya açık.

### 2. ToS ne diyor?

Lolalytics Terms of Service otomatik veri toplama ve scraping'i yasaklamaktadır.
Cloudflare korumasını atlatmak teknik olarak mümkün olsa da ToS ihlali sayılır.

### 3. Endpoint şekli sabit mi?

Sabit değil. HTML render'ı JavaScript bağımlı, yapı deploy'lar arasında değişebilir.
Bir site güncellemesi tüm scraper'ı kırabilir — fragile bağımlılık.

### 4. Rate limit / robots.txt

- `robots.txt`: Automated crawling disallowed.
- Cloudflare: Her istek için challenge token gerekir, headless browser olmadan aşılamaz.
- Rate limit: Belirsiz, ancak Cloudflare agresif şekilde bloklar.

---

## Karar Gerekçesi

| Kriter | Durum |
|--------|-------|
| Public JSON API | Yok |
| ToS uyumluluğu | Scraping yasak |
| Güvenilirlik | Cloudflare → fragile |
| Bakım maliyeti | Yüksek (site güncellemelerine duyarlı) |

**DEFERRED.** Sprint 4 (`import_builds`) ve Sprint 5 (`import_matchups`) manuel seed verileri
kapalı beta için yeterlidir. Meraki Analytics entegrasyonu (`sync_meraki_rates`) meta sinyali
sağlamaktadır.

---

## Mevcut Durum (Yeterli)

| Kaynak | Kapsam | Komut |
|--------|--------|-------|
| Meraki Analytics | Win/pick/ban rate — tüm şampiyonlar | `sync_meraki_rates` |
| Build seed | 20 şampiyon × ana rol | `import_builds` |
| Matchup seed | 50 lane matchup | `import_matchups` |

---

## Gelecekte Yeniden Değerlendirme

Public release öncesi aşağıdaki alternatifler araştırılabilir:

- **CDragon tam veri seti** — community-maintained, CC lisansı altında JSON
- **op.gg veya u.gg** — public JSON endpoint araştırması (spike gerekir)
- **Kendi veri toplama pipeline'ı** — Riot API match history → lokal istatistik

Lolalytics implementasyonu `src-tauri/src/meta/lolalytics.rs` stub olarak kalır.
