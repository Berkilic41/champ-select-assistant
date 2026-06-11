# Competitive Benchmark - Champ Select Assistant vs iTero

> Date: 2026-06-02  
> Goal: keep the "better than iTero" claim measurable, not vibes-based.

## Current Baseline

| Area | iTero | Champ Select Assistant | Target |
| --- | --- | --- | --- |
| Installed app | The iTero Coach 3.6.5 | 0.9.0 beta | CSA wins on speed, privacy, explainability |
| Desktop stack | Electron / Chromium bundle | Tauri 2 + Rust + React | Keep Tauri; do not move to Electron |
| Main executable / installer | `The iTero Coach.exe` ~183.14 MB | MSI ~5.44 MB, NSIS setup ~4.21 MB | Keep public installer under 25 MB |
| App payload | `resources/` ~208.98 MB, `app.asar` ~203.61 MB | `dist/` ~1.73 MB | Keep frontend payload under 5 MB |
| Public traction | Overwolf listing: 4.4 rating, ~546K downloads | Beta/local project | Build trust with benchmarks + signed releases |
| Draft | Real-time champion/build recommendations | Scoring + Draft IQ + game plan | Add trust badges, richer explanations, feedback loop |
| Macro coach | Post-game and macro analysis | Team game plan only | Add post-game coach v1 |
| Overlay | Timers, skill, gold, damage, ultimate timers | Always-on-top app window | Add safe hybrid overlay, no injection |
| Data | Large account analyser, external datasets | 172 champions, 109 combos, 80 matchup seed, 31 build seed | 1000+ matchups, 172+ primary-role builds |
| Privacy | Overwolf/client dependency, public policy outside this repo | Local-first, no telemetry | Make privacy a product differentiator |

## Feature Matrix

| Capability | iTero | CSA Now | CSA Next |
| --- | --- | --- | --- |
| Real-time pick recommendations | Yes | Yes | Add data-source trust per recommendation |
| Enemy-comp aware builds | Yes | Partial | Expand seed + source-aware resolver |
| Rune/summoner advice | Yes | Partial | Add confidence + source badges |
| Advanced lobby scouting | Yes | Partial enemy pool/ban hints | Add playstyle and pool confidence |
| Ban advisor | Yes/likely | Yes | Add denial value and source confidence |
| Macro game plan | Yes | Yes, draft-level | Add post-game "next game focus" |
| In-game overlay | Yes | No dedicated overlay | Hybrid overlay v1 |
| Map/objective timers | Yes | No | Overlay v2 |
| Damage/gold tracker | Yes | No | Overlay v2 if safe/compliant |
| Account analyser | Yes, 500+ stats | Local stats limited | Summarize fewer stats into better coaching actions |
| Explainability | Medium/publicly opaque | High potential | Make every score auditable |
| Turkish UX | Generic/global | Strong | Keep Turkish coach voice as wedge |
| Local-first privacy | Not the wedge | Strong | Keep telemetry-free default |

## Hard Targets

| Metric | Current | 10/10 Target |
| --- | --- | --- |
| Recommendation latency | Logged, target currently 500 ms | p95 < 300 ms, worst-case < 500 ms |
| Cold start | Not measured in this doc | < 2.5 s |
| Idle RAM | Not measured in this doc | < 180 MB |
| Champ-select RAM | Not measured in this doc | < 250 MB |
| Champion coverage | 172 | 172 maintained every patch |
| Combo coverage | 109 | 500 curated/pro-coach synergies |
| Matchup coverage | 80 seed | 1000+ exact matchup rows |
| Build coverage | 31 seed | 172 primary-role builds, then multi-role |
| Meta role coverage | Depends on sync | > 95% after meta sync |
| User feedback hit-rate | Not implemented | > 85% "mantikli oneri" feedback |

## Implementation Guardrails

- Do not reverse-engineer iTero proprietary app code; compare public features and measurable local package facts only.
- Keep Tauri/Rust as the native advantage.
- Hybrid overlay must not inject into the game, read game memory, or automate lock/ban/pick.
- Every aggressive data source must have rate limit, cache, source label, confidence, and graceful fallback.
- Every recommendation should answer: why this pick, what can go wrong, which data backed it, and what to do in lane/teamfight.

## Sprint Review Checklist

Run this after every major sprint:

```powershell
pnpm typecheck
pnpm test:run
cd src-tauri
cargo test --all
cargo clippy --all-features -- -D warnings
cargo fmt --all -- --check
```

Then update:

- Installer size.
- Frontend payload size.
- Recommendation p95/worst latency.
- Data quality report (`get_data_quality_report`).
- Feature matrix row status.
- One paragraph: "what now beats iTero, what still does not."
