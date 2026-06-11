-- V014: Runtime ingestion parity fields.
--
-- Match-V5 canonical rows carry region/confidence/sample-size/build-games data.
-- Keep this additive so existing recommendation queries and historical local
-- rows continue to work. The local client remains an active-region snapshot:
-- existing unique constraints are intentionally preserved, so a later refresh
-- for another region updates the same source row and records the latest region.

ALTER TABLE champion_rates ADD COLUMN region TEXT NOT NULL DEFAULT 'global';

ALTER TABLE champion_matchups ADD COLUMN region TEXT NOT NULL DEFAULT 'global';
ALTER TABLE champion_matchups ADD COLUMN confidence TEXT NOT NULL DEFAULT 'low';
ALTER TABLE champion_matchups ADD COLUMN sample_size INTEGER NOT NULL DEFAULT 0;

ALTER TABLE builds ADD COLUMN region TEXT NOT NULL DEFAULT 'global';
ALTER TABLE builds ADD COLUMN games INTEGER NOT NULL DEFAULT 0;
ALTER TABLE builds ADD COLUMN confidence TEXT NOT NULL DEFAULT 'low';

CREATE INDEX IF NOT EXISTS idx_champion_rates_region
    ON champion_rates(region, champion_id, position);

CREATE INDEX IF NOT EXISTS idx_matchups_region
    ON champion_matchups(region, champion_id, opponent_id, position);

CREATE INDEX IF NOT EXISTS idx_builds_region
    ON builds(region, champion_id, position);
