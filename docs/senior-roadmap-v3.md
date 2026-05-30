# champ-select-assistant - Senior Roadmap v3

> Tarih: 2026-05-19  
> Amaç: Projeyi senior developer standardına taşımak; ürün, UI/UX, içerik, mimari, kalite, performans ve release süreçlerini aynı seviyeye getirmek.

## 1. Mevcut Durum

Proje artık ilk sprint planının çok ötesinde:

- Stack: Tauri v2, React 19, Vite 7, TypeScript, Rust, SQLite.
- Mevcut modüller: `lcu`, `db`, `ddragon`, `riot`, `recommendation`, `meta`, `commands`.
- Mevcut ürün akışı: onboarding, settings, LCU bağlantısı, champ-select poller, Riot sync, Data Dragon/CDragon cache, öneri ekranı.
- Doğrulama: `pnpm typecheck` başarılı. `cargo test` başarılı: 26 test geçti.
- Teknik borç sinyalleri: 15 Rust warning, template README, frontend test/e2e/lint eksikliği, repo kökünde yanlışlıkla oluşmuş boş dosyalar, canlı UI görsel QA eksikliği.

Senior hedef: "Çalışıyor" seviyesinden "güvenilir, hızlı, bakımlı, açıklanabilir ve iyi tasarlanmış ürün" seviyesine geçmek.

## 2. Lider + Bağımsız Yan Agent Modeli

Bu modelde agentlar subagent gibi birbirinin altında çalışmaz. Hepsi bağımsız iş sahibi gibi davranır; tek merkezde `lol-lead` karar verir, scope çakışmalarını önler ve kalite kapısından geçirir.

| Rol | Sahiplik | Ana çıktı | Dokunmaması gerekenler |
| --- | --- | --- | --- |
| `lol-lead` | Ürün hedefi, roadmap, merge sırası, kalite kapıları | Sprint brief, issue packet, final karar | Mikro implementasyon |
| `lol-product` | Kullanıcı problemi, rakip analiz, feature önceliği | Product spec, acceptance criteria | Rust/React kodu |
| `lol-architect` | Modül sınırları, Tauri command kontratları, veri akışı | Architecture decision record | CSS polish |
| `lol-rust` | `src-tauri/src/**`, DB, LCU, Riot, recommendation backend | Backend patch, Rust testleri | React bileşenleri |
| `lol-frontend` | `src/**`, state management, component davranışı | UI patch, typecheck/build | Rust command implementasyonu |
| `lol-ux-content` | Champ-select akışı, ekran hiyerarşisi, mikro metinler | UX review, copy deck, accessibility notları | DB/Riot entegrasyonu |
| `lol-data` | Meta veriler, matchup/build kaynakları, cache stratejisi | Data quality report, ETL planı | Görsel UI polish |
| `lol-qa` | Test stratejisi, mock LCU/Riot, e2e, regression suite | Test patch, QA matrix | Ürün scope kararı |
| `lol-perf` | Startup, event latency, render latency, DB sorguları | Benchmark report, perf fixes | Feature genişletme |
| `lol-security-release` | Riot policy kontrolü, secrets, updater, packaging | Security checklist, release checklist | Recommendation scoring |

### Yan Agent Çalışma Kuralları

1. Her agent tek issue packet ile başlar: amaç, bağlam, sahip olduğu dosyalar, dokunmayacağı dosyalar, kabul kriterleri.
2. Aynı anda çalışan agentların write scope'u çakışmaz.
3. Her agent işi bitirince `docs/handoffs/YYYY-MM-DD-<agent>.md` formatında kısa handoff bırakır.
4. `lol-lead` merge sırasını belirler: architecture -> backend contract -> frontend -> QA -> review.
5. Lead onayı olmadan "büyük refactor" yapılmaz.
6. Her sprint sonunda sadece çalışan kod değil, ölçülmüş kalite teslim edilir.

### Issue Packet Şablonu

```md
## Agent
lol-rust

## Objective
Champ-select öneri komutunda typed error ve ölçülebilir latency ekle.

## Context
Mevcut dosyalar: src-tauri/src/commands/champ_select.rs, src-tauri/src/recommendation/**

## Owns
- src-tauri/src/commands/champ_select.rs
- src-tauri/src/recommendation/**
- src-tauri/src/db/** sadece gerekli test fixture ekleri

## Do Not Touch
- src/components/**
- package.json

## Acceptance
- cargo test geçer
- cargo clippy warning üretmez veya mevcut warning sayısı azalır
- get_recommendations hata durumlarında kullanıcıya anlamlı error döner
- latency tracing logu eklenir

## Handoff
Ne değişti, riskler, test komutları, sonraki agent için notlar.
```

## 3. Ürün Kuzey Yıldızı

Champ-select yaklaşık 20-30 saniyelik bir karar anı. Uygulama bu anı kalabalıklaştırmamalı; oyuncuya hızlı, güvenilir ve açıklanabilir seçim desteği vermeli.

Ana ürün vaadi:

- "Şu anki takım ve rakip kompozisyonuna göre sana en iyi 5 güvenilir seçimi göster."
- "Neden önerdiğini tek satırda açıkla."
- "Oyuncunun konforunu, son maçlarını, meta değerini ve takım ihtiyacını birlikte değerlendir."
- "Pick zamanı geldiğinde kullanıcı düşünmek yerine karar verebilsin."

## 4. Öncelikli Yol Haritası

### Faz 0 - Repo Hijyeni ve Güvenli Başlangıç

Süre: 0.5-1 gün  
Sahipler: `lol-lead`, `lol-qa`

Hedef: Projeyi temiz, ölçülebilir ve devam edilebilir hale getirmek.

Yapılacaklar:

- Kök ve `src-tauri` içindeki yanlışlıkla oluşmuş boş dosyaları temizle: `0`, `0.0`, `0.90`, `1.0`, `` ` ``, `fn())`, `EnemyComposition`, `{,+`, `s.champion_id)`.
- `.env` dosyasının git'e girmediğini doğrula; `.env.example` güncel kalsın.
- README'yi template olmaktan çıkar: ürün amacı, kurulum, env, test, dev, build.
- Scriptleri netleştir: `typecheck`, `build`, `tauri dev`, `cargo test`, `cargo fmt`, `cargo clippy`.
- `cargo test` warning listesini tasklara böl.
- `docs/sprint-plan-v2.md` eski kaldığı için "archived" notu ekle veya yeni plana yönlendir.

Kabul kriterleri:

- `pnpm typecheck` geçer.
- `cargo test` geçer.
- Repo kökü sadece anlamlı dosyalar içerir.
- Yeni geliştirici README ile projeyi çalıştırabilir.

### Faz 1 - Mimari Sertleştirme

Süre: 2-3 gün  
Sahipler: `lol-architect`, `lol-rust`

Hedef: Backend tarafını üretim davranışına yaklaştırmak.

Yapılacaklar:

- `AppError` gerçekten komutlarda kullanılsın; `Result<T, String>` kademeli olarak typed error modeline taşınsın.
- SQLite erişimi gözden geçirilsin: uzun async işlemler DB lock tutmamalı. Gerekirse `spawn_blocking` veya küçük connection pool stratejisi değerlendirilsin.
- Startup hatalarında `expect` yerine kullanıcıya anlamlı hata yüzeyi tasarlansın.
- Tauri command kontratları dokümante edilsin: input, output, hata, loading state.
- Kullanılmayan modüller ayrıştırılsın: `websocket.rs`, `builds_repo.rs`, `lolalytics.rs` ya aktif akışa bağlansın ya da backlog'a alınsın.
- Rust warning sayısı sıfıra yaklaştırılsın.

Kabul kriterleri:

- `cargo test` temiz geçer.
- `cargo clippy -- -D warnings` hedeflenir.
- Command hataları frontend'de anlamlı mesajlara dönüştürülebilir.
- Büyük DB işlemleri UI'ı bloklamaz.

### Faz 2 - Ana Ürün Döngüsü

Süre: 3-5 gün  
Sahipler: `lol-rust`, `lol-frontend`, `lol-ux-content`

Hedef: LCU bağlantısından champ-select önerisine kadar ana deneyimi kusursuzlaştırmak.

Yapılacaklar:

- LCU bağlantı durumları netleşsin: client kapalı, login bekleniyor, lobby, champ-select, in-game, error.
- Poller/WebSocket stratejisi kararlaştırılsın. Polling kalacaksa interval, retry ve backoff açık yazılsın; WebSocket'e geçilecekse eski poller geçici fallback olsun.
- `hover_champion` akışı UI'da güvenli hale gelsin: buton, disabled state, hata toast'u, aktif pick action yoksa açıklama.
- Champ-select ekranı hızlı taranabilir hale gelsin: en iyi seçim, alternatifler, rakip tehdidi, takım eksiği.
- Ban fazı boş kalmasın; en azından "en yüksek tehdit" mantığı ile ban önerisi gelsin.
- "Veri eksik" durumları ürün diliyle gösterilsin: yeni oyuncu, RIOT_API_KEY yok, match sync yapılmadı, CDragon cache boş.

Kabul kriterleri:

- Kullanıcı LoL client açıkken uygulamada hangi durumda olduğunu anlar.
- Pick fazında ilk öneri 500ms hedefiyle görünür.
- Veri yokken UI boş veya teknik hata gibi görünmez.
- Hover/pick yardımcı aksiyonu başarısız olursa kullanıcı ne olduğunu anlar.

### Faz 3 - Öneri Motoru ve Veri Kalitesi

Süre: 4-6 gün  
Sahipler: `lol-data`, `lol-rust`, `lol-perf`

Hedef: Önerilerin güvenilir, açıklanabilir ve test edilebilir olması.

Yapılacaklar:

- Scoring ağırlıkları konfigüre edilebilir hale gelsin: comfort, lane counter, team counter, synergy, meta.
- Recommendation snapshot testleri eklensin: aynı session + fixture DB -> aynı top 5.
- Düşük güven durumları gösterilsin: az maç, yeni patch, meta verisi yok, rol bilinmiyor.
- Matchup/build veri kaynağı stratejisi netleştirilsin. Veri kaynağı scraping ise güncel kullanım şartları ve oran limitleri `lol-security-release` tarafından kontrol edilsin.
- Core item ve rune önerileri gerçek build verisine bağlansın; yoksa fallback mantığı açık olsun.
- Rakip kompozisyon analizi oyuncu diline çevrilsin: "AP ağırlıklı", "frontline yok", "CC düşük", "assassin tehdidi".

Kabul kriterleri:

- Her önerinin kısa nedeni vardır.
- Snapshot testleri recommendation regression yakalar.
- Build/rune önerisi veri yokken yanlış kesinlik hissi vermez.
- Perf raporu `get_recommendations` DB + compute süresini ölçer.

### Faz 4 - UI/UX ve İçerik Üst Seviye Polish

Süre: 4-5 gün  
Sahipler: `lol-frontend`, `lol-ux-content`, `lol-qa`

Hedef: Uygulama araç gibi hızlı, oyun içi yardımcı gibi odaklı ve premium hissiyatlı olsun.

Yapılacaklar:

- Design system netleşsin: spacing, radius, color tokens, typography, focus ring, icon button standardı.
- Metinler profesyonelleşsin: "Maç geçmişini yükle -> Öneri gelecek" yerine bağlama göre net, kısa Türkçe mikro metin.
- Text icon yerine gerçek ikon kütüphanesi değerlendirilsin. Settings için `⚙` tek başına kalmasın; tooltip ve erişilebilir label olsun.
- Champion card hiyerarşisi: isim, rol, skor, güven seviyesi, neden, aksiyon.
- Empty/loading/error/success state'leri tasarım sistemiyle tutarlı olsun.
- Görsel QA: desktop ve dar pencere screenshot kontrolleri. Text overflow, overlap ve renk kontrastı kontrol edilsin.
- Onboarding akışı gerçekten değer katsın: API key, LCU bağlantı beklentisi, veri sync, gizlilik açıklaması.

Kabul kriterleri:

- İlk ekran "ürün" hissi verir; template hissi kalmaz.
- Champ-select ekranında 3 saniyede ana karar okunur.
- Dar pencere ve standart desktop boyutunda metin taşması yoktur.
- UI teknik hata diliyle konuşmaz.

### Faz 5 - Test, Release ve Operasyon

Süre: 3-5 gün  
Sahipler: `lol-qa`, `lol-security-release`, `lol-perf`

Hedef: Geliştirme hızı bozulmadan release kalitesi gelsin.

Yapılacaklar:

- Frontend test altyapısı: Vitest + React Testing Library.
- E2E/görsel test: Playwright veya Tauri uyumlu smoke flow.
- Mock LCU/Riot fixture server: bağlantı, lobby, champ-select, in-game senaryoları.
- CI yerel komut standardı: `pnpm typecheck`, `pnpm build`, `cargo test`, `cargo fmt --check`, `cargo clippy`.
- Secrets audit: `.env`, logs, crash report, Riot key.
- Tauri build/release: updater, signing, versioning, changelog.
- Crash/error telemetry seçeneği: kullanıcı rızası ve gizlilik notuyla.

Kabul kriterleri:

- Release öncesi tek checklist ile kalite doğrulanır.
- Temel kullanıcı akışı mock ortamda otomatik test edilir.
- Paket build alınır.
- Gizli anahtar veya kişisel veri loglanmaz.

## 5. Sprint Sıralaması

| Sprint | Süre | Fokus | Aktif yan agentlar |
| --- | --- | --- | --- |
| Sprint A | 1 gün | Repo hijyeni + README + kalite komutları | lead, qa |
| Sprint B | 2-3 gün | Rust mimari sertleştirme + warning azaltma | architect, rust, qa |
| Sprint C | 3-5 gün | Ana champ-select ürün döngüsü | rust, frontend, ux-content |
| Sprint D | 4-6 gün | Recommendation/data kalitesi | data, rust, perf, security-release |
| Sprint E | 4-5 gün | UI/UX polish + visual QA | frontend, ux-content, qa |
| Sprint F | 3-5 gün | Release, updater, packaging, security | security-release, qa, perf |

## 6. Lead İçin Günlük Kontrol Listesi

Her gün başında:

- Bugünün tek ürün hedefi ne?
- Hangi agent hangi dosya alanına sahip?
- Çakışan write scope var mı?
- Dün kalan risk bugün kapatılıyor mu?
- Bugünün kalite komutu ne?

Her merge öncesi:

- `pnpm typecheck`
- `cargo test`
- İlgili UI değiştiyse screenshot kontrolü
- Handoff notu
- Kullanıcı akışında boş/hatalı state kontrolü

Her sprint sonunda:

- Çalışan demo
- Test çıktısı
- Known issues
- Bir sonraki sprintin ilk üç işi
- Teknik borç listesinde eklenen/azalan maddeler

## 7. İlk Başlatılacak 5 Issue

1. `repo-hygiene-readme`
   - Boş dosya temizliği, README yenileme, eski planı v3'e yönlendirme.

2. `rust-warning-zero`
   - Kullanılmayan import/dead code kararları, `cargo clippy` hedefi.

3. `command-error-contracts`
   - En sık kullanılan Tauri command'larda typed error ve frontend error mapping.

4. `champ-select-decision-ui`
   - Hero recommendation, quick picks, confidence, reason ve hover aksiyonunun UI polish'i.

5. `recommendation-snapshot-tests`
   - Fixture session + fixture DB ile deterministic top 5 testleri.

## 8. Definition of Senior Done

Bir iş ancak şu koşullarda tamam sayılır:

- Kullanıcı değeri açık.
- Kod modül sınırlarına uyuyor.
- Hata durumu tasarlanmış.
- Loading/empty state düşünülmüş.
- Test veya bilinçli test notu var.
- Performans riski varsa ölçülmüş.
- UI değiştiyse görsel kontrol yapılmış.
- Handoff notu yazılmış.

Bu standardı her agent kullanır; `lol-lead` de bu standardın bekçisidir.
