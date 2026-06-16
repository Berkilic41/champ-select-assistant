# BACKLOG

> Skor: `değer = etki + borç + test-edilebilirlik + hedef-uyum − risk − efor`.
> Durumlar: `todo · doing · done · blocked · wontfix`. Tarih: 2026-06-16.
> Aşağıdaki B-09+ işleri `csa-backlog-discovery` workflow'unun (36 ajan, 30 aday →
> **21 doğrulanmış bulgu**, adversaryal koddan-teyit) çıktısıdır.

## Aktif
(boş — Discovery-3 tükendi: B-38/B-39/B-40 done; yalnız B-24 ertelenmiş. Sıradaki tur yeni keşif tarar.)

## Açık — yüksek/orta değer (koddan teyitli)
| id | sev | dosya | özet | durum |
|---|---|---|---|---|
| ~~B-37~~ | low | worker `index.ts` | (loop-keşif) cron `scheduled` yolu `runIngestion` reddini bağlamlı loglamıyordu (HTTP yolu logluyor) → `.catch(console.error("scheduled ingest failed", e))`. Cron production birincil sürücü; sessiz hata `wrangler tail`'de görünür. +regresyon-kilidi testi (reddi yutar). worker 17 test | **done** |
| ~~B-36~~ | low | worker `ingest.ts` | (loop-keşif) readRates/readMatchups/readBuilds 3× birebir aynı patch-çözümleme bloğu (B-14 yorumu dâhil) → `resolveLatestPatch` helper'ı. Bakım tuzağı (1 yer); davranış (boş-string dâhil `patch ?? …`) korundu. worker 16 test + typecheck | **done** |
| ~~B-14~~ | **high** | worker `ingest.ts` | patch leksik-sort → `ORDER BY updated_at DESC` (recency). Worker 16 test. **deploy bekliyor** | **done (deploy bekliyor)** |
| ~~B-03~~ | med | worker `ingest.ts` + `sources.ts` | freshness sinyali UÇTAN UCA: worker readRates `updated_at` döndürür; desktop syncEdgeRates >48s bayatsa confidence'ı 'low'a düşürür → mevcut data-quality/öneri akışı dürüstçe yansıtır. worker+desktop test. **(prod: worker deploy ister)** | **done** |
| ~~B-10~~ | med | `DataStatusBadges.tsx` | noMeta artık yapısal `missing_signals` ('meta') alanını kullanır (core json_api::compute_missing_signals); `meta_score==0.3` sihirli-sabiti kalktı → ~%50.1 WR yanlış-pozitifi de giderildi. renderer 15 test | **done** |
| ~~B-11~~ | med | `useChampSelect.ts` | puuid çözülünce aktif session için recs yeni puuid'le refetch edilir (ayrı effect); boş-puuid stale öneriler kalmıyor | **done** |
| ~~B-12~~ | med | `riot/client.ts` | routingForRegion **br1→americas** eklendi (BR account-v1+match-v5 doğru host). desktop test | **done (br1)** |
| ~~B-12b~~ | low | `riot/client.ts` | `oc1→americas` (account-v1 OCE doğru host; eskiden 'europe' default → 404). test. | **done** |
| ~~B-12c~~ | low | `match-v5.ts` + `riot-sync.ts` + `client.ts` | `matchRoutingForRegion` (oc1→`sea`) eklendi; iki match-v5 caller'ı ona geçti, account-v1 `routingForRegion`→americas'ta kaldı → OCE maç-çekimi 404 yemez. desktop test | **done** |
| ~~B-13~~ | med | `sources.ts` | u.gg fallback satırları `uggPatch` (gerçek kaynak patch) ile etiketleniyor → staleness maskesi kalktı. desktop 16 sources test | **done** |
| ~~B-15~~ | med | `OnboardingWizard.tsx` | onboarding LCU sync'inden ÖNCE `sync_ddragon_champions` çağırır → şampiyon tablosu gerçek anahtarla dolar, placeholder numeric key (ikon 404) yazılmaz | **done** |

## Açık — düşük değer / cila
| id | sev | dosya | özet | durum |
|---|---|---|---|---|
| ~~B-16~~ | low | `PoolBuilder.tsx` | loading vs empty (loading state + `poolBuilder.loading`) | **done** |
| ~~B-17~~ | low | `RankCard/TrendPanel/WeeklySummaryCard` | ortak `useActiveSummonerPuuid` hook'u (retry'lı); puuid çözülünce kartlar verilerini yeniden çeker (kalıcı-null giderildi + DRY). test | **done** |
| ~~B-18~~ | low | `StatsView.tsx` | WR grafiği <3 maçlık havuzda ince-veri notu gösterir (notsuz gizleme yok) | **done** |
| ~~B-19~~ | low | `src/lib/ddragon.ts` + `App.tsx` | App mount'ta global `get_ddragon_version`→`applyDdragonVersion` (sentinel-guard'lı); tüm giriş yolları canlı patch alır. test | **done** |
| ~~B-20~~ | low | `outcomes.ts` | `pickRecorded=true` try içine alındı → DB hatasında sonraki IN_GAME event'i retry eder, eğitim etiketi kaybolmaz. retry testi | **done** |
| ~~B-21~~ | low | `lcu/websocket.ts` | reconnect catch hata sebebini loglar (`catch (err)`+warn) → cert/pin/upgrade hataları sessiz değil. davranışsal test | **done** |
| ~~B-22~~ | low | `useChampSelect.ts` + `RoleSelector.tsx` | roleSource kalıcı tercih → 'preferred' (nötr "Geçen oyundan hatırlandı" hint), yanlış "Rolü sen seçtin" kalktı | **done** |
| ~~B-23~~ | low | `DataStatusBadges.test.tsx` | noRiotKey + liveDataAge (bayat>24s) + taze=chip-yok testleri eklendi (baseTrajectory fixture). renderer 18 test | **done** |
| **B-24** | low | `recommendations.test.ts` / `engine.rs` | cold-start recs e2e — **KISMEN ÇÖZÜLDÜ + ERTELENDİ**: ✅ "noMastery chip ölü mü?" doğrulandı — DEĞİL. Aday havuzu mastery-TABANLI değil, TÜM şampiyonlar+rol-filtre (engine.rs:23,73); stretch gate `comfort<0.10` eler ama güçlü-kombo'lu (cb≥0.80) 1 stretch geçer → mastery yokken kombo'lu tek rec (comfort 0) noMastery'i tetikler (dar ama ulaşılabilir). DataStatusBadges noMastery testi (`rec(_,0)`) zaten chip'i kapsıyor. ⏸ KALAN (ertelendi): motor e2e — mastery'siz+kombo'lu session fixture'ı kurup engine'in böyle bir liste ürettiğini kilitlemek (yüksek-efor fixture) + orWarnDefault/engine-0.3 | todo |
| ~~B-06~~ | low | `.claude/CLAUDE.md` | Tauri→Electron güncellendi (stack/komut/klasör/kurallar; PROJECT_STATE/AGENTS/QUALITY_CHECKS'e işaret) | **done** |
| ~~B-08~~ | low | `useSummonerData.ts:83` | **wontfix (kapsanmış)**: cold-DB boş-champMap riski dar; görünür semptom (bozuk ikon) **B-01 onError fallback'iyle** çözülü, onboarding yolu **B-15** ddragon-önce-sync. Guard warm-path'i yavaşlatır + re-render riski → değer/risk düşük | **wontfix** |
| ~~B-02~~ | med | `scheduler.ts` + `data-pipeline.ts` | cold-start priming: `primeColdStartSeeds` (atomik, boş-tablo guard) DDragon source'undan HEMEN sonra (FK-valid champions) bundled offline seed'leri içe aktarır → otomatik yol artık manuel Settings sync'i beklemeden offline build/matchup kapsaması verir. **Not:** boot'ta DEĞİL (FK ON + champions boş → silent-fail); ilk edge fetch zaten 30s scheduler tick'inde. best-effort+atomik. desktop 155 test | **done** |

## Discovery-3 batch (loop, 2026-06-17 — `csa-loop-discovery-3`: 20 ajan, 15 aday → 4 doğrulanmış)
| id | sev | dosya | özet | durum |
|---|---|---|---|---|
| ~~B-38~~ | **high** | `engine.rs:110` | stretch-pick risk notu `losses = games - wins` korumasız u32 çıkarma; host SQLite `wins<=games` zorlamıyor (`SUM(win)`, CHECK yok) → bozuk satır `wins>games` → release/WASM `overflow-checks` kapalı, sessiz underflow "4294967290L" çöp not (debug panik). Not-üretimi saf `stretch_risk_note`'a çıkarıldı + `saturating_sub` (crate konvansiyonu) + 3 birim testi. core 569 test + clippy | **done** |
| ~~B-39~~ | med | `json_api.rs:332-337` | `my_pos()` Arena (queue 1700) brawl'ı ele almıyordu → `else` dalında `assigned_position` döner; renderer `applyRole` kalıcı tercih-rolünü (örn. "middle") queue-koşulsuz enjekte edince Arena session'a SR-rol sızar → satır 497 yanlış "lane_performance eksik" rozeti basar (Arena'da lane yok). Fix: `matches!(queue_id, 450\|1700)` (engine.rs `is_aram` ile hizalı). Regresyon testi (queue 1700 fixture → sinyal yok). core 570 test + clippy | **done** |
| ~~B-40~~ | med | `docs/api-key-policy.md` | stale Tauri referansları: `src-tauri/.env`, `dotenvy::dotenv()`, `tauri.conf.json` checklist, `target/release/*.exe` tarama — Electron+Node'a göçtü; gerçek mekanizma `desktop/src/main/riot/client.ts` `process.env.RIOT_API_KEY` (+ yakın `.env`). Dev-bölümü+checklist+LCU-note (`champ_select.rs`→`commands/lcu.ts`) güncellendi. Saf-doküman | **done** |

## Discovery-2 batch (a11y/concurrency/arch — 11 doğrulanmış; verify kısmen session-limit'e takıldı)
| id | sev | dosya | özet | durum |
|---|---|---|---|---|
| ~~B-26~~ | med | `useChampSelect.ts` | fetchRecommendations seq-guard'lı: out-of-order yanıt eskiyi ezmiyor + session-null sonrası bayat recs yazılmıyor. test | **done** |
| ~~B-25~~ | med | `Toast.tsx` | toast'lar SR'a duyuruluyor: error/warning `role=alert`/assertive, info/success `role=status`/polite. test | **done** |
| ~~B-27~~ | low | `PoolBuilder.tsx` | bozuk `role=tablist` → etiketli `role=group`+`aria-pressed` toggle grubu. test | **done** |
| ~~B-28~~ | low | `SettingsPanel.tsx`+`ChampionDetailCard.tsx` | **a)** dialog `aria-labelledby`→başlık ✅ **b)** `useModalFocus`: açılışta ilk-odak + kapanışta restore ✅. test. (Tab döngü-kapanı → B-28c nice-to-have) | **done** |
| ~~B-29~~ | low | `Timer.tsx` | SVG geri sayım `role=img`+`aria-label` ("{{n}} saniye kaldı") → SR'a duyurulur. test | **done** |
| ~~B-30~~ | low | `SettingsPanel.tsx` | bölge + pencere-boyutu `<select>` `aria-label`'lı (SR-etiketli). test | **done** |
| ~~B-31~~ | low | `ConnectionBadge.tsx` | tek persistent `role=status`+`aria-live` span → bağlantı durum değişimleri SR'a duyurulur. test | **done** |
| ~~B-32~~ | low | `data-pipeline.ts` | `syncDataPipelineInner` god-function (~140 satır) → `runSource<T>` helper (5 kaynak bloğu DRY). "tekdüze değil" endişesi çözüldü: kaynağa-özgü tek fark `fn`+`message(result)` callback'i; match_v5 çok-alanlı mesaj/default sadece farklı argüman. Mevcut güçlü e2e (summary+errors+log) net olarak korudu; 155 desktop test yeşil | **done** |
| ~~B-33~~ | low | `useChampSelect.ts` | 7 kopya fetch-on-signature useEffect → `useSessionDerived` helper (~140 satır tekrar kalktı). ÖNCE derived-state güvenlik-ağı testleri eklendi (gamePlan fetch+clear, puuid-threading, list-fallback), refactor davranış-koruyarak yapıldı; 243 renderer test yeşil + typecheck temiz | **done** |
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
