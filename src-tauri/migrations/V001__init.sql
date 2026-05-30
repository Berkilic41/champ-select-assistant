CREATE TABLE IF NOT EXISTS app_config (
    key   TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS summoners (
    puuid        TEXT NOT NULL PRIMARY KEY,
    game_name    TEXT NOT NULL,
    tag_line     TEXT NOT NULL,
    summoner_id  TEXT,
    region       TEXT NOT NULL,
    cached_at    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_summoners_region ON summoners(region);
