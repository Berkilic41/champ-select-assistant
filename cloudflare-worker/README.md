# champ-select-assistant — data backend (Cloudflare Worker)

Server-side Riot data service. The Riot API key lives **only** here (Worker
secret), never in the desktop client. It aggregates champion win/pick/ban
rates, lane matchups and builds per role, computed from a rolling sample of
Challenger ranked games (Match-V5), ingested on a cron schedule into D1.
Replaces the retired Meraki source.

> A per-user Riot passthrough (`/proxy/*`) was **removed** — it would have
> exposed the personal/dev key to anonymous abuse. The app is LCU-first; an
> auth-gated proxy may return only after a production key is approved.

> Status: rates + matchups + builds are live read endpoints. The desktop app
> keeps working without this backend (neutral meta fallback) — wire it up after
> deploy. See `docs/api-key-policy.md` / the project plan.

## Endpoints (all GET; non-GET → 405)
- `/v1/rates?region=tr1[&patch=16.11]` → `{ patch, region, total_games, rates:[{champion_id, role, win_rate, pick_rate, ban_rate, games}] }`
- `/v1/matchups?region=tr1[&patch=16.11]` → `{ patch, region, matchups:[{champion_id, opponent_id, role, games, wins, win_rate}] }`
- `/v1/builds?region=tr1[&patch=16.11]` → `{ patch, region, builds:[{champion_id, role, item_ids, rune_ids, summoner_spells, games, wins, win_rate}] }`
- `/v1/ingest` → run one bounded ingestion pass now. **Auth-gated**: requires
  `Authorization: Bearer <INGEST_SECRET>` (the cron runs it without HTTP). Returns 401 otherwise.
- `/v1/health` → `{ ok: true }`

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

# 3b) Set the manual-ingest secret (any strong string) — gates POST-less /v1/ingest
npx wrangler secret put INGEST_SECRET

# 4) Deploy
npx wrangler deploy
```

## Local test (no deploy needed)
```bash
npx wrangler d1 migrations apply champ-select-stats --local
# put secrets in a .dev.vars file (gitignored):
#   RIOT_API_KEY=RGAPI-...
#   INGEST_SECRET=<any-strong-string>   # gates the manual /v1/ingest trigger
npx wrangler dev
# then (ingest is Bearer-gated — the cron runs it without auth):
curl -H "Authorization: Bearer <INGEST_SECRET>" "http://localhost:8787/v1/ingest"
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
