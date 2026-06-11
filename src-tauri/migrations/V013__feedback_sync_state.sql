-- Feedback sync productionization (Sprint E): per-row retry bookkeeping so the
-- offline flush can back off + give up safely without losing the queue.
--   retry_count    - failed flush attempts so far (drives backoff)
--   last_error     - last flush error message (for the 'failed' UI sub-state)
--   next_retry_at  - unix seconds; row is "due" when NULL or <= now
-- A row is pending while synced_at IS NULL; setting synced_at marks it synced.
ALTER TABLE recommendation_feedback ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE recommendation_feedback ADD COLUMN last_error TEXT;
ALTER TABLE recommendation_feedback ADD COLUMN next_retry_at INTEGER;
