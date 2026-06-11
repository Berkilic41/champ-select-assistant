-- V010: Drop the V004 4-column unique index that didn't include opponent_archetype.
-- V009 already created idx_builds_matchup which covers the same columns + COALESCE(opponent_archetype,''),
-- so the old index would cause unique-constraint failures for matchup variants.
DROP INDEX IF EXISTS idx_builds_unique;
