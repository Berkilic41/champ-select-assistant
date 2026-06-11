CREATE TABLE IF NOT EXISTS draft_samples (
    id UUID PRIMARY KEY,
    user_hash TEXT NOT NULL,
    patch TEXT NOT NULL,
    region TEXT NOT NULL,
    queue_id INTEGER NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS recommendation_feedback (
    id UUID PRIMARY KEY,
    user_hash TEXT NOT NULL,
    champion_id INTEGER NOT NULL,
    champion_key TEXT NOT NULL,
    feedback TEXT NOT NULL,
    model_version TEXT NOT NULL,
    score REAL NOT NULL DEFAULT 0.0,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS match_outcomes (
    id UUID PRIMARY KEY,
    user_hash TEXT NOT NULL,
    match_id_hash TEXT NOT NULL,
    champion_id INTEGER NOT NULL,
    position TEXT NOT NULL,
    win BOOLEAN NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS champion_rates (
    patch TEXT NOT NULL,
    region TEXT NOT NULL,
    champion_id INTEGER NOT NULL,
    position TEXT NOT NULL,
    win_rate REAL NOT NULL,
    pick_rate REAL NOT NULL DEFAULT 0.0,
    ban_rate REAL NOT NULL DEFAULT 0.0,
    sample_size INTEGER NOT NULL DEFAULT 0,
    source TEXT NOT NULL,
    confidence TEXT NOT NULL DEFAULT 'low',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (patch, region, champion_id, position, source)
);

CREATE TABLE IF NOT EXISTS champion_matchups (
    patch TEXT NOT NULL,
    region TEXT NOT NULL,
    champion_id INTEGER NOT NULL,
    opponent_id INTEGER NOT NULL,
    position TEXT NOT NULL,
    games INTEGER NOT NULL DEFAULT 0,
    wins INTEGER NOT NULL DEFAULT 0,
    win_rate REAL NOT NULL DEFAULT 0.5,
    source TEXT NOT NULL,
    confidence TEXT NOT NULL DEFAULT 'low',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (patch, region, champion_id, opponent_id, position, source)
);

CREATE TABLE IF NOT EXISTS champion_builds (
    patch TEXT NOT NULL,
    region TEXT NOT NULL,
    champion_id INTEGER NOT NULL,
    position TEXT NOT NULL,
    payload JSONB NOT NULL,
    source TEXT NOT NULL,
    confidence TEXT NOT NULL DEFAULT 'low',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (patch, region, champion_id, position, source)
);

CREATE TABLE IF NOT EXISTS model_packs (
    version TEXT PRIMARY KEY,
    payload JSONB NOT NULL,
    active BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS source_fetch_log (
    id UUID PRIMARY KEY,
    source TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    status TEXT NOT NULL,
    risk_level TEXT NOT NULL DEFAULT 'medium',
    message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_feedback_created_at ON recommendation_feedback (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_draft_samples_created_at ON draft_samples (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_matchups_lookup ON champion_matchups (patch, region, champion_id, opponent_id, position);
