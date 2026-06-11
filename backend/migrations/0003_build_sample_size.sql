-- Store build sample size as top-level metadata.
-- Region/confidence already exist on champion_builds; item/rune/spell details
-- remain in payload JSONB.

ALTER TABLE champion_builds
    ADD COLUMN IF NOT EXISTS sample_size INTEGER NOT NULL DEFAULT 0;
