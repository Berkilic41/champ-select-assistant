# Draft Fork — UI Contract + UX Audit (Sprint H)

> Tarih: 2026-06-05 · Sahip: Claude (2. mühendis) · Kapsam: DraftFork response contract'ı, draftFork.* /
> draftSimulator.factor.* i18n audit'i (cross-language drift guard), ek pure fork regresyon senaryoları.
> **Hot UI'ya dokunulmadı** (ChampSelectWrapper/ChampSelectScreen/DraftForkPanel/DraftSimulatorPanel.tsx,
> commands/draft_simulator.rs). Tamamlayıcı: [draft-simulator-quality-v2.md](draft-simulator-quality-v2.md).

## 1. DraftFork response contract

Codex `compare_fork`'u `get_draft_fork` command'ına sardı; DraftForkPanel top-2 öneri için çağırıyor.
Contract `src/types/draft-fork.contract.test.ts` ile compile-time kilitlendi:

| Alan | Tip | Not |
|---|---|---|
| `option_a` / `option_b` | `DraftSimResult` | iki pick'in tam değerlendirmesi |
| `plan_divergence` | `string` (TR) | kimlik eksenleri ayrışması |
| `risk_divergence` | `string` (TR) | risk seviyesi farkı |
| `shared_factors` | `string[]` | ikisinin de geliştirdiği faktör key'leri |
| `diverging_factors` | `string[]` | birinin geliştirip diğerinin zayıflattığı faktörler |
| `recommendation` | `string` (TR) | hedged yönlendirme |

- `DraftSimResult` shape'i de (11 alan) ayrıca kilitli. **bigint yok** — score_delta/deltas hepsi `number`.
- Rust struct'ı değişirse ts-rs regen → `pnpm typecheck` bu contract'ta kırılır (panel binding drift'ten önce).

## 2. i18n audit — ✅ tam + drift-guarded

**Mevcut durum (doğrulandı):** tr/en parity **356/356**, orphan yok. Şu key'ler tam:
- `draftFork.*` (5): eyebrow, title, optionLine, shared, diverging
- `draftSimulator.factor.*` (**11**): damage_balance, engage, disengage, frontline, peel, scaling,
  lane_pressure, objective_identity, execution_risk, blind_safety, synergy
- `draftSimulator.riskLevel.*` (3): low, medium, high

Hepsi `i18n-parity.test.ts` REQUIRED_KEYS'te (Codex ekledi) → presence + parity guard.

**Yeni cross-language drift guard (Claude):** `draft_simulator_quality.rs`'te
`every_emitted_factor_and_risk_level_has_an_i18n_label` testi — `include_str!("../../../src/i18n/tr.json")`
ile, motorun `deltas[]`'te **gerçekten ürettiği her factor key**'inin + her risk level'ın bir
`draftSimulator.factor.*` / `riskLevel.*` TR label'ı olduğunu doğrular. en parity TS testinde (356/356).

> **Sonuç:** Rust motoruna yeni bir faktör eklenir ama i18n unutulursa → Rust testi **kırmızı**. UI asla ham
> machine-key göstermez. (feedback-vocabulary.json ile aynı iki-taraflı guard deseni.)

## 3. Ek pure fork regresyon senaryoları (draft_fork.rs)
- `shared_factors_lists_what_both_picks_improve` — AP-heavy base'e iki AD aday → `damage_balance` shared.
- `diverging_factors_lists_disagreements` — marksman base'e warden (peel↑) vs assassin (peel↓) → `peel` diverging.
- `clear_margin_names_the_stronger_pick` — net fark varsa recommendation güçlü pick'i adıyla anar (hedged).
- `fork_sentences_never_run_away` — plan/risk/recommendation cümleleri ≤ 60 kelime.
- (mevcut: engage-vs-scaling kontrastı + hedged dil, deterministiklik, yakın-seçenek konfor çerçevesi.)

## 4. Durum
- Baseline: cargo test **385** · gate clippy `-D warnings` **0** · fmt-all temiz · pnpm typecheck pass · vitest **161/40**.
- Yeni: DraftFork+DraftSimResult TS contract guard, i18n cross-language drift guard (Rust), 4 fork regresyon testi.
- **Kod davranışı değişmedi** (contract/test/doc + guard). Hiçbir hot UI / command dosyasına dokunulmadı.

### Codex'e
- DraftFork UI tarafı bağlı; contract + i18n artık QA-kalkanlı. Yeni faktör/risk eklerken Rust drift testi
  i18n'i hatırlatır; struct değişimi TS contract'ı kırar.
- `recommendation`/`plan_divergence`/`risk_divergence` TR prose (i18n-map gerekmez); `shared_factors`/
  `diverging_factors` machine-key → `draftSimulator.factor.*` ile map'lenir (panel zaten yapıyor).
