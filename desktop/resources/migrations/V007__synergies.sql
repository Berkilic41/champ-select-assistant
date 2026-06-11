CREATE TABLE IF NOT EXISTS champion_synergies (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    champion_id   INTEGER NOT NULL,
    ally_id       INTEGER NOT NULL,
    games         INTEGER NOT NULL DEFAULT 0,
    win_rate      REAL    NOT NULL DEFAULT 0.5,
    source        TEXT    NOT NULL DEFAULT 'local',
    patch_version TEXT    NOT NULL DEFAULT 'unknown',
    cached_at     INTEGER NOT NULL,
    UNIQUE(champion_id, ally_id, source)
);
