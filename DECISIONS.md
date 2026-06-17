# DECISIONS — mimari/teknik karar günlüğü (ADR-lite)

> Format: bağlam → karar → gerekçe → sonuç. En yeni üstte.

## ADR-005 — Lane-matchup faz-avantajı dürüstçe "KB tahmini" etiketlenir (2026-06-17)
- **Bağlam:** `lane_matchup_from_json` phase_advantage'ı YALNIZ arketip `power_curve`'den (`adv()`) hesaplıyor —
  ölçülen matchup verisine hiç bakmıyor — ama panel barları kaynak etiketsiz gösteriyordu → kullanıcı bunları
  ölçülen win-rate sanabilir. `inferred` yalnız rakip KİMLİĞİNİN tahmin olduğunu söyler, avantaj sayılarının değil.
- **Karar:** `LaneMatchup` struct'a `source: String` ekle (şimdilik sabit "kb_estimate"); panelde "KB tahmini"
  rozeti + tooltip göster. Ölçülen veriyi plumb etmek (source="measured") sonraki dilime ertelendi (geniş core değişikliği).
- **Gerekçe:** Dürüstlük DNA'sı (B-03/B-10/B-23 hattı); minimal + güvenli; **scoring/engine DEĞİŞMEZ (engine purity)**.
  Geniş `ctx.matchups` plumbing'i olmadan kullanıcı barların arketip-tahmini olduğunu anlar.
- **Sonuç:** core 570 + renderer 268 + host 161 yeşil; recommendation.ts `source?` (Rust hep emit, TS opsiyonel —
  `inferred?` deseni). WASM rebuild gerekti (host runtime'da alanı emit etsin; core/pkg gitignore'da → commit'lenmez).

## ADR-004 — Match-History Browser Epic: MVP kapsam varsayımları (2026-06-17)
- **Bağlam:** Kullanıcı "büyük geliştirme modu" + öncelik #1 match-history browser; kapsam belirsizse "soru sormadan makul varsay" dedi. Epic MVP'ye bölündü, ilk dikey dilim (liste sekmesi) uygulandı.
- **Karar (varsayımlar):**
  - **A1** Geçmiş aktif summoner puuid'sine kapsamlı (`useActiveSummonerPuuid`); varsayılan limit 20.
  - **A2** Listedeki "review verdict" = dürüst **"İncelendi"** rozeti (karne var mı). Zengin verdict (metric lines/koçluk) Slice-2 detay panelinde — sahte tek-değer aggregate verdict UYDURULMAZ (dürüstlük DNA'sı).
  - **A3** CS/dk = `cs/(süre/60)`; cs/cs_at_10/vision null → "—".
  - **A4** 4. LobbyView sekmesi (yeni App-level status/route YOK).
  - **A5** Tip elle (`src/types/match-history.ts` + eşleşen host şekli); saf host SQL → **core/ts-rs/WASM değişmez**.
  - **A6** Tarih relatif (`time.*` + LobbyView relativeTime deseni; played_at Unix SANİYE → ×1000).
  - **A7** Queue etiketi `review.queue.*` + host `queueGroup` (soloq/flex/aram/normal).
- **Gerekçe:** En az sürtünme + mevcut desen yeniden-kullanımı (`recentMatches` JOIN, RankCard tip deseni, P-07 dürüst loading/error/empty); core'a dokunmadan additive. Yeni Riot çağrısı/cloud yok.
- **Sonuç:** Slice 1 (liste) teslim — renderer 262 + host 160 + typecheck 0. Slice 2 (detay paneli: `GameReviewCard` `matchId`/`review` prop refactor) + Slice 3 (champion/rol/win-loss filtreleri) sıradaki turlar.

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
