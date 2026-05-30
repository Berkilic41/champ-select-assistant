CREATE TABLE IF NOT EXISTS builds (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    champion_id   INTEGER NOT NULL,
    position      TEXT    NOT NULL,
    patch_version TEXT    NOT NULL,
    item_ids      TEXT    NOT NULL,
    rune_ids      TEXT    NOT NULL,
    win_rate      REAL    NOT NULL DEFAULT 0.0,
    pick_rate     REAL    NOT NULL DEFAULT 0.0,
    source        TEXT    NOT NULL,
    cached_at     INTEGER NOT NULL,
    FOREIGN KEY (champion_id) REFERENCES champions(champion_id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_builds_unique    ON builds(champion_id, position, patch_version, source);
CREATE INDEX IF NOT EXISTS idx_builds_champ_pos ON builds(champion_id, position);
CREATE INDEX IF NOT EXISTS idx_builds_cached_at ON builds(cached_at DESC);
