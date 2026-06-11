CREATE TABLE IF NOT EXISTS recommendation_feedback (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    champion_id       INTEGER NOT NULL,
    champion_key      TEXT    NOT NULL,
    feedback          TEXT    NOT NULL,
    session_hash      TEXT,
    model_version     TEXT    NOT NULL DEFAULT 'unknown',
    score             REAL    NOT NULL DEFAULT 0.0,
    payload_json      TEXT    NOT NULL DEFAULT '{}',
    synced_at         INTEGER,
    created_at        INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_recommendation_feedback_sync
    ON recommendation_feedback (synced_at, created_at);

CREATE TABLE IF NOT EXISTS draft_brain_packs (
    kind         TEXT    NOT NULL PRIMARY KEY,
    version      TEXT    NOT NULL,
    payload_json TEXT    NOT NULL,
    source       TEXT    NOT NULL,
    fetched_at   INTEGER NOT NULL,
    expires_at   INTEGER
);
