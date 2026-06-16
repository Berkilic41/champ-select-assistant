# champ-select-assistant — Claude Code Config

> **Mimari pivot (2026-06): proje Tauri'den Electron'a göçtü.** Eski Tauri /
> `src-tauri/` referansları GEÇERSİZDİR. Tam güncel durum: `PROJECT_STATE.md`.
> Otonom geliştirme döngüsü + kurallar: `AGENTS.md`; kalite kapıları: `QUALITY_CHECKS.md`.

## Stack

- **core** — Rust → WASM (`wasm-pack --target nodejs`), deterministik motor: scoring,
  draft_brain, pipeline policy, json_api. `core/src/`. **Test-oracle.**
- **desktop host** — Electron 40 + Node (node:sqlite, LCU, IPC, scheduler, kaynak
  fetch'leri, WASM `Engine` sarmalı). `desktop/src/main/`.
- **renderer** — React 19 + TypeScript 5 + Vite 7, i18n (tr/en). `src/`.
- **worker** — Cloudflare Worker (wrangler): Match-V5 ingestion → D1 → `/v1/{rates,
  matchups,builds}`. `cloudflare-worker/`.
- **Shared types** — ts-rs (Rust → TS), `src/types/generated/`.
- **Paket yöneticisi** — pnpm 11 workspace (`desktop`, `cloudflare-worker`; renderer = kök paket).

## Build & Test (tam liste: QUALITY_CHECKS.md)

```powershell
pnpm dev                                       # renderer (Vite)
pnpm --filter csa-desktop dev                  # Electron host (dev)
pnpm typecheck ; pnpm test:run                 # renderer: tsc --noEmit + vitest
pnpm --filter csa-desktop test                 # host testleri (+ test:e2e app-launch smoke)
# core (cwd core/):
cargo test --all ; cargo clippy --all-targets --all-features -- -D warnings
pnpm --filter champ-select-riot-proxy test     # worker
```

## Folder Conventions

| Path | Contents |
|------|----------|
| `core/src/` | Rust/WASM motor: scoring, draft_brain, pipeline policy, json_api |
| `desktop/src/main/` | Electron host; `ipc.ts` = tek `"cmd"` dispatcher + `buildCommandRegistry()` |
| `desktop/src/main/commands/` | IPC komut handler'ları |
| `desktop/resources/migrations/` | SQL migration'lar (`V0NN__*.sql`, node:sqlite) |
| `desktop/resources/seeds/` | Bundled seed JSON (`builds_seed`, `meta/matchup_seed`) |
| `src/components/` | React UI |
| `src/hooks/` | Custom hooks |
| `src/i18n/` | `tr.json` + `en.json` (parite zorunlu) |
| `src/types/generated/` | ts-rs üretilen TS tipleri |
| `cloudflare-worker/src/` | Edge worker (ingest + `/v1` read) |
| `docs/` | Landing (GitHub Pages) + `live-smoke-checklist.md` |

## Critical Rules

- NEVER commit `.env` / API key / secret.
- IPC: renderer `window.api.invoke(name, args)` → tek `"cmd"` dispatcher → `buildCommandRegistry()` Map. Yeni komut renderer'da çağrılıyorsa dispatcher'a da eklenmeli (ipc-contract testi yakalar).
- ALWAYS run migrations on startup before any DB access (`index.ts` boot sırası: DB+migrations → `Engine.load()` → scheduler → IPC).
- Deterministik motor (core) **test-oracle**'dır; ML/LLM yalnız fallback seam'iyle AUGMENT eder, sessizce değiştirmez.
- i18n paritesi: `src/i18n/{tr,en}.json` anahtarları senkron olmalı (i18n-parity testi).
- DDragon sürüm fallback `14.10.1`; `"unknown"` sentinel reddedilir (ikon URL'i bozulmasın).
- combo `ability_ref` mekanik olarak doğru olmalı (abartı/kesin dil yok).

## Commit Convention

```
feat(scope): ...
fix(scope): ...
perf(scope): ...
```

## Performance Target

Champ-select event → öneri görüntülenmesi: **< 500ms**
