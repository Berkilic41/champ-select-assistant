-- Aggregated champion statistics, computed from sampled Riot Match-V5 games.
-- One row per (patch, region, champion, role). win/pick/ban rates are derived
-- at read time from these counters + ingest_meta.total_games.
CREATE TABLE IF NOT EXISTS champion_rates (
    patch       TEXT    NOT NULL,
    region      TEXT    NOT NULL,
    champion_id INTEGER NOT NULL,
    role        TEXT    NOT NULL,           -- top | jungle | middle | bottom | utility
    games       INTEGER NOT NULL DEFAULT 0, -- games this champ was played in this role
    wins        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (patch, region, champion_id, role)
);
CREATE INDEX IF NOT EXISTS idx_champion_rates_pr ON champion_rates (patch, region);

-- Bans are not role-specific, so they live per champion (per patch/region).
-- ban_rate = bans / ingest_meta.total_games.
CREATE TABLE IF NOT EXISTS champion_bans (
    patch       TEXT    NOT NULL,
    region      TEXT    NOT NULL,
    champion_id INTEGER NOT NULL,
    bans        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (patch, region, champion_id)
);

-- Denominator + freshness per (patch, region): total games sampled, used for
-- pick_rate (picks/total) and ban_rate (bans/total).
CREATE TABLE IF NOT EXISTS ingest_meta (
    patch       TEXT    NOT NULL,
    region      TEXT    NOT NULL,
    total_games INTEGER NOT NULL DEFAULT 0,
    updated_at  INTEGER NOT NULL DEFAULT 0, -- epoch ms
    PRIMARY KEY (patch, region)
);

-- Dedupe: match ids already aggregated, so re-runs don't double count.
-- Pruned by processed_at to bound table size.
CREATE TABLE IF NOT EXISTS processed_matches (
    match_id     TEXT    NOT NULL PRIMARY KEY,
    processed_at INTEGER NOT NULL            -- epoch ms
);
CREATE INDEX IF NOT EXISTS idx_processed_at ON processed_matches (processed_at);
