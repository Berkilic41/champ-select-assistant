# Gizlilik Politikası — Champ Select Assistant

> Yürürlük tarihi: 2026-06-14 · Sürüm: 0.10.0-beta.4

Champ Select Assistant ("Uygulama") gizliliğini ciddiye alır. Bu belge,
Uygulamanın hangi verilere eriştiğini, bunları nasıl kullandığını ve neyi
**yapmadığını** açıklar.

## Özet

- **Telemetry yok.** Uygulama hiçbir analitik veya izleme servisine veri göndermez.
- **Veriler yerelde kalır.** Tüm veriler yalnızca senin bilgisayarında saklanır.
- **Hesap yok.** Kayıt, giriş veya bulut senkronizasyonu yoktur.

## Eriştiğimiz Veriler

Uygulama, yalnızca o anda League of Legends'a giriş yapmış **yerel oyuncuya**
ait aşağıdaki verilere erişir:

| Veri | Kaynak | Amaç |
|------|--------|------|
| Maç geçmişi (son maçlar) | League Client (LCU) | Comfort/öneri hesabı |
| Şampiyon mastery | League Client (LCU) | Öneri sıralaması |
| Champ-select oturum durumu | League Client (LCU) | Canlı öneri üretimi |
| Sunucu/dil tercihi, ağırlık ayarları | Senin girdiğin değerler | Uygulama yapılandırması |

Bu veriler **yalnızca yerel SQLite veritabanında** saklanır ve cihazından dışarı
çıkmaz.

## Verileri Nasıl Kullanırız

- Veriler yalnızca champ-select önerileri üretmek için cihazında işlenir.
- Kişisel verin (maç geçmişi, mastery, PUUID, isim) üçüncü taraflarla **paylaşılmaz,
  satılmaz veya bir sunucuya yüklenmez** ve cihazından çıkmaz.
- Cihazından çıkan tek ağ trafiği, kişisel tanımlayıcı **içermeyen** isteklerdir:
  (a) genel statik içerik (DDragon/CDragon), (b) bölge bazlı toplu meta sorgusu
  (yalnızca `region`/`patch`), ve (c) yalnızca sen açıkça yapılandırırsan,
  anonimleştirilmiş öneri geri bildirimi. Detaylar aşağıda.

## Dış Bağlantılar (Yalnızca Genel Statik İçerik)

Uygulama, kişisel veri **içermeyen** genel statik içerik için şu servislere
bağlanır:

- `ddragon.leagueoflegends.com` ve `cdn.communitydragon.org` — şampiyon/item
  görselleri ve statik oyun verisi (Data Dragon / Community Dragon).
- Toplu meta veri servisi (varsayılan: kendi Cloudflare Worker'ımız) — yalnızca
  `region` (+ opsiyonel `patch`) parametresiyle toplu (anonim, oyuncu-bağımsız)
  win/pick/ban oranlarını okur. İstekte PUUID/isim/match ID **yoktur**. Bu adres
  `EDGE_BASE_URL` ile değiştirilebilir veya boşaltılarak kapatılabilir.

Bu isteklerin hiçbirinde kişisel tanımlayıcı (PUUID, isim, match ID) gönderilmez.

## Riot Developer API Key

- Uygulama normal kullanım için Riot developer API key **istemez**.
- İsteğe bağlı bir key yalnızca yerel `.env` dosyasında tutulabilir; binary'ye
  gömülmez ve loglanmaz.

## Opsiyonel: Anonim Öneri Geri Bildirimi

- **Varsayılan olarak kapalıdır.** Yalnızca `DRAFT_BRAIN_API_BASE` ortam
  değişkenini sen açıkça ayarlarsan etkinleşir.
- Etkinse, öneri kalitesini iyileştirmek için **anonimleştirilmiş** geri bildirim
  yüklenir: hangi önerinin kabul/ret edildiği + bir oturum **hash**'i (`user_hash`).
  Ham PUUID, isim veya maç kimliği **gönderilmez**.
- Hash üretilemeyen (≥16 karakter olmayan) kayıtlar tamamen **atlanır**, asla
  gönderilmez. Her gönderim, yinelenmeyi önleyen bir idempotency anahtarı taşır.
- Bu özelliği hiç açmazsan, hiçbir geri bildirim cihazından çıkmaz.

## Saklama ve Silme

- Tüm veriler yerel uygulama veri klasöründeki SQLite dosyasındadır.
- Uygulamayı kaldırmak veya bu dosyayı silmek tüm verilerini kalıcı olarak siler.

## Çocukların Gizliliği

Uygulama, League of Legends'ın yaş şartlarına tabidir ve özellikle çocuklardan
veri toplamayı amaçlamaz.

## Değişiklikler

Bu politika güncellenirse yürürlük tarihi değiştirilir ve CHANGELOG'da belirtilir.

## İletişim

Sorular için: GitHub Issues — https://github.com/Berkilic41/champ-select-assistant/issues

---

Champ Select Assistant isn't endorsed by Riot Games and doesn't reflect the views
or opinions of Riot Games or anyone officially involved in producing or managing
Riot Games properties. Riot Games and all associated properties are trademarks or
registered trademarks of Riot Games, Inc.
