# champ-select-assistant — data backend (Cloudflare Worker)

Server-side Riot data service. The Riot API key lives **only** here (Worker
secret), never in the desktop client. Two jobs:

1. **Aggregated stats** (`/v1/rates`) — champion win/pick/ban rates per role,
   computed from a rolling sample of Challenger ranked games (Match-V5),
   ingested on a cron schedule into D1. Replaces the retired Meraki source.
2. **Per-user proxy** (`/proxy/{riot-host}/{path}`) — pass-through to the Riot
   API with the key attached (so the desktop client never holds the key).

> Status: scaffold (Phase 1 = rates). Matchups + builds are planned (see
> `docs/api-key-policy.md` / the project plan). The desktop app keeps working
> without this backend (neutral meta fallback) — wire it up after deploy.

## Endpoints
- `GET /v1/rates?region=tr1[&patch=16.11]` → `{ patch, region, total_games, rates:[{champion_id, role, win_rate, pick_rate, ban_rate, games}] }`
- `GET /v1/ingest` → run one bounded ingestion pass now (cron does this automatically)
- `GET /v1/health` → `{ ok: true }`
- `GET /proxy/{riot-host}/{riot-path}` → Riot API passthrough (allowed hosts only)

## One-time setup
```bash
cd cloudflare-worker
npm install
npx wrangler login

# 1) Create the D1 database, then paste the printed database_id into wrangler.toml
npx wrangler d1 create champ-select-stats

# 2) Apply schema
npx wrangler d1 migrations apply champ-select-stats --remote

# 3) Set the Riot API key (production key once approved; a dev key works for testing)
npx wrangler secret put RIOT_API_KEY

# 4) Deploy
npx wrangler deploy
```

## Local test (no deploy needed)
```bash
npx wrangler d1 migrations apply champ-select-stats --local
# put RIOT_API_KEY in a .dev.vars file (gitignored): RIOT_API_KEY=RGAPI-...
npx wrangler dev
# then:
curl "http://localhost:8787/v1/ingest"                 # pull+aggregate a small sample
curl "http://localhost:8787/v1/rates?region=tr1"       # read back the rates
```

## Tuning (wrangler.toml [vars])
- `INGEST_REGIONS` — platform routing values to sample (e.g. `tr1,euw1,na1,kr`)
- `MAX_MATCHES_PER_RUN` — matches fetched per cron run (keep under your key's rate limit)
- `SEED_PLAYERS_PER_REGION`, `MATCHES_PER_PLAYER` — sample breadth per run
- cron cadence: `[triggers] crons` (default every 2h)

## Next: wire the desktop app
After deploy, set `EDGE_BASE_URL=https://<worker>.workers.dev` in the app's
`.env` — the Electron scheduler's `cloud_edge` source picks it up on the next
tick (no code change; see `desktop/src/main/scheduler.ts` / `sources.ts
syncEdgeRates`). Rates, matchups and builds land in the existing canonical
tables.

Rehearsal status (2026-06-12, Faz A4): `wrangler deploy --dry-run` bundles
cleanly with the D1 binding; the 3 migrations' tables match the code's queries
7/7. Key-day = the 4 commands above + the `.env` flip.
