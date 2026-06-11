-- V017: Hash-only Match-V5 discovery crawl state.
--
-- Stores only PUUID hashes. Raw Riot PUUIDs are intentionally not persisted; the
-- runtime keeps a transient hash -> raw PUUID map only for the active sync call.

CREATE TABLE IF NOT EXISTS match_discovery_players (
    puuid_hash         TEXT    PRIMARY KEY,
    region             TEXT    NOT NULL DEFAULT 'unknown',
    source             TEXT    NOT NULL DEFAULT 'match_participant',
    contribution_count INTEGER NOT NULL DEFAULT 0,
    first_seen_at      INTEGER NOT NULL,
    last_seen_at       INTEGER NOT NULL,
    last_crawled_at    INTEGER,
    crawl_count        INTEGER NOT NULL DEFAULT 0,
    updated_at         INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_match_discovery_players_region_seen
    ON match_discovery_players(region, last_seen_at DESC);

CREATE INDEX IF NOT EXISTS idx_match_discovery_players_crawled
    ON match_discovery_players(last_crawled_at DESC);
