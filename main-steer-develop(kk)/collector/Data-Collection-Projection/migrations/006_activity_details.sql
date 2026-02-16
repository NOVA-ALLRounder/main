CREATE TABLE IF NOT EXISTS activity_details (
    app TEXT NOT NULL,
    title_hash TEXT NOT NULL,
    title_hint TEXT,
    first_seen_ts TEXT,
    last_seen_ts TEXT,
    total_duration_sec INTEGER NOT NULL DEFAULT 0,
    blocks INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (app, title_hash)
);

CREATE INDEX IF NOT EXISTS idx_activity_details_last_seen
ON activity_details(last_seen_ts);
