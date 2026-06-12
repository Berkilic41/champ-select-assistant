-- Koç döngüsü (Faz C1+C2): maç sonu karneleri + odak hedefleri.
-- review_json = core GameReview çıktısı olduğu gibi (tek kaynak: motor);
-- focus_goals.result NULL = açık hedef, 'met'/'missed'/'no_data' = kapandı,
-- 'superseded' = kontrol edilemeden yenisiyle değiştirildi.

CREATE TABLE IF NOT EXISTS game_reviews (
    match_id    TEXT    NOT NULL PRIMARY KEY,
    puuid       TEXT    NOT NULL,
    queue_group TEXT    NOT NULL,
    created_at  INTEGER NOT NULL,
    review_json TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_game_reviews_puuid
    ON game_reviews(puuid, created_at DESC);

CREATE TABLE IF NOT EXISTS focus_goals (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    puuid            TEXT    NOT NULL,
    queue_group      TEXT    NOT NULL,
    metric           TEXT    NOT NULL,
    target           REAL    NOT NULL,
    direction        TEXT    NOT NULL,
    label            TEXT    NOT NULL,
    created_at       INTEGER NOT NULL,
    source_match_id  TEXT,
    checked_match_id TEXT,
    result           TEXT
);

CREATE INDEX IF NOT EXISTS idx_focus_goals_open
    ON focus_goals(puuid, queue_group, result);
