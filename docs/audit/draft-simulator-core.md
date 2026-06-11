# Draft Simulator Core — Engine-Pure Module (Sprint F)

> Tarih: 2026-06-05 · Sahip: Claude (2. mühendis) · Kapsam: "draft okuma / hamle simülasyonu" saf çekirdeği.
> **UI yok, hot dosyalara dokunulmadı** (HeroCard/DataStatusBadges/ChampSelectWrapper/ChampSelectScreen.tsx,
> engine.rs, scoring.rs, commands/champ_select.rs). Tamamen saf: DB/network yok. Veri uydurma yok —
> trait değerleri `champion_types`/`scouting`'in kullandığı **aynı KB archetype taksonomisinden** türetilmiş
> grounded heuristik; tüm koçluk dili hedged (garanti dil yok, `coach_quality` ile test-doğrulandı).

## 1. Modül
`src-tauri/src/recommendation/draft_simulator.rs` (saf, `#[allow(dead_code)]` UI bağlanana dek).

## 2. Modeller (ts-rs export'lu)
`DamageType` (ad/ap/mixed/true) · `SimChampion` (champion_id, key, archetype, damage, combo_partner_ids) ·
`DraftSimState` (my_team, enemy_team, blind, first_pick) · `DraftSimMove` (champion, position?) ·
`DraftSimInput` (state, candidate_moves) · `DraftSimDelta` (factor, before, after, delta) ·
`DraftSimRisk` (level, summary, factors) · `DraftSimPlanShift` (before, after, note) · `DraftSimResult`.

## 3. Motor (saf)
- `apply_move(state, move) -> DraftSimState` — şampiyonu my_team'e ekler (clone, saf).
- `evaluate_state(state) -> DraftSimResult` — kompozisyonu boş-baseline'a karşı okur.
- `compare_moves(state, moves) -> Vec<DraftSimResult>` — her hamleyi aynı base'e karşı; **deterministik**
  (`score_delta` desc, eşitlik `champion_id` asc).
- `simulate(input) -> Vec<DraftSimResult>` — command layer girişi.

## 4. Değerlendirme boyutları (11 faktör, machine-key)
`damage_balance` · `engage` · `disengage` · `frontline` · `peel` · `scaling` · `lane_pressure` ·
`objective_identity` · `execution_risk` (yüksek = kötü) · `blind_safety` · `synergy`.

- **Grounding:** her archetype → `profile_for(archetype)` 9 trait (engage/disengage/frontline/peel/scaling/
  lane_pressure/objective/exec_risk/blind_safety), scouting'in `playstyle_for`'u ile aynı taksonomi.
- **damage_balance:** AD/AP dağılım dengesi (mono-hasar → ~0). Move zaten baskın tipi pekiştiriyorsa
  (float taban yapsa bile) `damage_balance` worsened işaretlenir.
- **execution_risk:** archetype exec-risk ortalaması + **greedy-scaling cezası** (scaling yüksek & lane_pressure
  düşük → erken oyun riski). Bu yüzden "scaling comp'a scaling eklemek erken riski artırır".
- **synergy:** combo_partner_ids takım-içi bağlantı oranı (KB combo'ları read-only, caller çözer).
- **blind_safety:** takımın en riskli blind pick'i (min); risk yalnız `blind && first_pick` bağlamında yükselir.

> `scaling` bir eğri (iyi/kötü değil) → improved/worsened'a girmez, sadece `deltas`'ta + execution_risk'e besler.

## 5. Hamle çıktısı (her DraftSimResult)
`score_delta` · `improved_factors[]` (machine-key) · `worsened_factors[]` · `deltas[]` (tüm faktör before/after) ·
`risk` (DraftSimRisk: low/medium/high + TR summary + factor key'leri) · `plan_shift` (kimlik before→after + TR not) ·
`coach_sentence` (TR, kısa) · `why_this_move` (TR) · `why_not_alternative` (TR).

> machine-key'ler UI'da i18n-map'lenir; prose alanları TR. Aynı "token + i18n" deseni (threat_level gibi).

## 6. Guardrails
- **Abartı/garanti dil YOK** — "kazandırır/kesin" gibi ifadeler yasak; tüm üretilen cümleler `coach_quality::
  has_absolute_language` ile test-doğrulandı (her senaryoda `no_absolute`).
- Koç cümlesi kısa, açıklama grounded (faktör değişimine bağlı).
- Mevcut `coach_quality`/`explanation_audit` guardrail desenleriyle uyumlu.

## 7. Testler (7 senaryo + grounding)
- 4AP comp'a AP eklemek → `damage_balance` worsened ✓
- engage'siz comp'a vanguard → `engage` improved ✓
- scaling comp'a scaling → execution_risk yükselir (erken risk) ✓
- blind + first_pick + assassin → risk `high`, `blind_safety` faktörü yüzeye çıkar ✓
- combo pick → `synergy` improved AMA `execution_risk` risk'te kalır (gizlenmez) ✓
- compare_moves deterministik + sıralı ✓
- boş / kısmi draft güvenli fallback (panik yok, neutral) ✓
- ek: her sonuçta abartı-dil yok.

## 8. Durum
- Baseline: cargo test **370** · gate clippy `-D warnings` **0** · fmt-all temiz · pnpm typecheck pass · vitest 154/37.
- ts-rs: 9 DTO `src/types/generated/`'a üretildi (hepsi number/string/union — bigint yok).

### Codex'e (sonra — UI binding)
- `simulate`/`compare_moves`'u bir command'a sar (caller: champ-select'teki my_team + aday picks → SimChampion;
  archetype + damage + combo_partner_ids KB/champion verisinden çözülür). **Champ-select sırasında cloud yok** (saf, hızlı).
- UI: hamle başına improved/worsened faktör key'lerini i18n-map'le; risk/plan_shift/coach_sentence göster.
- archetype/damage/combo çözümü command layer'da (engine saflığı korunur — simülatör saf kalır).
