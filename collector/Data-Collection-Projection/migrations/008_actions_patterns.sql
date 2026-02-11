-- 008: Actions and Patterns tables for integrated ActionBuffer/PatternAnalyzer

CREATE TABLE IF NOT EXISTS actions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    action_id       TEXT UNIQUE NOT NULL,
    action_type     TEXT NOT NULL,
    app             TEXT,
    window_title    TEXT,
    control_name    TEXT,
    control_type    TEXT,
    final_value     TEXT,
    started_at      TEXT NOT NULL,
    ended_at        TEXT NOT NULL,
    duration_ms     INTEGER DEFAULT 0,
    metadata_json   TEXT,
    created_at      TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_actions_started ON actions(started_at);
CREATE INDEX IF NOT EXISTS idx_actions_app     ON actions(app);
CREATE INDEX IF NOT EXISTS idx_actions_type    ON actions(action_type);

CREATE TABLE IF NOT EXISTS potential_patterns (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern_hash     TEXT UNIQUE NOT NULL,
    sequence         TEXT NOT NULL,
    occurrence_count INTEGER DEFAULT 0,
    pattern_length   INTEGER DEFAULT 0,
    detected_at      TEXT NOT NULL,
    actions_json     TEXT,
    created_at       TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_patterns_hash     ON potential_patterns(pattern_hash);
CREATE INDEX IF NOT EXISTS idx_patterns_detected ON potential_patterns(detected_at);
