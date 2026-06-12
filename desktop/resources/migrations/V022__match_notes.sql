-- Maç notları (Faz C5): kullanıcının maç başına serbest notu + etiket çipleri.
-- tags = JSON dizi (örn. ["tilt","wave"]). Tek yeniden-üretilemez kullanıcı
-- verilerinden biri (feedback/hedeflerle birlikte).

CREATE TABLE IF NOT EXISTS match_notes (
    match_id   TEXT    NOT NULL PRIMARY KEY,
    puuid      TEXT    NOT NULL,
    note       TEXT    NOT NULL DEFAULT '',
    tags       TEXT    NOT NULL DEFAULT '[]',
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_match_notes_puuid ON match_notes(puuid, updated_at DESC);
