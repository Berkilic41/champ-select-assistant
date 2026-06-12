-- Veto & tercihler (Faz D3): kullanıcının şampiyon-bazlı kararları.
-- 'never'    = asla önerme (öneri listesinden hard filtre)
-- 'learning' = öğreniyorum (sınırlı pozitif boost)
-- Yeniden-üretilemez kullanıcı verisi (feedback/not/hedef sınıfı).

CREATE TABLE IF NOT EXISTS user_preferences (
    puuid       TEXT    NOT NULL,
    champion_id INTEGER NOT NULL,
    preference  TEXT    NOT NULL CHECK (preference IN ('never', 'learning')),
    created_at  INTEGER NOT NULL,
    PRIMARY KEY (puuid, champion_id)
);
