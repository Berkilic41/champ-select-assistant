CREATE TABLE IF NOT EXISTS champion_matchups (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    champion_id   INTEGER NOT NULL,
    opponent_id   INTEGER NOT NULL,
    position      TEXT    NOT NULL,
    games         INTEGER NOT NULL DEFAULT 0,
    wins          INTEGER NOT NULL DEFAULT 0,
    win_rate      REAL    NOT NULL DEFAULT 0.5,
    source        TEXT    NOT NULL DEFAULT 'local',
    patch_version TEXT    NOT NULL DEFAULT 'unknown',
    cached_at     INTEGER NOT NULL,
    UNIQUE(champion_id, opponent_id, position, source)
);
CREATE INDEX IF NOT EXISTS idx_matchups_champ ON champion_matchups(champion_id, position);
