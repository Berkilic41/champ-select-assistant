CREATE TABLE IF NOT EXISTS champion_rates (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    champion_id   INTEGER NOT NULL,
    position      TEXT    NOT NULL,
    win_rate      REAL    NOT NULL,
    pick_rate     REAL    NOT NULL DEFAULT 0.0,
    ban_rate      REAL    NOT NULL DEFAULT 0.0,
    sample_size   INTEGER NOT NULL DEFAULT 0,
    patch         TEXT    NOT NULL DEFAULT 'unknown',
    source        TEXT    NOT NULL DEFAULT 'meraki',
    confidence    TEXT    NOT NULL DEFAULT 'low',
    cached_at     INTEGER NOT NULL,
    UNIQUE(champion_id, position, source)
);
CREATE INDEX IF NOT EXISTS idx_champion_rates_lookup
    ON champion_rates (champion_id, position);
