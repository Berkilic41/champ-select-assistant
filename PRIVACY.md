# Gizlilik Politikası — Champ Select Assistant

> Yürürlük tarihi: 2026-05-31 · Sürüm: 0.9.0-beta.1

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
- Hiçbir veri üçüncü taraflarla paylaşılmaz, satılmaz veya bir sunucuya yüklenmez.

## Dış Bağlantılar (Yalnızca Genel Statik İçerik)

Uygulama, kişisel veri **içermeyen** genel statik içerik için şu servislere
bağlanır:

- `ddragon.leagueoflegends.com` ve `cdn.communitydragon.org` — şampiyon/item
  görselleri ve statik oyun verisi (Data Dragon / Community Dragon).
- `cdn.merakianalytics.com` — genel meta (win/pick/ban rate) verisi.

Bu isteklerde kişisel tanımlayıcı (PUUID, isim, match ID) gönderilmez.

## Riot Developer API Key

- Uygulama normal kullanım için Riot developer API key **istemez**.
- İsteğe bağlı bir key yalnızca yerel `.env` dosyasında tutulabilir; binary'ye
  gömülmez ve loglanmaz.

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
