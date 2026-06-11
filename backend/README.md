# Draft Brain Backend

Rust + Postgres service for cloud-first DraftBrain learning.

## Run Locally

```powershell
cd backend
$env:DATABASE_URL="postgres://postgres:postgres@localhost:5432/draft_brain"
sqlx migrate run
cargo run
```

Default bind address is `127.0.0.1:8080`. Override with `BIND_ADDR`.

## Endpoints

- `GET /v1/health`
- `GET /v1/model-pack/latest`
- `GET /v1/data-pack/latest?patch=&region=`
- `POST /v1/draft-samples`
- `POST /v1/recommendation-feedback`
- `POST /v1/match-outcomes`
- `GET /v1/data-quality`

The desktop client must never block active champ-select on this service. It should
cache model/data packs and submit feedback asynchronously.
