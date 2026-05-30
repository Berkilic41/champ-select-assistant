# champ-select-assistant — Claude Code Config

## Stack

- **Backend**: Rust 1.80+ (Tauri 2), async/tokio, `src-tauri/src/`
- **Frontend**: React 19 + TypeScript 5 + Vite, `src/`
- **Package manager**: pnpm
- **Shared types**: ts-rs (Rust → TypeScript codegen), `shared/types/`

## Build & Test

```powershell
pnpm tauri dev          # dev (hot reload)
cargo test              # Rust unit tests (run from src-tauri/)
pnpm test               # Vitest
pnpm typecheck          # tsc --noEmit
cargo clippy            # lint
```

## Folder Conventions

| Path | Contents |
|------|----------|
| `src-tauri/src/lcu/` | LCU lockfile, HTTP client, WebSocket |
| `src-tauri/src/db/` | SQLite repo (rusqlite + refinery) |
| `src-tauri/src/riot/` | Riot API client + rate limiter |
| `src-tauri/src/ddragon/` | Data Dragon downloader + cache |
| `src-tauri/src/recommendation/` | Recommendation engine |
| `src-tauri/migrations/` | refinery SQL migration files (V001__*.sql) |
| `src/components/` | React UI components |
| `src/hooks/` | Custom React hooks |
| `shared/types/` | ts-rs generated TypeScript types |
| `docs/` | Project docs (sprint plan, ADRs) |
| `tests/` | Integration tests |

## Critical Rules

- NEVER commit `src-tauri/.env` or API keys
- ALWAYS use `rustls-tls` feature for reqwest (not native-tls) — LCU self-signed cert
- ALWAYS run migrations on app startup before any DB access
- Rate limit Riot API with `governor` — 20 req/s max
- LCU lockfile path: try 4 candidates (see `lol-lcu-api` skill)
- Tauri commands return `Result<T, String>` — use `anyhow` internally, convert with `.map_err(|e| e.to_string())`

## Agent Team (see docs/agent-team.md for full details)

| Role | Agent Type | Main Responsibility |
|------|------------|---------------------|
| lol-architect | system-architect | Module design, API contracts |
| lol-rust-dev | rust-pro | src-tauri/ implementation |
| lol-ts-dev | typescript-pro | src/ + shared/types/ |
| lol-frontend | frontend-developer | UI components, state |
| lol-ux | ui-designer | Overlay UX, 30s champ-select |
| lol-tester | test-automator | Tests, LCU mock |
| lol-reviewer | code-reviewer | Code quality + security |
| lol-perf | performance-engineer | Latency, render time |
| lol-debugger | debugger | LCU/cert/lockfile issues |
| lol-data-eng | data-engineer | SQLite schema, Riot ETL |
| lol-ml | ml-engineer | Recommendation engine (Sprint 3) |
| lol-security | security-auditor | Riot ToS, credential audit |

## Commit Convention

```
feat(lcu): add WebSocket champ-select listener
fix(db): handle migration failure on first launch
perf(ui): reduce champion grid rerender on hover
```

## Performance Target

Champ-select event → recommendation displayed: **< 500ms**
