CREATE TABLE IF NOT EXISTS daily_summaries (
    date_local TEXT PRIMARY KEY,
    start_utc TEXT,
    end_utc TEXT,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS pattern_summaries (
    created_at TEXT PRIMARY KEY,
    window_days INTEGER NOT NULL,
    payload_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS llm_inputs (
    created_at TEXT PRIMARY KEY,
    payload_json TEXT NOT NULL,
    payload_size INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_daily_summaries_created_at
ON daily_summaries(created_at);

CREATE INDEX IF NOT EXISTS idx_pattern_summaries_created_at
ON pattern_summaries(created_at);

CREATE INDEX IF NOT EXISTS idx_llm_inputs_created_at
ON llm_inputs(created_at);
