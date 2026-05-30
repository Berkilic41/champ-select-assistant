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
