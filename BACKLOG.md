# BACKLOG

> Skor: `değer = etki + borç + test-edilebilirlik + hedef-uyum − risk − efor`.
> Durumlar: `todo · doing · done · blocked · wontfix`. Tarih: 2026-06-16.
> Aşağıdaki B-09+ işleri `csa-backlog-discovery` workflow'unun (36 ajan, 30 aday →
> **21 doğrulanmış bulgu**, adversaryal koddan-teyit) çıktısıdır.

## Aktif
(boş — sıradaki iterasyonda skorla seçilecek: B-10 / B-12 / B-14)

## Açık — yüksek/orta değer (koddan teyitli)
| id | sev | dosya | özet | durum |
|---|---|---|---|---|
| ~~B-14~~ | **high** | worker `ingest.ts` | patch leksik-sort → `ORDER BY updated_at DESC` (recency). Worker 16 test. **deploy bekliyor** | **done (deploy bekliyor)** |
| **B-03** | med | worker `ingest.ts` + `sources.ts:570` | Worker okuma uçları yaş sinyali içermez + dev-key 24h expiry sessiz + desktop edge patch'i körlemesine kabul → stale "taze" sunulur (cluster: #8/#9/#11) | todo |
| **B-10** | med | `DataStatusBadges.tsx:155` | noMeta `meta_score==0.3` sihirli-sabitiyle tespit ediliyor; core'un yapısal `missing_signals` ('meta') alanı zaten var → kırılgan kuplajı kaldır | todo |
| ~~B-11~~ | med | `useChampSelect.ts` | puuid çözülünce aktif session için recs yeni puuid'le refetch edilir (ayrı effect); boş-puuid stale öneriler kalmıyor | **done** |
| ~~B-12~~ | med | `riot/client.ts` | routingForRegion **br1→americas** eklendi (BR account-v1+match-v5 doğru host). desktop test | **done (br1)** |
| **B-12b** | low | `riot/client.ts` + `getByRiotId`/`match-v5.ts` | OC1 routing: account-v1 `americas` ister, match-v5 `sea` ister (paylaşılan fonksiyon çakışıyor) → routing'i API'ye göre ayır (account vs match) ki OCE match-v5 `sea`'ya gitsin | todo |
| ~~B-13~~ | med | `sources.ts` | u.gg fallback satırları `uggPatch` (gerçek kaynak patch) ile etiketleniyor → staleness maskesi kalktı. desktop 16 sources test | **done** |
| ~~B-15~~ | med | `OnboardingWizard.tsx` | onboarding LCU sync'inden ÖNCE `sync_ddragon_champions` çağırır → şampiyon tablosu gerçek anahtarla dolar, placeholder numeric key (ikon 404) yazılmaz | **done** |

## Açık — düşük değer / cila
| id | sev | dosya | özet | durum |
|---|---|---|---|---|
| ~~B-16~~ | low | `PoolBuilder.tsx` | loading vs empty (loading state + `poolBuilder.loading`) | **done** |
| **B-17** | low | `RankCard/TrendPanel/WeeklySummaryCard` | puuid null → kalıcı null render, retry yok (PoolBuilder retry desenine bağla) | todo |
| ~~B-18~~ | low | `StatsView.tsx` | WR grafiği <3 maçlık havuzda ince-veri notu gösterir (notsuz gizleme yok) | **done** |
| **B-19** | low | `src/lib/ddragon.ts` | renderer DDragon patch'i yalnız LobbyView sync'iyle set; onboarding/direkt-champ-select yolunda set edilmez → App mount'ta global `get_ddragon_version` | todo |
| ~~B-20~~ | low | `outcomes.ts` | `pickRecorded=true` try içine alındı → DB hatasında sonraki IN_GAME event'i retry eder, eğitim etiketi kaybolmaz. retry testi | **done** |
| ~~B-21~~ | low | `lcu/websocket.ts` | reconnect catch hata sebebini loglar (`catch (err)`+warn) → cert/pin/upgrade hataları sessiz değil. davranışsal test | **done** |
| ~~B-22~~ | low | `useChampSelect.ts` + `RoleSelector.tsx` | roleSource kalıcı tercih → 'preferred' (nötr "Geçen oyundan hatırlandı" hint), yanlış "Rolü sen seçtin" kalktı | **done** |
| **B-23** | low | `DataStatusBadges.test.tsx` | noRiotKey + liveDataAge dürüst chip'leri test'siz (en sık canlı durum: prod-key yok) | todo |
| **B-24** | low | `recommendations.test.ts` / `engine.rs` | cold-start recs e2e + orWarnDefault hata-yolu + engine 0.3 nötr fallback e2e test boşlukları (#17/#18/#19) | todo |
| ~~B-06~~ | low | `.claude/CLAUDE.md` | Tauri→Electron güncellendi (stack/komut/klasör/kurallar; PROJECT_STATE/AGENTS/QUALITY_CHECKS'e işaret) | **done** |
| **B-08** | low | `useSummonerData.ts:83` | fire-and-forget sync yarışları (B-11/B-19 ile örtüşüyor) | todo |
| **B-02** | med | `scheduler.ts`/`index.ts` | cold-start priming (seed import + ilk edge fetch boot'ta) | todo |

## Discovery-2 batch (a11y/concurrency/arch — 11 doğrulanmış; verify kısmen session-limit'e takıldı)
| id | sev | dosya | özet | durum |
|---|---|---|---|---|
| ~~B-26~~ | med | `useChampSelect.ts` | fetchRecommendations seq-guard'lı: out-of-order yanıt eskiyi ezmiyor + session-null sonrası bayat recs yazılmıyor. test | **done** |
| ~~B-25~~ | med | `Toast.tsx` | toast'lar SR'a duyuruluyor: error/warning `role=alert`/assertive, info/success `role=status`/polite. test | **done** |
| **B-27** | low | `PoolBuilder.tsx:93` | bozuk ARIA tablist (role=tablist ama tab/aria-selected yok) → `role=group`+`aria-pressed` | todo |
| **B-28** | low | `SettingsPanel.tsx`+`ChampionDetailCard.tsx` | dialog'da `aria-labelledby` + focus trap/restore yok | todo |
| ~~B-29~~ | low | `Timer.tsx` | SVG geri sayım `role=img`+`aria-label` ("{{n}} saniye kaldı") → SR'a duyurulur. test | **done** |
| **B-30** | low | `SettingsPanel.tsx` | bölge/pencere-boyutu `<select>` etiketsiz → `aria-label` | todo |
| ~~B-31~~ | low | `ConnectionBadge.tsx` | tek persistent `role=status`+`aria-live` span → bağlantı durum değişimleri SR'a duyurulur. test | **done** |
| **B-32** | low | `data-pipeline.ts` | `syncDataPipelineInner` 220-satır god-function, 5 kopya source-step bloğu → refactor | todo |
| **B-33** | low | `useChampSelect.ts` | 7 kopya fetch-on-signature useEffect → ortak helper | todo |
| ~~B-34~~ | low | `riot/client.ts` | puuid/matchId path segment'leri `encodeURIComponent`'li (4 URL builder) — path/query injection defensive. test | **done** |
| ~~B-35~~ | low | `useToast.ts` | auto-dismiss timer'ları ref'te tutulup unmount'ta temizlenir (sızıntı + unmounted state-update giderildi). test | **done** |

## Tamamlanan / Kapatılan
- **B-01** (done) — Image fallback: `BanIcon` + `CounterItemIcon` onError. 214 test.
- **B-05** (done) — `BuildSummary` 'none' → dürüst "build verisi yok" (+tr/en+test). 217 test.
- **B-09** (done) — DataStatusBadges cap önceliği: aksiyon-alınabilir chip'ler (meta/mastery/Riot-key/stale) cap'ten önce öne alındı (stable sort), diagnostiklerce atılmıyor. Eviction testi eklendi. 218 test.
- **B-16** (done) — `PoolBuilder` loading state: boş puuid/settling sırasında "öneri yok" yerine "Öneriler yükleniyor…" (+tr/en `poolBuilder.loading` + 2 test). 220 test.
- **B-14** (done — deploy bekliyor) — worker `readRates/readMatchups/readBuilds` patch çözümü leksik `patch DESC` → `updated_at DESC` (recency); "16.9">"16.10" bayatlık giderildi. Davranışsal regresyon testi (mock D1 ORDER BY'a saygı duyar). Worker 16 test. **NOT: prod'da etkili olması için `wrangler deploy` gerekir.**
- **B-13** (done) — `syncUgg` u.gg fallback satırlarını canlı patch yerine gerçek kaynak patch'iyle (`uggPatch.replace('_','.')`) etiketler → 1-2 patch eski u.gg verisi 'güncel' sanılıp `patch_fresh`'i yanlış true yapmıyor. Back-level davranışsal test (16.11 canlı → 16_9 servis → satır "16.9"). desktop 16 sources test.
- **B-12** (done — br1) — `routingForRegion`'a `br1→americas` eklendi (account-v1 + Match-V5 doğru bölgesel host; eskiden 'europe' default'una düşüp BR maç/öneri verisini sessizce 404'lüyordu). Test eklendi. oc1 → B-12b (routing-split gerekir).
- **B-18** (done) — `StatsView` WR bölümü, havuzdaki tüm şampiyonlar <3 maçsa sessizce gizlenmek yerine "≥3 maçlık şampiyon yok" ince-veri notu gösterir (+tr/en `stats.winRateThin` + 2 test). renderer 222 test.
- **B-20** (done) — `OutcomeTracker.onGameflowPhase`: `pickRecorded=true` koşulsuz set ediliyordu; pick-record INSERT throw ederse o maçın öneri→pick eğitim etiketi kalıcı kaybolurdu. Flag artık yalnız başarılı INSERT sonrası set edilir → sonraki IN_GAME event'i retry eder. Davranışsal retry testi (throwing-db → boş; gerçek db → kayıt). desktop 15 outcomes test.
- **B-22** (done, 6c5024c) — `roleSource` kalıcı tercihten gelen rolü 'manual' yerine yeni 'preferred' provenance'ıyla etiketler; `RoleSelector` "Rolü sen seçtin" yerine nötr "Geçen oyundan hatırlandı" gösterir (+tr/en `rolePreferredHint` + test assertion). renderer 222 test.
- **B-11** (done) — `useChampSelect`: puuid mount'ta asenkron çözüldüğünden ilk öneriler boş-puuid (kişiselleştirmesiz) hesaplanıyordu; ayrı bir effect puuid çözülünce aktif session için recs'i yeni puuid'le yeniden çeker. Davranışsal test (puuid '' → 'puuid-9' rerender → refetch). renderer 223 test.
- **B-15** (done) — Onboarding `handleDone`, LCU mastery/maç sync'inden ÖNCE `sync_ddragon_champions` (best-effort) çağırır → şampiyon tablosu gerçek anahtarlarla dolar, kullanıcının kendi şampiyonları için numeric placeholder key (ikon 404→"26" baş-harf) yazılmaz. Sıra testi (ddragon < lcu). renderer 222 test. (B-04 reopen kapandı)
- **B-07** (done — renderer) — cold-start dürüst-UI `DataStatusBadges.test.tsx`'te kapsanmış; host-tarafı e2e boşluğu B-24'e taşındı.
- ~~**B-04** wontfix~~ → **B-15 reopen**: workflow placeholder-key'in kullanıcının kendi mastery ikonlarını bozduğunu doğruladı (kendi-iyileşse de pencere gerçek).
