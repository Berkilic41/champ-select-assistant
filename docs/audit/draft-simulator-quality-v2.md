# Draft Simulator Quality v2 + Draft Fork — Audit (Sprint G)

> Tarih: 2026-06-05 · Sahip: Claude (2. mühendis) · Kapsam: draft_simulator için ölçülen kalite matrisi,
> tüm simülatör cümlelerinin coach_quality re-audit'i, ve "draft fork" (A vs B) saf yardımcısı. **Hot
> dosyalara dokunulmadı** (ChampSelectWrapper/ChampSelectScreen/DraftSimulatorPanel.tsx, commands/draft_simulator.rs).
> Sayılar **ölçüldü** (testten), varsayılmadı. Tamamlayıcı: [draft-simulator-core.md](draft-simulator-core.md).

## 1. Kalite matrisi — 28 senaryo

`draft_simulator_quality.rs` testi gerçek `compare_moves` motorundan 28 senaryo geçirir; her senaryoda
beklenen kalite özelliğini **ve** her cümlenin coach_quality re-audit'ini doğrular.

### Ölçülen sonuç (test çıktısı)
```
        blind_risk: 4/4 held
     combo_synergy: 4/4 held
    damage_balance: 4/4 held
            engage: 4/4 held
    greedy_scaling: 4/4 held
          identity: 4/4 held
    peel_frontline: 4/4 held
  sentence-audit failures: 0
```
**28/28 senaryo beklendiği gibi davranıyor; 0 cümle-audit hatası.**

| Kategori | Ne ölçülüyor | Örnek senaryo → beklenti |
|---|---|---|
| `damage_balance` | mono-damage cezası / denge kazanımı | 4AP + AP → `damage_balance` worsened; 3AP + AD → improved |
| `engage` | engage gap doldurma | engage'siz comp + vanguard/diver/catcher → `engage` improved |
| `peel_frontline` | peel/ön saf ihtiyacı | squishy comp + warden → `peel`+`frontline` improved |
| `blind_risk` | blind first-pick riski | blind+first + assassin → risk `high` + `blind_safety`; + warden → `low` |
| `greedy_scaling` | açgözlü scaling riski | scaling comp + scaling pick → `execution_risk` worsened |
| `combo_synergy` | combo sinerjisi riski gizlemez | combo pick → `synergy` improved AMA `execution_risk` risk'te kalır |
| `identity` | obje/lane kimliği netliği | poke'suz comp + artillery → `objective_identity` improved |

## 2. coach_quality re-audit (tüm cümleler)

`audit_sim_result(&DraftSimResult)` her sonucun 5 cümlesini (coach_sentence, why_this_move,
why_not_alternative, risk.summary, plan_shift.note) denetler:
- **abartı/garanti dil yok** (`has_absolute_language`)
- boş/anlamsız değil (`is_meaningful ≥ 2`)
- **runaway yok** (tek cümle ≤ 60 kelime)
- faktör listeleri **deduped** + bir faktör hem improved hem worsened OLAMAZ (conflict yok)

28 senaryonun tamamında **0 hata**. Ek sanity testi: enjekte edilen "kesin kazandırır" cümlesini auditor
gerçekten yakalıyor (`absolute:coach_sentence`).

## 3. Kalite düzeltmesi (matrisin yüzeye çıkardığı)
**Mono-damage risk size-guard:** tek/iki şampiyonlu kısmi draft'ta `damage_balance` trivially ~0 (tek champ
hep "imbalanced") → yanlışlıkla risk yükseltiyordu. `build_risk` artık damage_balance riskini **yalnız ≥3 pick**
varken işaretliyor (gerçek takım). Bu, blind+warden gibi güvenli erken pick'lerin yanlış "medium" almasını
düzeltti. (draft_simulator.rs — Claude'un core dosyası; Codex command'ı etkilenmez, sadece daha doğru risk.)

## 4. Draft Fork (A vs B) — saf yardımcı
`draft_fork.rs`: `compare_fork(state, move_a, move_b) -> DraftFork` (ts-rs export).
- `option_a` / `option_b`: iki pick'in tam DraftSimResult'ı.
- `plan_divergence`: kimlik eksenleri nasıl ayrışıyor (A → engage, B → scaling gibi).
- `risk_divergence`: risk seviyeleri farkı.
- `shared_factors`: ikisinin de güçlendirdiği faktörler · `diverging_factors`: birinin geliştirip diğerinin
  zayıflattığı faktörler.
- `recommendation`: **hedged** yönlendirme (yakınsa konfor/rol; değilse "biraz daha katkı veriyor ama
  risklerini tart" — garanti dil yok).
- 3 test: engage-vs-scaling kontrastı + hedged dil, deterministiklik, yakın-seçenek konfor çerçevesi.

## 5. Durum
- Baseline: cargo test **380** · gate clippy `-D warnings` **0** · fmt-all temiz · pnpm typecheck pass · vitest 156/38.
- Yeni saf modüller: `draft_fork.rs` (A/B fork, ts-rs `DraftFork`), `draft_simulator_quality.rs` (audit + 28-senaryo matris).
- `DraftFork.ts` üretildi (DraftSimResult'ı içerir, bigint yok).

### Codex'e (sonra — opsiyonel UI)
- Draft Fork'u bir command'a sarıp panelde "A pick mi B pick mi?" karşılaştırması göster (iki aday seçilince).
  `compare_fork` saf; archetype/damage/combo çözümü command layer'da (engine saflığı korunur).
- Simülatör cümleleri artık QA-kalkanlı (`audit_sim_result`); regresyon olursa matris testi kırmızı.
