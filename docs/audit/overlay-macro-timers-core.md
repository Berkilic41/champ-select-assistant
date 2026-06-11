# Hybrid Overlay — Macro/Objective Timer Core v1 (Faz 4)

> Tarih: 2026-06-07 · Sahip: Claude · Kapsam: oyun-içi overlay'in **beyni** — oyun süresi + alınan objelerden
> bir sonraki Dragon/Baron/Herald/Voidgrubs penceresini ve kısa makro hatırlatmalarını hesaplayan **saf motor**.
> Overlay runtime (transparan always-on-top pencere + Live Client Data API polling) **sonraki dilim** (canlı oyun gerektirir).

## 1. Modül
`recommendation/macro_timers.rs` (saf, `#[allow(dead_code)]` overlay runtime bağlanana dek).
Giriş: `compute_macro_state(&MacroTimerInput) -> MacroState`.

## 2. Policy-safe (CLAUDE.md kuralı)
**Yalnız genel oyun-kuralı obje zamanlamaları (public bilgi) + kendi oyun süresi + alınan objeler (ekranda görünür).**
**Gizli bilgi YOK** — düşman cooldown/ward/summoner timer gösterilmez. Spawn/respawn değerleri public **oyun kuralları**
(sabit const, uydurma veri değil); motor yalnız aritmetik yapar. Riot **Live Client Data API** (port 2999, resmî) beslemesi —
process injection / game-memory read YOK.

## 3. DTO + token
**Input (Rust-only):** `ObjectiveEvent` (objective/killed_at_secs), `MacroTimerInput` (game_time_secs/events).
**Output (ts-rs, bigint yok — u32/i32→number):** `ObjectiveTimer` (objective/next_spawn_secs/seconds_until/state),
`MacroState` (game_time_secs/phase/objectives/reminders/phase_note).
**Vocab (pub const):** OBJECTIVES[4] grubs/herald/dragon/baron · OBJECTIVE_STATES[4] pending_first/respawning/soon/up ·
GAME_PHASES[3] early/mid/late.

## 4. Oyun kuralları (sabit, patch'e göre ayarlanır)
Dragon ilk 5:00 / respawn 5:00 · Baron ilk 25:00 / respawn 6:00 · Herald 14:00 (one-off) · Voidgrubs 6:00 (one-off) ·
SOON penceresi 45sn.

## 5. Mantık
- **Repeating (dragon/baron):** son kill + respawn, yoksa ilk spawn.
- **One-off (grubs/herald):** alındıysa listeden DÜŞER (respawn yok); aksi ilk spawn.
- **state:** seconds_until≤0→up · ≤45sn→soon · now≥first_spawn→respawning · else pending_first.
- **reminders:** up/soon objeler için TR makro hatırlatma (dragon vision/tempo, baron vision-kontrol, herald plate, grubs jungle).
- **phase:** <14:00 early · <25:00 mid · else late + phase_note (TR).
- **Deterministik sort:** en yakın spawn (clamped) → objective adı. **No fabrication.**

## 6. Test (10 — hepsi geçti)
dragon pending→up@5:00 · dragon respawn 5dk sonra · soon penceresi+reminder · grubs one-off alınca düşer ·
baron up→vision reminder · phase early/mid/late · sort+vocab-lock · no-event no-fabrication.
TS contract `macro-timers.contract.test.ts` (bigint yok + token vocab).

Baseline: cargo test **594** · clippy 0 · fmt 0 · typecheck pass · vitest 199/51.

## 7. Sonraki dilim (overlay runtime — canlı oyun gerektirir)
1. **Live Client Data client:** `GET http://127.0.0.1:2999/liveclientdata/allgamedata` (resmî, self-signed cert → rustls)
   → gameTime + events (DragonKill/BaronKill/HeraldKill/...) → `MacroTimerInput`.
2. **Overlay penceresi:** Tauri transparan + always-on-top + click-through; yalnız makro state gösterir (obje timer + reminder).
   Policy-safe: gizli bilgi yok, kullanıcı aksiyonuyla aç/kapa.
3. **i18n:** dataPipeline benzeri `overlay.objective.*` / `overlay.state.*` / `overlay.phase.*` + drift guard.
4. **Poll cadence:** 1-2sn; oyun yokken (2999 kapalı) sessiz no-op.

> Sıra: Claude saf makro-timer motorunu kurdu (fixture-test'li, deterministik, policy-safe); sonraki dilim Live Client Data
> polling + transparan overlay penceresi + i18n (canlı maçta doğrulanır). Motor "overlay'in beyni" — UI yalnız gösterir.

## 8. OVERLAY RUNTIME BAĞLANDI (2026-06-07, Pillar A) — full-stack
Motor artık canlı veriye bağlı + UI'da render ediliyor.
- **`riot/live_client.rs`:** `LiveClientApi` (LCU rustls deseni: `danger_accept_invalid_certs` + 2sn timeout) →
  `fetch_all_game_data()` GET `https://127.0.0.1:2999/liveclientdata/allgamedata`. **Saf parser** `parse_macro_input(&Value)
  →MacroTimerInput`: `gameData.gameTime`→floor, `events.Events[]` `EventName`→token (DragonKill/BaronKill/HeraldKill/HordeKill=grubs
  via `EVENT_OBJECTIVE_MAP`); eksik/bozuk→0/boş, panik yok. 4 fixture testi (token map, floor, noise filtre, malformed-no-panic,
  parser→engine dragon respawn). Fixture `tests/fixtures/live_client_allgamedata.json`.
- **`commands/overlay.rs`:** `get_macro_state()→OverlayMacroState{live:bool, state:Option<MacroState>}` (ts-rs, bigint yok).
  2999 kapalı→`{live:false,state:null}` (**Err DEĞİL**, warn yok → sessiz poll). i18n drift guard
  `every_overlay_token_has_an_i18n_label` (OBJECTIVES/OBJECTIVE_STATES/GAME_PHASES × overlay.*). lib.rs kayıt.
- **`IngameView.tsx`:** stub→canlı overlay; `get_macro_state` 1.5sn poll; faz chip + phase_note + obje satırları (mm:ss countdown
  + state pill) + reminders; hepsi i18n; oyun yoksa "Oyun bekleniyor…". IngameView.css compact (560×180). TS contract
  overlay-macro-state.contract.test.ts.
- **i18n:** tr/en `overlay.{title,waiting,reminders,now,objective.*,state.*,phase.*}` + REQUIRED_KEYS.
- **Policy-safe:** yalnız resmî Live Client Data API + kendi oyun süresi + public obje takeleri; gizli bilgi yok; injection yok.
cargo test **600** · clippy 0 · fmt 0 · typecheck · vitest 200/52. **Canlı doğrulama kullanıcının** (gerçek maçta overlay görünür);
oyun yokken `{live:false}` sessiz no-op (test-doğrulandı).
