-- V009: Matchup-aware build variants.
-- Adds optional `opponent_archetype` to the builds table so a champion can have
-- a different item path depending on the enemy lane archetype (e.g. Darius into
-- Fiora → executioner rush; Zed into tank → lethality-first).
--
-- NULL opponent_archetype = the default build used when no specific matchup exists.
-- Queries should fall back: (champion, position, opponent_archetype) → (champion, position, NULL).

ALTER TABLE builds ADD COLUMN opponent_archetype TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_builds_matchup
    ON builds(champion_id, position, patch_version, source, COALESCE(opponent_archetype, ''));

CREATE INDEX IF NOT EXISTS idx_builds_matchup_lookup
    ON builds(champion_id, position, opponent_archetype);
