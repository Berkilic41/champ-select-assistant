# Draft Brain Explanation Grounding v1 — Audit

> Tarih: 2026-06-07 · Sahip: Claude (2. mühendis) · Kapsam: #2 Draft Brain özgüllük/grounding bloğunun
> **ölçüm temeli**. `explanation_audit` "her pillar PRESENT mı?" sorusunu sorup **%100 presence** ölçtü; bu modül
> kullanıcının işaret ettiği bir sonraki ekseni sorar: cümle gerçekten **bu draft'ın verisine mi dayanıyor**
> (karşı şampiyonu, build item'ını, takım ihtiyacını, patch'i adıyla anıyor mu) yoksa her pick için aynı okunacak
> **generic koç şablonu** mu? Saf: DB/network/UI YOK. Tamamlayıcı: [draftbrain-explanation-quality.md](draftbrain-explanation-quality.md).

## 1. Modül
`recommendation/explanation_grounding.rs` (saf, `#[allow(dead_code)]` coach/QA yüzeyi bağlanana dek).
Giriş: `score_grounding(prose, &GroundingContext) -> GroundingReport` · `audit_grounding(&Recommendation, &ctx)`
(rec'in **reasoning** alanlarını toplar — decision_sentence/lane/mid/teamfight/fallback/risk/why_not; enemy_team_summary
ve provenance HARİÇ, onlar veri-dökümü, gerekçe değil).

## 2. DTO'lar
**Input (Rust-only):** `GroundingContext` (enemy_names/ally_names/item_names/team_need?/patch? — caller komut katmanında
çözer; boş alan = **not applicable**, ne ödül ne ceza).
**Output:** `GroundingReport` (verdict/grounded_score/grounded_dimensions/applicable_dimensions/specificity_hits/generic).

## 3. Token vocabulary (sabit `pub const`)
- **GROUNDING_DIMENSIONS [5]:** `matchup` · `synergy` · `build` · `team_need` · `patch`
  (matchup/synergy/build proper-noun sürücülü; team_need/patch tek-token, daha zayıf eksen).
- **GROUNDING_VERDICTS [4]:** `grounded` · `partial` · `generic` · `insufficient_context`

## 4. Mantık
- **Boundary-aware token eşleşmesi** (`mentions`): needle, haystack'te iki yanı alfanumerik-olmayan tam-token olarak
  geçmeli — "Vi" "Viktor"un içinde **eşleşmez**, "Vi'nin"/"Vi koparır" eşleşir. Lowercase + Unicode-duyarlı (TR ı/ş/ç).
  Tek-karakter token'lar non-specific sayılır (atlanır).
- **Uygulanabilirlik:** yalnız context token'ı olan boyut **applicable**. Blind pick (düşman yok) matchup'tan
  ceza almaz → **no fabrication**.
- **grounded_score = grounded_dims / applicable_dims** (applicable 0 → 0.0).
- **verdict:** applicable 0 → `insufficient_context`; ≥0.67 → `grounded`; ≥0.34 → `partial`; else → `generic`.

## 5. Test (8 — hepsi geçti)
fully-grounded (5/5 boyut) → grounded · generic şablon (gerçek Draft Brain prose şekli, 0 proper-noun) → generic ·
enemy+patch ama build/synergy yok → partial · blind/no-context → insufficient_context · **boundary** (Vi≠Viktor) ·
yalnız-applicable sayılır · **vocab lock** · **gerçek-pipeline ölçüm**.

## 6. ÖLÇÜM (gerçek pipeline, audit number)
4 görünür-matchup senaryosu (mid-vs-Ahri, top-vs-Darius, bot-vs-Jinx+Leona, sup-vs-Thresh) gerçek
`compute_recommendations` + `upgrade_*` çıktısından, `GroundingContext.enemy_names` ile skorlandı:

```
=== Draft Brain explanation GROUNDING (5 recs) ===
  grounded: 0   partial: 0   generic: 5
```

**Bulgu net:** presence %100 ama **grounding %0** — mevcut prose karşı şampiyonu **hiç** adıyla anmıyor. Bu, "generic
koç cümlesi mi, draft'a özel mi" frontier'ının **ölçülmüş** kanıtı. (Eşik assert edilmedi; bu bir baseline.)

## 7. BUILDER GROUNDING UYGULANDI — Tier-1 (2026-06-07, Claude)
> Kullanıcı yaklaşımı onayladı: Tier-1 (matchup + team_need + synergy), engine→builder akışı, **arşetip-seviyesi dürüst
> grounding** (ad + sahip olduğumuz arşetip/power-curve; ability mekaniği UYDURMA — CLAUDE.md `ability_ref` kuralı).

**Veri akışı:** Engine zaten `enemy_laner_name`'i çözüyordu (Precomputed) → yeni `Recommendation.lane_opponent_name:
Option<String>` alanı (models.rs, serde default + skip_if_none; hand-written `src/types/recommendation.ts`'e `?:` optional)
engine.rs:545 constructor'da `pre.enemy_laner_name`'den dolduruldu. 4 Rust literal + 3 test fixture güncellendi.

**Builder değişimi (`draft_brain.rs`, Codex-hot — Codex yok, full-stack izni):**
- **matchup:** `upgrade_recommendation_with_context` sonunda finalize edilmiş lane_plan'ı `ground_lane_with_opponent` ile
  rakibin adıyla öne çerçeveler ("Ahri karşısında: …") — kaynağı ne olursa olsun (engine `lane_phase_advice` ∥ heuristik
  builder). decision_sentence sonra rebuild edilir → "Lane:" clause'u grounded prose'u taşır. **No-op** rakip bilinmiyorsa
  (blind/first-pick/ARAM) ∥ zaten adıysa → boş isim-damlatma yok.
- **team_need:** `build_mid_game_plan` `draft_plan.fills_team_need[0]`'i adıyla anar ("Takıma AD burst katıyorsun…").
- **synergy:** `combo_driver` zaten `ally_champion_key`'i anıyordu (mevcut).

## 8. ÖLÇÜM — sonra (gerçek pipeline)
```
=== Draft Brain explanation GROUNDING (5 recs) ===   (BUILDER GROUNDING SONRASI)
  grounded: 5   partial: 0   generic: 0
```
**Grounding %0 → %100** (görünür lane rakibi olan 4 senaryo). `grounding_measurement_over_real_pipeline` testine
**regresyon eşiği** eklendi: matchup-applicable rec'lerde **≥%60 grounded** zorunlu → şablona geri dönüş kırmızı.
2 yeni draft_brain testi: grounded lane+mid+decision (Ahri/AD burst) + blind-pick rakip-uydurmaz.

Baseline: cargo test **576** · clippy 0 · fmt 0 · typecheck pass · vitest 196/50.

## 9. DERİN MATCHUP GROUNDING UYGULANDI (2026-06-07, Claude)
`narrative.rs::build_lane_phase_advice` artık power-curve tempo satırına **rakip-arşetip-spesifik** bir uyarı ekliyor
(`opponent_threat_note`): aynı late-scaler iki farklı early-bully'ye (juggernaut vs assassin) karşı artık **farklı** tavsiye alır
— juggernaut → "uzun all-in verme; kite edip kısa takasla aşındır"; assassin → "lvl 6 sonrası all-in ve roam penceresine karşı
pozisyon al". **Yalnız KB arşetip sınıfı** (ability mekaniği UYDURMA). Test `lane_phase_advice_tailors_to_opponent_archetype`
(iki arşetip → farklı satır, ikisi de freeze; assert_ne). **Word-budget düzeltmesi:** derin tavsiye decision_sentence'ı
60-kelime sınırını aşırdı (3 pipeline-audit testi kırıldı) → `build_decision_sentence` "Lane:" clause'una artık yalnız lane
plan'ın **ilk clause'unu** (";" öncesi tempo+ad) inline ediyor; arşetip-threat kuyruğu standalone lane_plan'da kalıyor
(kendi UI bölümünde). decision_sentence punchy, lane_plan tam-derin. cargo test **577** · clippy 0 · fmt 0.

## 10. BUILD GROUNDING UYGULANDI — Tier-2 (2026-06-07, Claude)
`lane_opponent_name` desenini birebir tekrarlar: yeni `Recommendation.core_item_name: Option<String>` (models.rs serde
default+skip; hand-TS `?:`). **Command** (`champ_select.rs`) `enrich_build_for_rec`'ten SONRA `resolve_core_item_name`
ile `core_items[0]`'ı item cache'ten (`cdragon::ItemData`) isme çevirip rec'e koyar (hem `get_recommendations` hem
`get_champion_analysis`; enrich→resolve→upgrade sırası doğru). **Builder** (`draft_brain.rs`) `ground_mid_game_with_build`:
macro plan'a ilk-item notu ekler — **provenance dürüst** (`seed`→"oturmuş seed build", aksi→"arşetip-genel, kaynaklı winrate
yok"; **item stat/winrate UYDURMA**). Word-budget güvenli (decision_sentence'a değil, uncapped mid_game'e). 2 test:
`macro_plan_is_grounded_in_the_build` (Kraken Slayer + seed provenance) + `macro_plan_without_build_invents_no_item`
(build yoksa "İlk item" yok). build dimension `explanation_grounding`'de zaten unit-test'li (fully_grounded item_names).

**Grounding 5 boyut artık tam:** matchup ✓ · synergy ✓ · team_need ✓ · **build ✓** · patch (ertelendi, düşük değer).
cargo test **579** · clippy 0 · fmt 0 · typecheck pass · vitest 196/50.

## 11. Kalan / sonraki
- **patch grounding:** düşük değer (filler riski) — ertelendi.
- **UI (opsiyonel):** grounded/generic "özgüllük" sinyali; ts-rs export gerekirse.
- **Not (ölçüm):** saf pipeline (compute_recommendations+upgrade) item adı çözmez (command katmanı işi) → build grounding
  saf real-pipeline measurement'ında görünmez; builder unit-test'iyle ölçülür (dürüst — isimler gerçekten command'da).

> Sıra: Claude grounding ölçerini kurdu (presence %100→grounding %0 ölçüldü), sonra builder'ları grounded yaptı
> (%0→%100) ve regresyon eşiğiyle kilitledi. "İyileştirmeyi ölçülebilir yap → ölç → iyileştir → eşikle kilitle" döngüsü tam.
