-- V011: Pro-tier build fields.
-- Adds skill order, summoner spells, secondary rune tree path, and stat shards
-- to the builds table so the UI can show a complete coach-grade build, not just
-- the 4-item + keystone shorthand.
--
-- All columns are nullable — existing rows continue to work; UI sections render
-- only when data is present (graceful degradation).
--
-- Column semantics:
-- - skill_order:     Display text like "Q→W→E" (max priority left→right)
-- - summoner_spells: JSON [spell1_id, spell2_id] (e.g. [4, 12] = Flash + Teleport)
-- - secondary_runes: JSON [tree_id, rune1_id, rune2_id] (3 entries: tree + 2 picks)
-- - stat_shards:     JSON [offense_id, flex_id, defense_id] (3 stat shard IDs)

ALTER TABLE builds ADD COLUMN skill_order TEXT;
ALTER TABLE builds ADD COLUMN summoner_spells TEXT;
ALTER TABLE builds ADD COLUMN secondary_runes TEXT;
ALTER TABLE builds ADD COLUMN stat_shards TEXT;
