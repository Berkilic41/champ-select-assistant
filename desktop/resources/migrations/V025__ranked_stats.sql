-- Rank-aware koçluk (Faz D2): LCU'dan keyless çekilen ranked istatistikleri.
-- queue = "soloq" | "flex" (LCU RANKED_SOLO_5x5 / RANKED_FLEX_SR eşlenir).
-- Görüntü bağlamı; öneri motorunun kişisel-medyan baseline'larına KARIŞMAZ
-- (rank-eşiği uydurmak yerine oyuncunun kendi geçmişi referans kalır).

CREATE TABLE IF NOT EXISTS ranked_stats (
    puuid          TEXT    NOT NULL,
    queue          TEXT    NOT NULL,
    tier           TEXT    NOT NULL,
    division       TEXT    NOT NULL,
    league_points  INTEGER NOT NULL DEFAULT 0,
    wins           INTEGER NOT NULL DEFAULT 0,
    losses         INTEGER NOT NULL DEFAULT 0,
    is_provisional INTEGER NOT NULL DEFAULT 0,
    updated_at     INTEGER NOT NULL,
    PRIMARY KEY (puuid, queue)
);
