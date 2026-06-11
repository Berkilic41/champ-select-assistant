-- Idempotent feedback ingestion: a stable client-supplied dedup key so a re-sent
-- row (retry after a half-failed flush, double-tap) is a no-op instead of a
-- duplicate. NULL keys stay distinct, so legacy rows / clients are unaffected.
ALTER TABLE recommendation_feedback ADD COLUMN IF NOT EXISTS idempotency_key TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS uq_feedback_idempotency_key
    ON recommendation_feedback (idempotency_key);
