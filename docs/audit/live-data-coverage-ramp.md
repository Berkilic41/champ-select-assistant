# Live Data Coverage Ramp v1 — Audit

> Tarih: 2026-06-07 · Sahip: Claude (2. mühendis) · Kapsam: bir canlı ingestion turunun
> **before → after** tablo sayımlarını yapılandırılmış bir "ramp verdict"e çeviren **saf motor** —
> coverage gerçekten büyüdü mü, discovery → fetch → process funnel'ı ilerliyor mu, nerede takıldı.
> `data_pipeline_quality` (nokta-anı sağlık) ile **tamamlayıcı**: o "durum", bu "hareket" ölçer. Duplicate değil.
> DB/network/Riot/command/UI YOK. Veri uydurma yok. Bu, "Live Data Coverage Ramp v1" bloğunda Claude'un
> **coverage-quality eşik/regresyon guard** omurgası — Codex canlı sayıları buraya akıtır, eşikleri gerçeğe göre sıkarız.
> Tamamlayıcı: [data-pipeline-production-v1.md](data-pipeline-production-v1.md) ·
> [match-discovery-planner-core.md](match-discovery-planner-core.md).

## 1. Modül
`recommendation/coverage_ramp.rs` (saf, `#[allow(dead_code)]` ölçüm komutuna bağlanana dek).
Giriş: `evaluate_coverage_ramp(&CoverageRampInput) -> CoverageRampReport`.

## 2. DTO'lar
**Input (Rust-only — `taken_at` i64 taşır, export edilmez):**
`RampSnapshot` (taken_at/champion_rate_rows/matchup_rows/build_rows/discovered_matches/fetched_matches/
processed_matches/failed_matches/crawled_players) · `CoverageRampInput` (before/after/champ_select_active/crawl_budget).

**Output (ts-rs, bigint yok — delta'lar i32→number, sayımlar u32→number, oranlar f32→number):**
`CoverageDeltas` (champion_rate/matchup/build/discovered/processed/crawled_player delta + elapsed_secs) ·
`DiscoveryFunnel` (pending/fetched/processed/failed + process_ratio + failure_ratio + stalled_stage?) ·
`CoverageRampReport` (ramp_state/deltas/funnel/observations/summary/data_growing).

## 3. Token vocabulary (sabit `pub const`)
- **RAMP_STATES [5]:** `progressing` · `stalled` · `regressed` · `no_activity` · `no_budget`
- **FUNNEL_STAGES [3]:** `discovery` · `fetch` · `process`
- **RAMP_OBSERVATIONS [9]:** `coverage_growing` · `coverage_flat` · `coverage_regressed` ·
  `new_matches_discovered` · `no_new_matches` · `fetch_backlog_growing` · `processing_advancing` ·
  `high_failure_rate` · `champ_select_deferred`

## 4. Karar mantığı
- **Delta'lar:** after − before (i32, negatif olabilir). `elapsed_secs = max(0, after.taken_at − before.taken_at)` (clock-skew clamp).
- **ramp_state önceliği:**
  1. **no_budget:** `champ_select_active ∥ crawl_budget==0` ve ileri hareket yok → ertelendi, ölçüm yapılmadı
     (sahte "flat/no_new" iddiası yok — **champ-select'te network yok** kuralının ölçüm-tarafı).
  2. **regressed:** rate/matchup/build/processed delta < 0 → veri kaybı (aggregation yalnız ileri upsert eder; düşüş = kayıp).
     `discovered_delta < 0` regresyon DEĞİL (backlog tüketimi normal).
  3. **progressing:** rate/matchup/build büyüdü ∥ processed büyüdü.
  4. **stalled:** yeni keşif (known_delta>0) ∥ backlog var (discovered>0) ama hiçbir şey ilerlemedi.
  5. **no_activity:** hiçbir değişiklik yok.
- **Funnel stall localization (`stalled_stage`, yalnız stalled iken):** discovered var + fetched 0 → `fetch`;
  fetched var + processed delta 0 → `process`; aksi → `discovery`. Aksi halde `null`.
- **Funnel oranları:** processed/total_known, failed/total_known (total 0 → 0, div-by-zero yok).
  `total_known = discovered+fetched+processed+failed` (saturating).
- **Observations:** deferred→champ_select_deferred (ve flat/no_new bastırılır); coverage regressed/growing/flat;
  new_matches_discovered ∥ no_new_matches; processing_advancing; fetch_backlog_growing
  (discovered büyüdü + processed sabit); high_failure_rate (failure_ratio ≥ **0.25**). **Sıralı + deduped.**
- **No fabrication:** deferred turdan ramp çıkarmaz; veri yoksa no_activity (sahte ilerleme yok).

## 5. Eşik / regresyon guard yüzeyi (canlı veriyle SIKILACAK)
- `HIGH_FAILURE_RATIO = 0.25` (failed/total). **Codex'in ilk canlı turundan sonra gerçeğe göre ayarlanır.**
- Forward-only invariant: rate/matchup/build/processed asla düşmemeli → düşerse `regressed` (rollback/cache sinyali).
- Backlog büyürken işleme durması → `stalled` + `fetch_backlog_growing` (rate-limit/fetch tıkanması erken uyarı).
- İleride eklenebilir (canlı baseline sonrası): min beklenen processed/saat, max backlog oranı, stale-ramp süresi.

## 6. Test matrisi (16 — hepsi geçti)
progressing (coverage+processing büyür) · backlog tüketimi regresyon DEĞİL (discovered↓ processed↑) ·
processed düşüşü → regressed · matchup düşüşü → regressed · discovered ama fetch yok → stalled@fetch +
fetch_backlog_growing · fetched ama process yok → stalled@process · değişiklik yok → no_activity (no_new+flat) ·
champ-select → no_budget (flat/no_new bastırılır) · crawl_budget 0 → no_budget · high_failure_rate (0.4≥0.25) ·
elapsed clock-skew clamp → 0 · observations sıralı+deduped · **vocab lock** (state∈RAMP_STATES, obs∈RAMP_OBSERVATIONS,
stage∈FUNNEL_STAGES tüm fixture'larda).

## 7. ts-rs + contract
`CoverageDeltas` · `DiscoveryFunnel` · `CoverageRampReport` üretildi (**bigint yok**). TS contract guard:
`src/types/coverage-ramp.contract.test.ts` (3 tip exhaustive `Record<keyof T, true>` + ramp_state/stage/observation
vocab). Baseline: cargo test **551** · gate clippy `-D warnings` exit 0 · fmt-all temiz · typecheck pass · vitest **192/50**.

## 8. i18n (⏳ UI gelince)
`dataPipeline.coverageRamp.{state,funnelStage,observation}.*` i18n henüz YOK → Rust `pub const` vocab-lock guard'ı.
Discovery/scheduler status paneli ramp'i gösterirse Codex tr/en + REQUIRED_KEYS ekler, sonra Claude
`every_emitted_ramp_token_has_an_i18n_label` drift guard'ını bağlar (coverage_expansion deseni). Hazır JSON:
```json
// dataPipeline.coverageRamp
"coverageRamp": {
  "state": { "progressing": "İlerliyor", "stalled": "Takıldı", "regressed": "Geriledi",
             "no_activity": "Değişiklik yok", "no_budget": "Ertelendi" },
  "funnelStage": { "discovery": "Keşif", "fetch": "Çekme", "process": "İşleme" }
}
```

## 9. Codex'e (Live Data Coverage Ramp — ölçüm bağlama)
1. **Before snapshot:** canlı turdan ÖNCE 9 sayımı oku → `RampSnapshot` (champion_rates/champion_matchups/builds satır
   sayıları; match_v5_fetch_history status'lerine göre discovered/fetched|parsed/processed/failed; match_discovery_players sayısı).
2. `sync_data_pipeline` / Match-V5 ingestion'ı **küçük bütçeyle** çalıştır.
3. **After snapshot:** aynı 9 sayımı tekrar oku → `evaluate_coverage_ramp(&{before, after, champ_select_active, crawl_budget})`.
4. `CoverageRampReport` ile raporla: ramp_state + funnel.stalled_stage + observations + delta'lar. Champ-select
   aktifken `no_budget` + `champ_select_deferred` beklenir (network tetiklenmediğinin ölçüm-doğrulaması).
5. Canlı sayılar geldiğinde **Claude**: HIGH_FAILURE_RATIO + min-processed/stale-ramp eşiklerini gerçeğe göre sıkar,
   `evaluate_pipeline_quality`'nin thin/degraded/healthy sınıflandırmasının gerçek veriyle uyumunu audit eder.

> Sıra: Claude saf ramp/funnel/delta + regresyon-eşik motorunu kurdu (fixture-test'li, key/network/DB bağımsız);
> Codex before/after sayımı + küçük-bütçe canlı tur + rapor binding'e bağlar. Bu blok veri büyütmeyi
> **ölçülebilir** yapar: kaç keşfedildi/fetch edildi/işlendi, nerede takıldı, regresyon var mı — hissiyatla değil sayıyla.

## 10. Runtime BAĞLANDI (Codex, 2026-06-07)
`commands/data_quality.rs`: `CoverageRampSnapshotView` + `LiveCoverageRampReport` (ts-rs), DB'den ramp snapshot okuyan helper,
`sync_data_pipeline` gövdesi `sync_data_pipeline_inner`'a ayrıldı, yeni komut **`measure_live_coverage_ramp`** (before snap →
refresh → after snap → `evaluate_coverage_ramp` verdict). lib.rs handler kaydı. Response: `{before, after, refresh, ramp,
champ_select_active, crawl_budget}`. `CoverageRampSnapshotView.taken_at` **number** (bigint yok). Contract test runtime
wrapper'ı kapsayacak şekilde genişletildi (Codex). **Canlı Riot run YAPILMADI** — bu shell'de `RIOT_API_KEY`/`PROXY_URL` yok;
key gelince komut doğrudan canlı before/after raporu üretir. Baseline 554 → (Claude coherence guard ile) **555**.

## 11. Pre-live kalibrasyon + thin/degraded/healthy gerçekçilik audit (Claude, 2026-06-07)
> Canlı sayı yok → eşikleri **uydurmuyorum**; bunun yerine motorların davranışını gerçek veri-üretim yapısına karşı denetledim.

**Bulgu — `degraded` tabanı:** `data_pipeline_quality` status'ü, `coverage_low = matchup_count < 1000 ∥ build_cov < 0.95 ∥
rate_cov < 0.90` olduğu sürece **en iyi ihtimalle `degraded`** (asla `healthy`). Match-V5 hattı tek aktif oyuncu (~20 ranked) +
discovery crawl (per-player cap, batch 6, saatlik tick) ile 1000 matchup'a uzun süre ulaşmaz → status ramp'in tamamında
`degraded` okur. **Bu dürüst ama statik:** tek başına "takıldı/kötü" gibi okunur, oysa veri sağlıklı büyüyor olabilir.
Çözüm **hedefleri düşürmek DEĞİL** (bu sağlık uydurması olur) → `coverage_ramp.ramp_state`'i status'ün yanında göstermek:
`degraded + progressing` = "ısınıyor, hedefe yaklaşıyor"; `degraded + stalled` = "takıldı, müdahale gerek". Mimari bunu zaten destekliyor.
**Kilitlendi:** `coverage_ramp::complements_pipeline_quality_below_target_but_growing` — aynı erken-ramp state'inde quality=degraded
+ ramp=progressing; ileride bir kalibrasyon değişikliği ikisini çelişkiye düşürürse (örn. ramp progressing iken quality insufficient) **kırmızı**.

**Eşik envanteri (canlı turdan sonra SIKILACAK):**

| Eşik | Değer | Durum | İlk canlı run neyi doğrular |
|---|---|---|---|
| `coverage_ramp::HIGH_FAILURE_RATIO` | 0.25 | ⏳ PROVISIONAL | gerçek failed/total oranı (rate-limit/parse hatası tabanı) |
| forward-only invariant (rate/matchup/build/processed ≥0) | — | ✅ GROUNDED (mantıksal) | upsert gerçekten ileri-yönlü mü |
| `pipeline_quality::HIGH_MATCHUPS` | 1000 | ⏳ erişilebilirlik | saatlik matchup üretim hızı → 1000 ne kadar uzakta |
| `pipeline_quality::RATE_COVERAGE_MIN` | 0.90 | ⏳ | erken ramp'te rate cov gerçekçi mi |
| `pipeline_quality::HIGH_BUILD_COVERAGE` | 0.95 | ⏳ | Match-V5 build kapsaması bu bandı yakalıyor mu |
| `pipeline_quality::SOURCE_STALE_HOURS` | 48 | ⏳ | scheduler saatlik → 48h stale gerçekçi |
| min-processed/saat (henüz YOK) | — | 🔜 | canlı throughput'tan türetilecek (stalled eşiği) |

**Codex çalıştırınca Claude (key gelince):** (1) `measure_live_coverage_ramp` çıktısındaki failure_ratio/throughput'a göre
HIGH_FAILURE_RATIO + yeni min-processed eşiği. (2) matchup üretim hızına göre 1000 hedefi gerçekçi mi yoksa ara "warming_up"
bandı mı gerek (sadece veri gösterirse — uydurmadan). (3) thin/degraded/healthy bant audit'i gerçek sayılarla teyit.

## 12. İLK CANLI RUN sonuçları (2026-06-07) + Claude grounded tur
**Codex canlı çalıştırdı** (PUUID fix sonrası: local DB'deki aktif summoner PUUID'si LCU-UUID formatındaydı → Match-V5 400;
Codex `is_lcu_uuid_puuid` guard + Account-V1 `get_by_riot_id` ile gerçek Riot PUUID çözümü ekledi, DB'ye upsert).

| Metrik | before | after | delta |
|---|---|---|---|
| champion_rate_rows | 0 | 49 | **+49** |
| matchup_rows | 80 | 140 | **+60** |
| build_rows | 31 | 80 | **+49** |
| discovered_matches | 0 | 24 | **+24** |
| processed_matches | 0 | 6 | **+6** |
| crawled_players | 0 | 52 | **+52** |

`ramp_state=progressing` · `process_ratio=0.2` (6/30) · `failure_ratio=0.0` · obs: coverage_growing + new_matches_discovered +
processing_advancing · refresh: degraded **→ stale** · match_v5_matches=6 · errors=0 · cache=promote.
**→ "degraded/stale + progressing = warming up / healthy growth" yorumu CANLI VERİYLE DOĞRULANDI.**

**Claude bu turda (eşiklere DOKUNMADAN — tek veri noktası, failure 0.0, sıkıştırma kanıtı yok):**
- **`classify_data_trajectory(quality_status, ramp_state)`** — Codex önerisi #3'ün engine-pure çekirdeği. quality+ramp'i tek
  kullanıcı-yüzü token'a birleştirir (`DATA_TRAJECTORIES`): öncelik `regressing`(veri kaybı) > `deferred` > `healthy`(hedefte) >
  **`warming_up`**(hedef altı ama ilerliyor) > `stagnant`(hedef altı + durağan). Canlı run → stale+progressing → **warming_up**.
- **Regression anchor** `reproduces_first_live_ramp_2026_06_07` — canlı before/after birebir; motor bu ölçülen verdict'i üretmeyi
  bırakırsa kırmızı (+49/+60/+49/+24/+6/+52, ratio 0.2, failure 0).
- **stale coherence** `stale_quality_with_growing_ramp_is_warming_up` — after_status=stale çıktığı için (degraded değil); growing
  ramp stale ile de tutarlı.
- **Eşik kararı:** HIGH_FAILURE_RATIO **0.25'te bırakıldı** (gerçek 0.0; PROVISIONAL yorum eklendi). process_ratio 0.2 ilk tur
  için kabul (backlog discovery sonrası normal). min-processed/saat eşiği **henüz EKLENMEDİ** — 3-5 run gerekiyor (uydurma yok).

**Açık runtime not (Codex):** `after_status=stale` = Match-V5 maçlarının patch'i client'ın current_patch'inden farklı olabilir
(patch mismatch). UI'da `warming_up` bunu yumuşatır ama kök-neden (Match-V5 maç patch'i vs DDragon current) Codex'in bakması iyi olur.

**Sonraki (kullanıcı 3-5 run daha çalıştırınca) Claude:** çoklu-run failure_ratio + processed/run dağılımından min-processed eşiği
türet; matchup/run hızından 1000 hedefine ETA → ara band gerçekten gerekiyor mu karar; thin/degraded/healthy bant teyidi.
**Codex sonra (UI):** `classify_data_trajectory` token'ını quality kartına bağla + `dataPipeline.dataTrajectory.*` i18n
(warming_up="Veri büyüyor, hedefe yaklaşıyor" vb.) → Claude drift guard bağlar.

## 13. ÇOKLU-RUN kalibrasyon (2026-06-07, 4 run) — grounded eşik turu
Ek canlı run'lar hatasız. Kümülatif son state: rates **169** / matchup **428** / build **209** / discovered **69** /
processed **36** / failed **0** / crawled **308**. Son 3 run **stabil per-run**: +6 processed · +56-58 matchup ·
+19-29 build · +9 discovered · +50 crawled · failure_ratio **0.0**. `process_ratio` 0.32→0.33→0.34 (kümülatif 36/105≈0.343).
Backlog büyüyor ama sağlıklı (discovery, process'ten hızlı — by design; Codex "sorun değil"). 1000 matchup hedefine
428'den **~10-11 run** kaldı (~57/run).

**Claude grounded kararlar (kanıta dayalı, uydurma yok):**
- **`HIGH_FAILURE_RATIO=0.25` CONFIRMED, değişmedi** — 4 run boyunca gerçek failure 0.0; sıkıştırma için kanıt yok (yorum güncellendi).
- **`GROUNDED_PROCESSED_PER_RUN=6`** (pub const) — 3 ardışık run +6/run (batch_limit=6) → provisional baseline kayıt altında.
- **Yeni soft sinyal `processing_below_expected`** (RAMP_OBSERVATIONS 9→10): backlog varken (pending>0) processed ilerliyor
  ama floor altında (`0 < processed_delta < MIN_HEALTHY_PROCESSED_PER_RUN = 3` = grounded/2) → "ilerliyor ama yavaş", tam-stall
  DEĞİL. Gerçek sağlıklı run'da (+6) tetiklenmez; 5/6 gibi bir-eksik run gürültü olmaz; ≤2/6 backlog'lu run yakalanır.
  Çoklu-run penceresinde processed_delta büyük → false-positive yok (yalnız false-negative, soft watch için kabul).
- **Test:** `steady_state_run_processes_healthily` (kümülatif run-4 birebir: +6 proc/+57 matchup, process_ratio 0.343, hiçbir
  alarm yok) + `sluggish_processing_with_backlog_flagged` (processed_delta=2 + backlog → processing_below_expected, hâlâ progressing).
- **Min-processed/saat eşiği hâlâ HARD-LİMİT değil** — sadece soft observation. Daha fazla run + gerçek bir yavaşlama görülmeden
  ramp_state'i değiştirecek hard threshold eklenMEYECEK (no fabrication).

cargo test 561 (1 ignored=live smoke) · clippy 0 · fmt 0 · typecheck pass · vitest 193/50 (contract test observations 9→10).
**Kalibrasyon yeterli düzeyde kapandı.** Kalan: 1000 matchup'a ~10-11 run (kullanıcı isterse arka planda biriktirir);
`processing_below_expected`/`data_trajectory` token'ları UI'ya bağlanınca i18n drift guard.

## 14. 5000 collection target + 1000 healthy line (iki-sayı modeli) + `enriching` (2026-06-07)
**Codex runtime:** (a) `.env` runtime-reload (`riot/client.rs` reload_runtime_env/runtime_client_from_env/runtime_riot_configured;
key değişince app restart gerekmez, sonraki tick fresh client). (b) **`MATCH_V5_TARGET_MATCHUPS` 1000 → 5000** (`commands/data_quality.rs`,
**scheduler toplama hedefi**; 1000'i geçince skip_fresh'e düşüp yavaşlıyordu). **`data_pipeline_quality::HIGH_MATCHUPS=1000`
DEĞİŞMEDİ** — bilinçli iki-sayı modeli: **1000 = kalite "healthy" çizgisi** (bu kadar veri kararları sağlam kılar),
**5000 = toplama hedefi** (warmup boyunca zenginleştirmeye devam). Bu bir bug değil; ikisi ayrı amaç.

**Canlı durum (5000'e geçtikten sonra):** rates 292 · matchups **1454** (1000 çizgisini geçti) · builds 473 · processed 162 ·
failed **0** · players 1414. Match-V5 logu: `6 matches, 59 rates, 60 matchups, 59 builds, 0 errors` (per-run +6 proc/+60 matchup stabil).
cargo test 563 (Codex).

**Claude grounded tur (2026-06-07):** Veri 1000'i geçtiği için trajectory mesajı tutarsızdı (1454 matchup'ta "healthy" demek
hâlâ 5000'e doğru büyüdüğünü gizliyor). Yeni trajectory **`enriching`** (DATA_TRAJECTORIES 5→6): `quality==healthy && ramp==progressing`
→ "Veri sağlıklı, zenginleşmeye devam ediyor" (5000 hedefine doğru). `healthy + (no_activity∥stalled)` → düz `healthy`. Diğer dallar değişmedi.
Grounded anchor `post_1000_run_keeps_progressing_toward_5000` (canlı 1394→1454 matchup, +6 proc, failure 0 → progressing; 1000'i geçmek
ramp verdict'ini değiştirmez — quality çizgisi ve toplama hedefi bağımsız). Truth-table'a enriching/healthy-stalled eklendi.
cargo test 564 · clippy 0 · fmt 0 · typecheck pass · vitest 193/50 (yeni ts-rs DTO yok — trajectory String token).

**Açık (Codex döndüğünde / UI):** `data_trajectory` token'ı (healthy/enriching/warming_up/stagnant/regressing/deferred) quality
kartına bağlanınca `dataPipeline.dataTrajectory.*` i18n (enriching="Veri sağlıklı, zenginleşiyor"; warming_up="Veri büyüyor,
hedefe yaklaşıyor") → Claude `every_emitted_trajectory_token_has_an_i18n_label` drift guard. `after_status=stale` patch-mismatch
kök-nedeni hâlâ Codex notunda. Matchups 2000/3000/5000 bandına gelince thin/degraded/healthy yorumu tekrar değerlendirilebilir
(şu an gerek yok — bantlar gerçek veriyle dürüst).

## 15. Trajectory UI BAĞLANDI — full-stack (2026-06-07, Claude, Codex yokken)
Kullanıcı "Codex yokken full-stack çalışabilirsin" dedi → trajectory açık halkası uçtan uca bağlandı (engine→command→UI→i18n→guard).
- **Backend (`commands/data_quality.rs`, `lib.rs`):** `run_scheduler_tick` artık **best-effort ramp ölçüyor** (tick başı before-snapshot
  → refresh'ler → after-snapshot → `evaluate_coverage_ramp` → `AppState.last_coverage_ramp`'e yazar; **non-fatal**, hata tick'i bozmaz,
  network yok yalnız COUNT). Yeni `LastCoverageRamp` (in-memory, restart'ta sıfır) + `DataTrajectoryView` (ts-rs: trajectory/
  quality_status/ramp_state/data_growing/measured_at — **bigint yok**, measured_at u32→number|null). Yeni komut **`get_data_trajectory`**:
  güncel quality status + son scheduler ramp_state'i `classify_data_trajectory` ile birleştirir; ramp henüz yoksa `unknown`. lib.rs
  AppState field + handler kaydı.
- **Frontend:** ChampSelectWrapper `get_data_trajectory` çeker (refreshQualityReports) → Screen → **DataStatusBadges**. Trajectory,
  statik pipeline-status chip'inin **etiketini değiştirir** (3-chip kuralı korunur, yeni chip eklemez): `degraded` yerine
  "Veri büyüyor, hedefe yaklaşıyor"/"Veri sağlıklı, zenginleşiyor"; zengin gap/action hint korunur. `unknown` → statik status'e fallback.
  trajectoryKind: healthy/enriching→good, regressing/stagnant→danger, warming_up/deferred/unknown→neutral.
- **i18n + guard:** tr/en `dataPipeline.dataTrajectory.*` (7 token: 6 engine + unknown) + i18n-parity REQUIRED_KEYS + Rust
  `every_emitted_trajectory_token_has_an_i18n_label` (include_str!(tr.json), DATA_TRAJECTORIES + unknown).
- **Test:** TS contract DataTrajectoryView (exhaustive keyof + bigint-yok + vocab) + 2 DataStatusBadges component testi
  (warming_up etiketi statik status'ü ezer; unknown→fallback).
- **Doğrulama:** cargo test 566 · clippy 0 · fmt 0 · typecheck pass · vitest 196/50. **Kullanıcı artık champ-select'te trajectory
  rozetini görüyor** (scheduler ilk tick ramp ölçtükten sonra; o ana kadar `unknown`→statik status).
