-- Per-match total creep score (lane minions + neutral monsters) for CS/min analysis.
-- Nullable: matches synced before this migration keep NULL until they are re-synced.
ALTER TABLE matches ADD COLUMN cs INTEGER;
