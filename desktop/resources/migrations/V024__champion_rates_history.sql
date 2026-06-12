-- Meta trend tarihçesi (Faz D4): champion_rates upsert'i üstüne yazdığından
-- (ON CONFLICT DO UPDATE) delta için periyodik snapshot gerekir. Scheduler,
-- başarılı u_gg sync'i sonrası ≥6 saat aralıkla anlık görüntü alır; 14 günden
-- eski satırlar budanır. Delta = güncel u_gg satırı − ≥6 saat önceki snapshot.

CREATE TABLE IF NOT EXISTS champion_rates_history (
    champion_id INTEGER NOT NULL,
    position    TEXT    NOT NULL,
    source      TEXT    NOT NULL,
    win_rate    REAL    NOT NULL,
    sample_size INTEGER NOT NULL,
    patch       TEXT    NOT NULL,
    recorded_at INTEGER NOT NULL,
    PRIMARY KEY (champion_id, position, source, recorded_at)
);

CREATE INDEX IF NOT EXISTS idx_rates_history_lookup
    ON champion_rates_history(champion_id, position, source, recorded_at DESC);
