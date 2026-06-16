# DECISIONS — mimari/teknik karar günlüğü (ADR-lite)

> Format: bağlam → karar → gerekçe → sonuç. En yeni üstte.

## ADR-003 — Ban ikonu küçük bileşene çıkarılır (2026-06-16)
- **Bağlam:** `ChampSelectScreen.tsx`'te ban `<img>` iki yerde inline; URL 404'te `onError` yedeği yok (icon-bug sınıfı). React'te `.map()` içinde inline `useState` tutulamaz.
- **Karar:** `BanIcon.tsx` adlı küçük presentational bileşen oluştur; her iki ban bloğu onu kullansın.
- **Gerekçe:** onError state'i için bileşen ZORUNLU; ayrıca iki özdeş ternary'yi DRY yapar ve izole test edilebilir kılar. Mevcut `ItemIcon`/`ChampionIcon` desenine uyumlu.
- **Sonuç:** İki ban bloğu tek satıra iner; `BanIcon.test.tsx` ile davranış kilitlenir.

## ADR-002 — Otomatik commit/push YOK (2026-06-16)
- **Bağlam:** Otonom döngü süreklilik istiyor; ama kullanıcının değişmez kuralı "commit/push yalnız açıkça isteyince".
- **Karar:** Döngü hiçbir iterasyonda otomatik commit/push yapmaz; değişiklikleri çalışma ağacında bırakır.
- **Gerekçe:** Kullanıcı diff'leri biriktikçe gözden geçirir; geri-alınabilirlik + güven korunur.
- **Sonuç:** Kalite kapıları yeşil olsa bile commit edilmez; kullanıcı commit'ler.

## ADR-001 — Sürekli otonom geliştirme sistemi kuruldu (2026-06-16)
- **Bağlam:** Kullanıcı tek-seferlik görev yerine kendi backlog'unu üreten, önceliklendiren, uygulayan, test eden ve dokümante eden sürekli bir mühendislik döngüsü istedi.
- **Karar:** Repo kökünde 7 yönetim dosyası (`AGENTS/PROJECT_STATE/BACKLOG/TASKS/DECISIONS/CHANGELOG/QUALITY_CHECKS`) + Inspect→…→Continue döngüsü; kullanıcı rolleri gerçek `Agent` tiplerine eşlendi.
- **Gerekçe:** İzlenebilir + güvenli + tekrarsız iterasyon; mevcut araç envanteriyle çalışır (simülasyon değil).
- **Sonuç:** İlk iş B-01 (image fallback); döngü "dur" denene dek devam eder, riskli/büyük iş onay-kapılı.
