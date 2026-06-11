-- Time-series mastery snapshots for progress tracking (training mode). Mastery points
-- only ever increase, so points gained in a window = MAX(points) - MIN(points). A new
-- row is written only when the points changed since the last snapshot (see repo).
CREATE TABLE IF NOT EXISTS mastery_snapshots (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    puuid          TEXT    NOT NULL,
    champion_id    INTEGER NOT NULL,
    mastery_points INTEGER NOT NULL,
    mastery_level  INTEGER NOT NULL,
    snapshot_at    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_mastery_snapshots
    ON mastery_snapshots (puuid, champion_id, snapshot_at);
