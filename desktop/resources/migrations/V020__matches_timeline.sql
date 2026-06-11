-- Timeline-derived per-match stats (WS4 deep post-game). All nullable: only the
-- Riot sync path fills them (LCU match history has no timeline), and matches
-- synced before this migration stay NULL until re-synced — the report skips
-- NULLs honestly instead of guessing.
ALTER TABLE matches ADD COLUMN cs_at_10 INTEGER;      -- CS at 10:00 (laning benchmark)
ALTER TABLE matches ADD COLUMN deaths_pre_14 INTEGER; -- deaths before 14:00 (laning phase)
ALTER TABLE matches ADD COLUMN vision_score INTEGER;  -- Match-V5 participant.visionScore
