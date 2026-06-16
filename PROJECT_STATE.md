# PROJECT_STATE — champ-select-assistant

> Otonom geliştirme döngüsünün "mevcut durum" kanonu. Her iterasyon sonunda
> değişen kısımlar güncellenir. Son güncelleme: 2026-06-16.

## Ne yapar
League of Legends **champ-select** sırasında (ve oyun-içi) deterministik bir
motorla şampiyon önerisi, ban önerisi, build/rün, counter-pick ve counter-item
koçluğu veren masaüstü uygulaması. Veri kaynakları çevrimdışı bilgi tabanı +
canlı meta (u.gg, Leaguepedia, Cloudflare edge worker, Riot Match-V5, LCU).

## Stack & katmanlar
| Katman | Yer | Teknoloji |
|---|---|---|
| **core** | `core/` | Rust → WASM (`wasm-pack --target nodejs`). Deterministik motor: scoring, draft_brain, pipeline policy. **Test-oracle.** |
| **desktop host** | `desktop/src/main/` | Electron 40 + Node. DB (node:sqlite), LCU, IPC, scheduler, kaynak fetch'leri, WASM `Engine` sarmalı. |
| **renderer** | `src/` | React 19 + TS 5 + Vite 7. UI, hooks, i18n (tr). |
| **worker** | `cloudflare-worker/` | Cloudflare Worker (wrangler) — Riot Match-V5 ingestion → D1 → `/v1/{rates,matchups,builds}` anon-read aggregate. |
| **shared types** | `src/types/generated/` | ts-rs ile Rust'tan üretilen TS tipleri. |

**Paket yöneticisi:** pnpm 11 workspace (`desktop`, `cloudflare-worker`; renderer = kök paket).

## Önemli sınırlar / akışlar
- **IPC:** renderer `window.api.invoke(name,args)` → preload → tek `"cmd"` dispatcher (`desktop/src/main/ipc.ts`) → `buildCommandRegistry()` Map. Ayrı kanallar: `core:smoke`, `app:status`.
- **Boot:** `index.ts` → DB+migrations → `Engine.load()` (core.wasm) → `PipelineScheduler.start()` (30s gecikme, 3dk tick) → IPC + pencere.
- **Veri pipeline:** scheduler tick → `pipelineRefreshPlan` (core) → kaynaklar (ddragon/u_gg/leaguepedia/cloud_edge/match_v5; meraki disabled) → fetch-log + pack promotion. **Seed import (`importBuildsSeed`/`importMatchupsSeed`) yalnız manuel `sync_data_pipeline`'da** (Settings butonu) — scheduler seed import etmez.
- **DDragon:** renderer `src/lib/ddragon.ts` ikon/item/rün URL'leri (`getDdragonVersion` fallback `14.10.1`; `"unknown"` sentinel reddedilir — beta.6 fix).

## Çalıştırma & test (özet — tam liste: QUALITY_CHECKS.md)
- Renderer: `pnpm dev` · `pnpm test:run` · `pnpm typecheck`
- Desktop: `pnpm --filter csa-desktop dev|test|test:e2e|typecheck`
- Core: `cargo test --all` · `cargo clippy --all-targets --all-features -- -D warnings` (cwd `core/`)
- Worker: `pnpm --filter champ-select-riot-proxy test|typecheck`
- **CI** (`.github/workflows/ci.yml`): 4 job — rust (fmt+clippy+test), frontend (typecheck+test:run), desktop (wasm+typecheck+test+e2e xvfb), worker (typecheck+test).

## Bilinen riskler / açık alanlar
- **Cold-start:** ilk açılışta `champion_rates` boş → meta_score düz 0.3; builds/matchups seed'leri otomatik yüklenmiyor (yalnız manuel). Bkz. BACKLOG B-02.
- **Dev-key 24h:** Riot dev key ~24 saatte expire; worker stale/boş dönerse app sessizce "taze" harmanlıyor (patch-yaş kontrolü yok). Bkz. B-03.
- **İlk-açılış/paketli kırılganlık:** sentinel/empty değer + onError-eksik görseller (icon-bug sınıfı). Bkz. B-01/B-04/B-05.
- **Doküman borcu:** `.claude/CLAUDE.md` hâlâ Tauri'yi anlatıyor (proje Electron'a göçtü). Bkz. B-06.
- **Installer imzasız** (SmartScreen uyarısı) — harici sertifika gerektirir, kapsam dışı.

## Sürüm
desktop `package.json`: `0.10.0-beta.6`. Landing: GitHub Pages (`docs/`).
