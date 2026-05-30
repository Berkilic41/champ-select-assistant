CREATE TABLE IF NOT EXISTS ddragon_cache (
    version       TEXT NOT NULL PRIMARY KEY,
    base_path     TEXT NOT NULL,
    downloaded_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS champions (
    champion_id INTEGER NOT NULL PRIMARY KEY,
    key         TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    title       TEXT NOT NULL,
    cached_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_champions_key ON champions(key);
