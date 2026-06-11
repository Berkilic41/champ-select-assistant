# Champion Pool Coach Core v1 — Audit (Sprint I)

> Tarih: 2026-06-06 · Sahip: Claude (2. mühendis) · Kapsam: oyuncunun şampiyon havuzunu kapsama
> boyutlarıyla analiz eden saf motor + "3 şampiyonla rolünü güçlendir" planı + ölçülen kalite matrisi.
> **UI yok, hot dosyalara dokunulmadı.** Tamamen saf (DB/network yok). Trait'ler caller tarafından KB
> `ChampionArchetype` + mastery'den çözülür → grounded, **uydurma yok**; veri yoksa `thin` state döner.
> iTero'dan ayrışan **kişisel meta / oyuncu gelişim koçu** tarafı.

## 1. Modüller
- `recommendation/pool_coach.rs` — motor + DTO'lar (`#[allow(dead_code)]` UI bağlanana dek).
- `recommendation/pool_coach_quality.rs` — `audit_pool_plan` (coach_quality re-audit) + 28-senaryo matris.

## 2. Motor
`analyze_pool(&PoolCoachInput) -> ChampionPoolPlan` (saf + deterministik). Input: `role`, `pool` (oynadığı
şampiyonlar), `candidates` (öğrenebileceği rol-uyumlu adaylar). `PoolChampion` trait'leri (blind_safety,
execution_difficulty, power_late, engage, peel, comfort, games) caller KB+mastery'den çözer.

## 3. Kapsama boyutları (machine-key)
`role_covered` · `has_blind_safe` · `has_counter_flex` · `has_comfort` · `identity_variety` · `execution_risk`
(low/medium/high). Eksikler `PoolGap { dimension, severity, note }` olarak yüzeye çıkar.

- **role_covered:** `archetype_position_fit` (champion_types reuse) ile rol uyumu.
- **blind_safe:** havuzda blind_safety ≥ 0.6 pick var mı.
- **counter_flex:** ≥ 2 farklı archetype (matchup esnekliği).
- **comfort:** comfort ≥ 0.5 veya games ≥ 20 — **yalnız `rich` veride gap sayılır** (thin'de "bilinmiyor", yanlış ceza yok).
- **execution_risk:** havuz ortalama execution_difficulty (≥4 high, ≥3 medium).
- **identity_variety:** engage/peel/scaling'den ≥ 2'si.

## 4. "3 şampiyonla rolünü güçlendir" planı
`TrainingRecommendation` ×3 (machine-key `role_in_plan`):
- **comfort_bridge** (önce, eğer oynanmışsa) — en çok oynanmış aday; "zaten oynuyorsun" tanımlayıcı.
- **blind_safe** — en yüksek blind_safety (≥0.55 taban).
- **counter_pick** — en matchup-bağımlı (en düşük blind_safety) kalan; bilerek tepki pick'i.
- comfort fallback: oynanmış aday yoksa en kolay-exec kalan = meta köprüsü.
Aday yoksa boş plan (uydurma yok). Hepsi distinct şampiyon, ≤3.

## 5. Thin data (uydurma yok)
Havuz boş veya hiç games/comfort yoksa → `data_state: "thin"`, summary "ince veri (yeterli maç/mastery yok)".
Yapısal kapsama yine hesaplanır ama comfort gap'i bastırılır (false "konfor yok" yerine bilinmiyor).

## 6. Ölçülen kalite matrisi — 28 senaryo

`pool_coach_quality.rs` testi gerçek motordan 28 senaryo geçirir; her senaryoda beklenen kapsama/gap/plan
özelliğini **ve** coach_quality re-audit'ini doğrular.

```
        balanced: 2/2 held
      blind_safe: 4/4 held
         comfort: 3/3 held
    counter_flex: 2/2 held
       execution: 3/3 held
        identity: 2/2 held
            role: 4/4 held
            thin: 3/3 held
        training: 5/5 held
```
**28/28 senaryo beklendiği gibi.**

**coach_quality re-audit** (`audit_pool_plan`): summary + her gap.note + her training.reason → abartı/garanti
yok, meaningful, runaway yok (≤60 kelime), gap dimension/training role/champion **deduped**, ≤3 training. 28
senaryonun tamamında 0 hata. Sanity: enjekte "kesin kazanırsın" yakalanıyor.

## 7. Durum
- Baseline: cargo test **396** · gate clippy `-D warnings` **0** · fmt-all temiz · pnpm typecheck pass · vitest **166/42**.
- ts-rs: `ChampionPoolPlan` / `PoolCoverage` / `PoolGap` / `TrainingRecommendation` üretildi (bigint yok) +
  TS contract guard (`champion-pool-plan.contract.test.ts`).

## 8. Codex UI binding (✅ uygulandı, 2026-06-06)

- **Command:** `get_champion_pool_plan` — mastery + maç geçmişi + Draft IQ KB'den `ChampionPoolPlan` üretir
  (lib.rs handler kayıtlı). `PoolChampion` trait'leri server-side çözülür (archetype/blind_safety/execution KB'den,
  comfort/games mastery'den). Champ-select dışı, cloud yok.
- **UI:** `PoolBuilder.tsx` — havuz kapsaması, gap listesi, thin-data uyarısı, 3-champ gelişim planı gösteriliyor.
- **i18n:** `poolCoach.*` (dataState / coverage / executionRisk / severity / gap / training) tr/en eklendi,
  parity guard'a (REQUIRED_KEYS) bağlandı.

### Sprint H-stili guard'lar (Claude, bu tur)
- **TS contract guard güçlendirildi** (`champion-pool-plan.contract.test.ts`): `ChampionPoolPlan` (7) +
  `PoolCoverage` (6) + `PoolGap` (3) + `TrainingRecommendation` (5) için exhaustive `keyof` key-guard. Rust
  struct değişimi → `pnpm typecheck` kırılır.
- **Cross-language drift guard** (`pool_coach_quality.rs::every_emitted_pool_token_has_an_i18n_label`):
  28 senaryoyu motordan geçirip ürettiği **her token**'ın (`data_state`, `execution_risk`, gap `dimension`/
  `severity`, training `role_in_plan`) `include_str!(tr.json)` ile `poolCoach.*` label'ı olduğunu doğrular.
  Motora yeni bir token eklenip i18n unutulursa → **Rust testi kırmızı**. en parity TS testinde (parity guard).

Baseline (bu tur): cargo test **399** · gate clippy **0** · fmt-all temiz · pnpm typecheck pass · vitest **168/42**.
