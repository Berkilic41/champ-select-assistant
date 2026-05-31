# Riot API Key Policy

## Summary

The Riot API key is **never embedded in the application binary**.

## Development

- Store your key in `src-tauri/.env`:
  ```
  RIOT_API_KEY=RGAPI-your-key-here
  ```
- The `.env` file is in `.gitignore` and must never be committed.
- The app reads the key at runtime via `dotenvy::dotenv()` on startup.

## Production / Public Release (Sprint J2)

The public release will use a backend proxy architecture:

```
App → Cloudflare Worker (or Vercel Edge) → Riot Games API
```

- The Riot API key lives **only** in the proxy's environment secrets.
- The app calls the proxy's public URL instead of Riot directly.
- Rate limiting and response caching are handled by the proxy.
- The key is rotatable without a new app release.

### Proxy Contract

```
GET /summoner/{puuid}
GET /matches/{puuid}?count=20&type=ranked
GET /mastery/{puuid}
```

All responses are JSON-passthrough from Riot API, optionally cached (TTL: 5 min for summoner, 1 hour for matches).

## Security Checklist

- [ ] `.env` in `.gitignore` ✅
- [ ] `RIOT_API_KEY` not in `tauri.conf.json`
- [ ] `RIOT_API_KEY` not in any `.github/workflows/` (use secrets only)
- [ ] Binary scan: `strings target/release/*.exe | grep RGAPI` returns empty
- [ ] Public release: proxy deployed, app points to proxy URL

## Riot Developer Portal Registration (Public Beta Blocker)

Riot's third-party policy (`developer.riotgames.com/policies/general`) requires
products to be **registered in and audited through the Developer Portal**.

- **Closed beta (CB-1):** LCU-first flow needs no developer key; personal key is
  optional for the fallback path. Registration not strictly required to test.
- **Public release (BLOCKER):** Before public distribution, register the product
  on `developer.riotgames.com`, apply for a **production API key**, and complete
  Riot's audit. Until approved, keep the official-API path optional and rely on
  the LCU-first flow + DDragon/CDragon (officially supported static data).

**Status:** ⏳ Not yet registered — tracked as a public-beta blocker.

### LCU usage note (compliance)

The League Client (LCU) API is **unofficial/undocumented** — Riot does not list it
as a "supported service." This app uses it read-only plus a single **user-initiated
hover** action (`hover_champion`, `commands/champ_select.rs`) that never completes
(locks) a pick. This is the same tolerated category as Blitz / op.gg / Mobalytics.
No game process/memory is accessed (Vanguard-safe). Because LCU is unofficial, no
LCU-based tool can claim "officially fully compliant"; this product follows all
published rules (recommend-only, disclaimer, no key in binary, HTTPS, no telemetry).
