# QUALITY_CHECKS — kalite kapıları

> Her iterasyonda DEĞİŞEN katmanın kapıları çalıştırılır; tümünü her seferinde
> koşmak şart değil. Komutlar repo kökünden çalışır (aksi belirtilmedikçe).

## Komutlar (katman bazında)

### Renderer (`src/`) — kök paket
- Typecheck: `pnpm typecheck`
- Unit test: `pnpm test:run`
- Build (gerekirse): `pnpm build`

### Desktop host (`desktop/`)
- Typecheck: `pnpm --filter csa-desktop typecheck`
- Unit test: `pnpm --filter csa-desktop test`
- E2E app-launch smoke: `pnpm --filter csa-desktop test:e2e` (Linux'ta `xvfb-run -a …`)
- WASM build (test/e2e öncesi gerekebilir): `pnpm --filter csa-desktop build:wasm`

### Core (`core/`) — cwd `core/`
- Test: `cargo test --all`
- Lint: `cargo clippy --all-targets --all-features -- -D warnings`
- Format: `cargo fmt --all -- --check` (CI fmt'i zorunlu tutuyor)

### Worker (`cloudflare-worker/`)
- Typecheck: `pnpm --filter champ-select-riot-proxy typecheck`
- Unit test: `pnpm --filter champ-select-riot-proxy test`

## Değişiklik → çalıştırılacak kapılar
| Dokunulan | Kapılar |
|---|---|
| `src/**` (renderer) | `pnpm typecheck` + `pnpm test:run` |
| `desktop/**` | `--filter csa-desktop typecheck` + `test` (gerekirse `build:wasm`, `test:e2e`) |
| `core/**` (Rust) | `cargo fmt --check` + `cargo clippy …` + `cargo test --all` |
| `cloudflare-worker/**` | worker `typecheck` + `test` |

## Manuel smoke (kullanıcı tarafı, opsiyonel)
- `docs/live-smoke-checklist.md` — gerçek champ-select→maç→post-game turu; `[pipeline]` log satırları her aşamayı doğrular.

## Değişmez kurallar
- i18n paritesi: `src/i18n/tr.json` anahtarları eklenir/silinirken UI ile tutarlı kalmalı.
- Deterministik motor test-oracle'dır; ML/LLM yalnız fallback'le augment eder.
- **Commit/push YOK** (kullanıcı açıkça istemeden). Kapılar yeşil olsa bile commit edilmez.
- Kapı kırmızıysa: kök-neden → güvenli düzelt → tekrar. Çözülemezse TASKS.md'ye açıkça yaz.
