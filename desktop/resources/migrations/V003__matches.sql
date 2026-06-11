CREATE TABLE IF NOT EXISTS matches (
    match_id      TEXT    NOT NULL PRIMARY KEY,
    puuid         TEXT    NOT NULL,
    champion_id   INTEGER NOT NULL,
    position      TEXT,
    win           INTEGER NOT NULL CHECK (win IN (0,1)),
    kills         INTEGER NOT NULL DEFAULT 0,
    deaths        INTEGER NOT NULL DEFAULT 0,
    assists       INTEGER NOT NULL DEFAULT 0,
    duration_secs INTEGER NOT NULL,
    queue_id      INTEGER NOT NULL,
    played_at     INTEGER NOT NULL,
    FOREIGN KEY (puuid)       REFERENCES summoners(puuid) ON DELETE CASCADE,
    FOREIGN KEY (champion_id) REFERENCES champions(champion_id) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS idx_matches_puuid       ON matches(puuid);
CREATE INDEX IF NOT EXISTS idx_matches_puuid_champ ON matches(puuid, champion_id);
CREATE INDEX IF NOT EXISTS idx_matches_played_at   ON matches(played_at DESC);
CREATE INDEX IF NOT EXISTS idx_matches_queue       ON matches(queue_id);

CREATE TABLE IF NOT EXISTS mastery (
    puuid          TEXT    NOT NULL,
    champion_id    INTEGER NOT NULL,
    mastery_level  INTEGER NOT NULL,
    mastery_points INTEGER NOT NULL DEFAULT 0,
    last_play_time INTEGER,
    PRIMARY KEY (puuid, champion_id),
    FOREIGN KEY (puuid)       REFERENCES summoners(puuid) ON DELETE CASCADE,
    FOREIGN KEY (champion_id) REFERENCES champions(champion_id) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS idx_mastery_puuid  ON mastery(puuid);
CREATE INDEX IF NOT EXISTS idx_mastery_points ON mastery(puuid, mastery_points DESC);
