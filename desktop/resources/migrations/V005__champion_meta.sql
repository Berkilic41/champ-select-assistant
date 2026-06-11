CREATE TABLE IF NOT EXISTS champion_meta (
    champion_id  INTEGER PRIMARY KEY,
    roles        TEXT    NOT NULL DEFAULT '[]',
    is_melee     INTEGER NOT NULL DEFAULT 0,
    attack_range INTEGER NOT NULL DEFAULT 550,
    resource_type TEXT
);
CREATE INDEX IF NOT EXISTS idx_champion_meta_roles ON champion_meta(roles);
