# Canlı Duman Checklist'i (Faz G4)

> Her release ÖNCESİ elle koşulur. Otomasyon bu yolları kapsamıyor (canlı LCU +
> canlı oyun gerekir) — bu bilinçli bir kabul; checklist o boşluğun kalkanı.
> Süre: ~20 dk (1 normal/draft maçı dahil).

## Hazırlık
- [ ] Paketli build kurulu (dev değil) — updater yolu gerçek release'i görür
- [ ] League Client AÇIK, hesapta en az 5 maçlık geçmiş var

## 1. Bağlantı + Boot
- [ ] Uygulama açılışta "DB hazır" + LCU "bağlandı" durumuna geçiyor
- [ ] `%APPDATA%/csa-desktop/` altında `.corrupt-*` dosyası YOK (varsa önceki
      bozulma demektir — incele)
- [ ] İlk açılışta seans koçu ısınma checklist'i lobide görünüyor

## 2. Veri Senkronu
- [ ] "Maç geçmişini yükle" → maçlar + mastery doluyor (toast başarılı)
- [ ] İstatistik sekmesi: karne kartı + trend panosu + haftalık özet görünüyor
- [ ] DataStatusBadges dürüst (meta yaşı, eksik kaynaklar)

## 3. Champion Select (1 draft girişi)
- [ ] Ban fazı: ban önerileri + rakip hover chip'leri
- [ ] Pick fazı: 5 öneri, skor kırılımı TAM liste, build (seed/u.gg rune sayfası
      + skill order), takım sohbeti yardımcısı kopyalıyor
- [ ] Hover butonu client'ta şampiyonu hover'lıyor (kilitleMİyor!)
- [ ] 1-5 tuşları kartlar arasında geziyor
- [ ] Veto'lu şampiyon listede ÇIKMIYOR (detay kartından birini işaretleyip dene)

## 4. Oyun İçi (maçın ilk ~12 dakikası yeter)
- [ ] Overlay açılıyor; oyun planı kartı (win condition + dalga satırı) dolu
- [ ] 1:30-3:30 arası "Lvl 2-3 penceresi" hatırlatıcısı düştü
- [ ] Dragon/grubs timer'ları doğru sayıyor; ses AÇIKSA 60/30 sn bip'leri geliyor
- [ ] Overlay'i kapatıp açmak (tray) sorunsuz

## 5. Maç Sonu
- [ ] Lobiye dönüşte yeni karne üretildi (maç sonucu + hedef kontrolü)
- [ ] Maç notu yazılıp kaydoluyor
- [ ] 2+ kayıp serisindeyse tilt kartı göründü (yoksa bölümü atla)

## 6. Updater (yalnız yeni release yayınlandıysa)
- [ ] Eski sürüm açılınca yeni sürümü indirip kapanışta kuruyor
- [ ] Kurulum sonrası sürüm numarası doğru

## Kayıt
Tarih / sürüm / bulgular buraya işlenir:

| Tarih | Sürüm | Sonuç | Not |
|-------|-------|-------|-----|
|       |       |       |     |
