-- Build aggregation from sampled Riot Match-V5 games (Phase 3).
-- Full item sets are too high-cardinality for small samples, so items are
-- counted per item (top-6 by games composes the served build) while rune
-- pages + spells are counted as whole combos (low cardinality per champ/role).

CREATE TABLE IF NOT EXISTS champion_build_items (
    patch       TEXT    NOT NULL,
    region      TEXT    NOT NULL,
    champion_id INTEGER NOT NULL,
    role        TEXT    NOT NULL,           -- top | jungle | middle | bottom | utility
    item_id     INTEGER NOT NULL,           -- final slots 0-5 (trinket excluded)
    games       INTEGER NOT NULL DEFAULT 0,
    wins        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (patch, region, champion_id, role, item_id)
);
CREATE INDEX IF NOT EXISTS idx_build_items_pr ON champion_build_items (patch, region);

-- One row per distinct (rune page, summoner spells) combo. rune_ids is a JSON
-- array [primaryStyle, keystone, p2, p3, p4, subStyle, s1, s2]; spells is a
-- JSON array [summoner1Id, summoner2Id] (slot order preserved).
CREATE TABLE IF NOT EXISTS champion_build_pages (
    patch       TEXT    NOT NULL,
    region      TEXT    NOT NULL,
    champion_id INTEGER NOT NULL,
    role        TEXT    NOT NULL,
    rune_ids    TEXT    NOT NULL,
    spells      TEXT    NOT NULL,
    games       INTEGER NOT NULL DEFAULT 0,
    wins        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (patch, region, champion_id, role, rune_ids, spells)
);
CREATE INDEX IF NOT EXISTS idx_build_pages_pr ON champion_build_pages (patch, region);
