-- V016: Match-V5 fetch history.
--
-- Prevents duplicate Match-V5 detail fetches and keeps parse/process state for
-- the runtime batch ingester. A row is keyed by Riot match id; failed rows can be
-- retried by the pure fetch planner, while processed rows are deduped.

CREATE TABLE IF NOT EXISTS match_v5_fetch_history (
    match_id     TEXT    PRIMARY KEY,
    region       TEXT    NOT NULL DEFAULT 'unknown',
    patch        TEXT    NOT NULL DEFAULT 'unknown',
    queue_id     INTEGER NOT NULL DEFAULT 0,
    status       TEXT    NOT NULL DEFAULT 'discovered',
    discovered_at INTEGER NOT NULL,
    fetched_at   INTEGER,
    processed_at INTEGER,
    error        TEXT,
    updated_at   INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_match_v5_fetch_history_status_updated
    ON match_v5_fetch_history(status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_match_v5_fetch_history_region_patch
    ON match_v5_fetch_history(region, patch);
