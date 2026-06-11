-- Lane matchups, computed from sampled Riot Match-V5 games (Phase 2).
-- One row per (patch, region, champion, opponent, role); BOTH directions are
-- stored (A vs B and B vs A) so reads need no mirroring. wins counts the
-- champion_id side's wins; win_rate = wins / games at read time.
CREATE TABLE IF NOT EXISTS champion_matchups (
    patch       TEXT    NOT NULL,
    region      TEXT    NOT NULL,
    champion_id INTEGER NOT NULL,
    opponent_id INTEGER NOT NULL,
    role        TEXT    NOT NULL,           -- top | jungle | middle | bottom | utility
    games       INTEGER NOT NULL DEFAULT 0,
    wins        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (patch, region, champion_id, opponent_id, role)
);
CREATE INDEX IF NOT EXISTS idx_champion_matchups_pr ON champion_matchups (patch, region);
