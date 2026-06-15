-- Sprint 2B — match-outcome label (ML eğitim etiketi).
-- Bir champ-select'te ÖNERİ gösterildiyse ve oyuncu bir şampiyon KİLİTLEDİYSE
-- pending bir satır yazılır (win NULL). Oyun bitip o maç `matches` tablosuna
-- ingest olunca win/loss + match_id ile çözülür (resolved_at dolar).
--   resolved_at IS NULL = bekleyen etiket (henüz sonuç yok).
--   rec_rank/rec_score   = pick'in öneri listesindeki yeri; NULL = liste-dışı pick.
-- Bu satırlar YERELDE kalır — hiçbir veri sunucuya gitmez (rıza gerektirmez).
-- FK YOK (log-tarzı dayanıklılık): summoner/champion satırı henüz yokken de yazılabilir.
CREATE TABLE IF NOT EXISTS recommendation_outcomes (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    puuid         TEXT    NOT NULL,
    champion_id   INTEGER NOT NULL,
    champion_key  TEXT    NOT NULL,
    queue_id      INTEGER NOT NULL,
    position      TEXT,
    rec_rank      INTEGER,
    rec_score     REAL,
    model_version TEXT    NOT NULL DEFAULT 'unknown',
    context_json  TEXT    NOT NULL DEFAULT '{}',
    match_id      TEXT,
    win           INTEGER CHECK (win IN (0, 1)),
    created_at    INTEGER NOT NULL,
    resolved_at   INTEGER
);
CREATE INDEX IF NOT EXISTS idx_rec_outcomes_pending
    ON recommendation_outcomes (resolved_at, created_at);
CREATE INDEX IF NOT EXISTS idx_rec_outcomes_link
    ON recommendation_outcomes (puuid, champion_id, queue_id);
