# CB-1 — Closed Beta Validation
# Champ Select Assistant v0.9.0-beta.1

> Durum: RC Hazır · Tarih: 2026-05-22

## 1. Bağlam ve Durum

- RC artifact hazır: `0.9.0-beta.1`
- Updater beta için kapalı (`active: false`)
- LCU-first match history tamamlandı — developer key normal kullanıcıdan istenmiyor
- `platform_region` fix tamamlandı (TR, EUW vb. sunucu seçimi Settings'te)
- Amaç: gerçek LoL client ve gerçek kullanıcı akışında doğrulama

---

## 2. Beta Scope Freeze

CB-1 sırasında **yeni feature eklenmez.**

Yalnızca şu düzeltmeler kabul edilir:
- S1/S2 bugfix
- Crash fix
- LCU sync fix
- Yanlış recommend-only copy
- Layout taşması
- Kurulum/build sorunu
- Güvenlik/privacy blocker

**Kapsam dışı:** Yeni Draft IQ logic, Settings presets, KB genişletme,
Riot API mimarisi değişikliği, UI refactor, herhangi bir yeni feature.

---

## 3. RC Artifact Checklist

### Versiyon
- [ ] `src-tauri/Cargo.toml` version: `0.9.0-beta.1`
- [ ] `src-tauri/tauri.conf.json` version: `0.9.0` (MSI uyumu)
- [ ] `plugins.updater.active: false`
- [ ] MSI output mevcut: `target/release/bundle/msi/*.msi`
- [ ] NSIS output mevcut: `target/release/bundle/nsis/*-setup.exe`

### İçerik
- [ ] Changelog kısa özeti yazıldı
- [ ] Known issues listesi yazıldı (özellikle SmartScreen uyarısı)
- [ ] Onboarding 2. adım: "API key gerekmez" mesajı net
- [ ] Settings: platform_region default `tr1`
- [ ] ErrorBanner: developer key veya `developer.riotgames.com` yönlendirmesi yok

### Baseline

```
pnpm typecheck
pnpm test:run
cd src-tauri
cargo test --all
cargo clippy --all-features -- -D warnings
cargo fmt --all -- --check
```

- [ ] Tüm baseline komutları sıfır hata/uyarı ile tamamlandı

---

## 4. Smoke Test Matrix

Her senaryo için: ✅ Geçti · ❌ Başarısız · ⚠ Kısmi

### A. Kurulum ve Açılış

| # | Senaryo | Beklenen |
|---|---------|---------|
| A1 | MSI veya NSIS installer çalıştır | Sorunsuz kurulum |
| A2 | Uygulamayı ilk kez aç | Onboarding wizard 4 adım görünüyor |
| A3 | Tüm onboarding adımlarını geç | "Başla!" → Lobby ekranı açılıyor |
| A4 | Ayarlar aç | Slider'lar, Sunucu select ve Dil seçici görünüyor |
| A5 | Settings → Kaydet | Toast çıkıyor, ayarlar kaydediliyor |
| A6 | Settings → İptal | Değişiklikler kaydedilmiyor |
| A7 | Sunucu: TR seçip Kaydet | `platform_region: "tr1"` kaydediliyor |
| A8 | LoL client **kapalıyken** "Maç geçmişini yükle" | "League Client açık değil" mesajı |
| A9 | LoL client **açıkken** uygulama başlat | Otomatik bağlantı veya bağlan çalışıyor |

### B. LCU-first Sync

| # | Senaryo | Beklenen |
|---|---------|---------|
| B1 | Riot API key `.env`'de **yokken** sync | Sync çalışıyor, hata mesajı yok |
| B2 | TR sunucu seçili → "Maç geçmişini yükle" | Backend: `sync_lcu_player_data(region=tr1)` |
| B3 | Sync sonrası "Şampiyonlarım" sekmesi | Şampiyon listesi görünüyor |
| B4 | Sync sonrası "İstatistiklerim" sekmesi | Win rate ve games played görünüyor |
| B5 | `matches_synced` değeri | Sıfır değil (en az 1 maç) |
| B6 | `masteries_synced` değeri | Sıfır değil (en az 1 şampiyon) |
| B7 | Lobby → Patch badge | "Patch {version}" formatında badge görünüyor |
| B8 | LCU **kapalıyken** sync | "League Client açık değil" — API key prompt YOK |

### C. Champ Select

| # | Senaryo | Beklenen |
|---|---------|---------|
| C1 | Normal SR (SQ/Flex) kuyruğuna gir | Champ select açılınca öneriler geliyor |
| C2 | Ban phase | Ban önerileri görünüyor, ban hint metni doğru |
| C3 | Pick phase — HeroCard ana yüzü | Şampiyon adı + en az 1 karar chip görünüyor |
| C4 | HeroCard "Detay ↓" aç | DraftPlanPanel: Win Condition, Combo, Riskler |
| C5 | Stretch pick varsa | "Stretch pick riski" chip kırmızı görünüyor |
| C6 | Klavye kısayolu [1]-[5] | Farklı öneri seçiliyor |
| C7 | Finalization fazı | Başlık "Seçim kilitlendi — Build planı:" — "Kilitleniyor" yok |
| C8 | ARAM kuyruğu (varsa) | Öneriler geliyor, lane bilgisi gösterilmiyor |
| C9 | Uygulama champ select'te lock/ban yapmıyor | Sadece hover intent ayarlıyor |

### D. Stabilite

| # | Senaryo | Beklenen |
|---|---------|---------|
| D1 | Champ select sırasında LoL client kapat | Uygulama crash etmiyor |
| D2 | Uygulama yeniden başlat | Settings (server, dil, ağırlıklar) korunuyor |
| D3 | Oyun başlayınca | Overlay pencere açılıyor (küçük banner) |
| D4 | Oyun bitince (EndOfGame) | Pencere kullanıcı tercihine (compact/standard/wide) dönüyor |
| D5 | 30 dakika açık bırak (idle) | Crash veya belirgin yavaşlama yok |
| D6 | Birden fazla champ select | İkinci sefer de öneriler geliyor, hafıza artmıyor |

---

## 5. LCU Canlı Kanıt Tablosu

> **Kural:** Hedef, API key olmadan LCU-first akışın çalışmasıdır.
> API key yalnızca teknik fallback testi için kullanılır.

| Tester | Sunucu | API Key Var mı? | Match Sync | Mastery Sync | matches_synced | masteries_synced | Hata Mesajı | Not |
|--------|--------|-----------------|------------|--------------|----------------|------------------|-------------|-----|
| T1 | | | ✅/❌ | ✅/❌ | | | | |
| T2 | | | ✅/❌ | ✅/❌ | | | | |
| T3 | | | ✅/❌ | ✅/❌ | | | | |
| T4 | | | ✅/❌ | ✅/❌ | | | | |
| T5 | | | ✅/❌ | ✅/❌ | | | | |

---

## 6. Tester Dağıtım Planı

### Hedef: 5-10 Tester

| Profil | Kota | Amaç |
|--------|------|------|
| Casual (ranked değil, <100 oyun/yıl) | 2 | Onboarding anlaşılırlığı, ilk izlenim |
| Regular (ranked, 200-500 oyun/yıl) | 3 | Champ select, öneri kalitesi |
| Veteran (Diamond+, 1000+ oyun) | 2 | Edge case, Draft IQ doğruluğu, yanlış öneri tespiti |
| Technical (yazılımcı) | 1-2 | Hata raporu kalitesi, log analizi, crash tespiti |

### Tester Kurulum Notu

**Neden denemeye değer (doğrulanabilir iddialar):**
- 172/172 şampiyon draft analizi (DDragon'a karşı test-zorunlu kapsam)
- Güncel patch (16.11) — oranlar Meraki, statik veri DDragon ile otomatik
- 109 ability-referanslı combo + 80 lane matchup + tüm-çift arketip counter
- Az veri/yeni patch durumları "güven" etiketiyle dürüstçe gösterilir
- Otomatik lock/ban yok · API key gerekmez · telemetry yok

```
Champ Select Assistant v0.9.0-beta.1 — Kurulum Adımları

1. MSI veya EXE installer çalıştır.
   Windows SmartScreen uyarısı çıkarsa: "Yine de çalıştır" seç.
   Bu imzasız bir beta build'idir — tanımlı publisher yoktur.

2. League of Legends client'ını aç ve giriş yap.

3. Champ Select Assistant'ı başlat.

4. "Maç geçmişini yükle" butonuna tıkla.
   Riot developer API key gerekmez — verini doğrudan LoL client'tan okur.

Önemli:
- Uygulama otomatik lock, ban veya pick YAPMAZ.
- Sadece öneri ve oyun planı gösterir.
- Kendi seçimini kendin yaparsın.

Hata bildirimi:
- Ekran görüntüsü veya kısa açıklama yeterli.
- Hangi aşamada olduğunu yaz (onboarding / lobby / champ select / oyun içi).
```

---

## 7. Feedback Form Soruları

### A. Güven ve Anlaşılabilirlik

1. Bir şampiyonun neden önerildiğini anladın mı?  
   `[ ] Evet, net  [ ] Kısmen  [ ] Hayır, anlamadım`

2. Ana karttaki kısa chip'ler (örn. "Nocturne ile güçlü combo", "Blind pick güvenli") yeterli bilgi verdi mi?  
   `1 — Hiç yardımcı değil · · · 5 — Çok yardımcı`

3. "Detay" açıldığında DraftPlanPanel'deki bilgiler (Win Condition, Combo, Riskler) faydalı mıydı?  
   `1 — Hiç faydalı değil · · · 5 — Çok faydalı`

### B. LCU ve Veri

4. "Maç geçmişini yükle" butonuna tıkladığında maç geçmişin geldi mi?  
   `[ ] Evet  [ ] Hayır  [ ] Kısmen / gecikmeyle`

5. Mastery verilerin (hangi şampiyonları çok oynadığın) yansıdı mı?  
   `[ ] Evet  [ ] Hayır  [ ] Emin değilim`

6. API key girmeden uygulamanın çalıştığı yeterince net miydi?  
   `[ ] Evet, netti  [ ] Belirsizdi  [ ] Hayır, key lazım sandım`

### C. Vanguard / Güven Algısı

7. Bu uygulamanın seni ban ettirebileceğini düşündüğün bir an oldu mu?  
   `[ ] Hiç  [ ] Bir an geçti aklımdan  [ ] Endişe yarattı`

8. Uygulamanın otomatik lock veya ban yapmadığı yeterince netti mi?  
   `[ ] Evet, tamamen net  [ ] Belirsizdi  [ ] Hayır, otomatik yapabilir sandım`

9. Uygulamanın League Client verini okuması seni rahatsız etti mi?  
   `[ ] Hayır  [ ] Biraz  [ ] Evet, rahatsız etti`

### D. Genel Puan

10. Bu uygulamayı bir League oyuncusu arkadaşına tavsiye eder misin? (NPS, 0-10)  
    `0 — Asla · · · 10 — Kesinlikle`

11. En kafa karıştıran yer neresiydi? *(Serbest metin)*

12. En değerli bulduğun özellik neydi? *(Serbest metin)*

13. Public beta'ya çıkmadan önce **kesin düzelmeli** dediğin bir şey var mı? *(Serbest metin)*

---

## 8. Bug Severity Sınıflandırması

### S1 — Critical (Anında Düzelt, Beta Durdurulabilir)
- Uygulama crash oluyor ve otomatik kapanıyor
- Uygulama açılmıyor
- Uygulama yanlışlıkla lock/ban/pick yapıyor (Riot ToS riski)
- PUUID, API key veya kişisel veri sızıntısı
- Vanguard/ban riski yaratabilecek herhangi bir davranış

### S2 — Major (Kapalı Beta Bitmeden Düzelt)
- LCU sync tamamen çalışmıyor (LoL açıkken bile)
- Match history veya mastery verisi hiç gelmiyor
- Champ select önerileri görünmüyor
- Settings server/region yanlış kaydediliyor
- Kullanıcıya developer key veya `.env` yönlendirmesi yapılıyor
- Onboarding tamamlanamıyor

### S3 — Minor (Public Beta Öncesi Düzelt)
- Copy hatası (yazım, diacritics eksikliği)
- Layout taşması veya UI kırılması
- Badge okunmuyor (renk, boyut)
- Yanlış ama blocker olmayan UI davranışı

### S4 — Backlog (Sonraki Sürümde)
- Feature request
- KB genişletme isteği (daha fazla şampiyon/combo)
- Settings preset isteği
- Yeni Draft IQ sinyal önerisi
- Performans iyileştirme önerisi

---

## 9. Privacy ve Veri Sınırı

- Kullanıcıdan Riot developer API key **istenmez**, beklenmez.
- LCU verisi yalnızca o anda LoL'a giriş yapan yerel oyuncuya aittir.
- PUUID, match ID veya başka kişisel tanımlayıcı public beta öncesi loglardan temizlenir.
- API key binary'ye gömülmez; `.env`'de bile bulunması isteğe bağlıdır.
- **Telemetry yok:** Uygulama herhangi bir analitik servisine veri göndermez.
- Telemetry eklenecekse kullanıcı açık rızası zorunludur.

### Log Audit Checklist

Beta öncesi log çıktıları aşağıdaki hassas verilerden arındırılmış olmalı:

- [ ] PUUID loglanmıyor
- [ ] API key (`RGAPI-…`) loglanmıyor
- [ ] Match ID loglanmıyor
- [ ] Summoner display name loglanmıyor (log'da gerekiyorsa maskelenmeli)
- [ ] `.env` dosyası veya içeriği loglara düşmüyor
- [ ] `tracing::info!` çıktılarında hassas alan yok

**Public beta öncesi hazırlanması gerekenler:**
- Privacy Policy (LCU verisinin yerel kaldığını açıklar)
- Terms of Service (recommend-only, otomatik aksiyon yok)
- Riot Games third-party disclaimer
- Uygulama imzası ve SmartScreen stratejisi

---

## 10. Go / No-Go Kriterleri

Closed beta sonunda **tek karar** alınır.

### GO — Public Beta'ya Geç

Tüm maddeler sağlanmalı:

- [ ] S1 bug sayısı = **0**
- [ ] S2 bug sayısı = **0**
- [ ] LCU sync başarı oranı ≥ **%80** (testerların en az %80'i B5-B6'yı geçti)
- [ ] NPS ortalaması ≥ **7**
- [ ] Testerların çoğunluğu "neden önerildiğini anladım" yanıtı verdi
- [ ] En az **5 farklı gerçek LoL akışı** tamamlandı (ban/pick/ARAM vb.)
- [ ] En az **3 farklı server** doğrulandı — **hedef: 5 server**
- [ ] Developer key prompt hiçbir tester tarafından görülmedi
- [ ] Auto-lock/auto-ban **algısı oluşmadı** (soru 8 çoğunlukla "net")

### NO-GO — Kapalı Beta Devam Eder

Herhangi biri gerçekleşirse public beta ertelenir:

- Herhangi S1 bug
- S2 bug sayısı > 0
- LCU sync güvenilmez (<%80 başarı)
- Testerlar ban veya Vanguard riski hissediyor
- Öneriler anlaşılmıyor (soru 1-3 ortalaması < 3/5)
- Kurulum veya ilk açılış birden fazla tester için sorunlu

---

## 11. Public Beta Öncesi Blocker Listesi

- [x] **Updater/signing kararı:** `active: false` bilinçli karar olarak `docs/release-checklist.md`'de dokümante edildi
- [x] **Windows SmartScreen stratejisi:** Tester talimatı (beta) yazıldı (CB-1 §6); imzalı sertifika public release'e ertelendi
- [x] **Privacy Policy** hazırlandı — `PRIVACY.md`, README'den linkli
- [x] **Terms of Service** hazırlandı — `TERMS.md` (recommend-only, otomatik aksiyon yok, Riot ToS uyumu)
- [x] **Riot Games Third-Party Disclaimer** — Riot'un tam zorunlu metniyle hizalandı (in-app onboarding, `LICENSE`, `TERMS.md`, `README.md`, `PRIVACY.md`, tauri bundle)
- [ ] **Riot Developer Portal kaydı/audit** — public öncesi BLOCKER (`docs/api-key-policy.md`). Closed beta LCU-first ile çalışır; production API key başvurusu + audit public dağıtımdan önce tamamlanmalı
- [x] **Riot 3. parti uyumluluk denetimi** — recommend-only; `hover_champion` yalnız hover (lock/ban/pick yok, kullanıcı-tetikli); Vanguard-safe (sadece LCU HTTP); LCU resmî-değil/tolere kategori (Blitz/op.gg ile aynı)
- [x] **Download sayfası** — GitHub Releases yayında: https://github.com/Berkilic41/champ-select-assistant/releases/tag/v0.9.0-beta.1 (MSI + NSIS installer ekli)
- [x] **CHANGELOG.md** oluşturuldu (0.9.0-beta.1 girişi)
- [x] **Contact/Support kanalı** — GitHub Issues (tüm legal dosyalarda + README'de linkli)

---

## 12. Çıktı

CB-1 tamamlandığında aşağıdaki belgeler üretilir:

| Çıktı | İçerik |
|-------|--------|
| **Tester Sonuç Tablosu** | LCU Canlı Kanıt Tablosu doldurulmuş hali |
| **Bug Listesi** | S1/S2/S3/S4 kategorili, tüm tester bulguları |
| **NPS Ortalaması** | Soru 10 cevaplarının ortalaması |
| **LCU Sync Başarı Oranı** | B5/B6 geçen tester / toplam tester |
| **Go / No-Go Kararı** | Tek satır: GO veya NO-GO + gerekçe |
| **Public Beta Hotfix Listesi** | NO-GO durumunda düzeltilecek S1/S2 bug'lar |
