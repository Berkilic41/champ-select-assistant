-- V015: Local data-pipeline fetch log.
--
-- Used by the runtime scheduler policy and the data-quality surface to answer:
-- what refreshed, when, with which decision/status, and why.

CREATE TABLE IF NOT EXISTS source_fetch_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    source      TEXT    NOT NULL,
    status      TEXT    NOT NULL,
    decision    TEXT    NOT NULL DEFAULT 'refresh',
    message     TEXT,
    started_at  INTEGER NOT NULL,
    finished_at INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_source_fetch_log_source_finished
    ON source_fetch_log(source, finished_at DESC);

CREATE INDEX IF NOT EXISTS idx_source_fetch_log_finished
    ON source_fetch_log(finished_at DESC);
