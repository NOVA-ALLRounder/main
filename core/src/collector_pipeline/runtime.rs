use super::*;

pub fn resolve_db_path(config_path: Option<&Path>) -> PathBuf {
    if let Some(path) = env_path("STEER_COLLECTOR_DB_PATH") {
        return path;
    }

    if let Some(path) = config_db_path(config_path) {
        return path;
    }

    if let Some(path) = env_path("STEER_DB_PATH") {
        return path;
    }

    if !collector_separate_db_enabled() && collector_auto_link_enabled() {
        if let Some(path) = discover_shared_steer_db() {
            return path;
        }
    }

    if let Some(mut path) = dirs::data_local_dir() {
        path.push("steer");
        let _ = fs::create_dir_all(&path);
        path.push(DEFAULT_COLLECTOR_DB_FILE);
        return path;
    }

    PathBuf::from(DEFAULT_COLLECTOR_DB_FILE)
}

pub fn resolve_privacy_rules_path(config_path: Option<&Path>) -> PathBuf {
    if let Ok(value) = std::env::var("STEER_PRIVACY_RULES_PATH") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Some(path) = config_privacy_path(config_path) {
        return path;
    }

    PathBuf::from("configs/privacy_rules.yaml")
}

pub fn open_connection(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed creating db dir {}", parent.display()))?;
        }
    }
    let conn = Connection::open(path)
        .with_context(|| format!("failed opening sqlite db {}", path.display()))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

pub fn ensure_pipeline_tables(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS events_v2 (
            schema_version TEXT,
            event_id TEXT PRIMARY KEY,
            ts TEXT NOT NULL,
            source TEXT NOT NULL,
            app TEXT NOT NULL,
            event_type TEXT NOT NULL,
            priority TEXT,
            resource_type TEXT,
            resource_id TEXT,
            payload_json TEXT,
            privacy_json TEXT,
            pid INTEGER,
            window_id TEXT,
            window_title TEXT,
            browser_url TEXT,
            raw_json TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_events_v2_ts ON events_v2(ts)",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS sessions_v2 (
            session_id TEXT PRIMARY KEY,
            start_ts TEXT NOT NULL,
            end_ts TEXT NOT NULL,
            duration_sec INTEGER,
            summary_json TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS collector_state (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS collector_routine_candidates (
            pattern_id TEXT PRIMARY KEY,
            pattern_json TEXT NOT NULL,
            support INTEGER NOT NULL,
            confidence REAL NOT NULL,
            last_seen_ts TEXT NOT NULL,
            evidence_session_ids TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_collector_routines_support
         ON collector_routine_candidates(support)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_collector_routines_last_seen
         ON collector_routine_candidates(last_seen_ts)",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS collector_handoff_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            package_id TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL,
            status TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            payload_size INTEGER NOT NULL,
            expires_at TEXT,
            error TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_collector_handoff_status
         ON collector_handoff_queue(status)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_collector_handoff_created
         ON collector_handoff_queue(created_at)",
        [],
    )?;
    ensure_column_exists(
        conn,
        "collector_handoff_queue",
        "attempt_count",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column_exists(conn, "collector_handoff_queue", "last_attempt_at", "TEXT")?;
    ensure_column_exists(conn, "collector_handoff_queue", "next_retry_at", "TEXT")?;
    ensure_column_exists(conn, "collector_handoff_queue", "lease_until", "TEXT")?;
    ensure_column_exists(conn, "collector_handoff_queue", "claimed_by", "TEXT")?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_collector_handoff_next_retry
         ON collector_handoff_queue(next_retry_at)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_collector_handoff_lease
         ON collector_handoff_queue(lease_until)",
        [],
    )?;

    Ok(())
}

pub fn get_state(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM collector_state WHERE key = ?1",
        [key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(Into::into)
}

pub fn set_state(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO collector_state (key, value, updated_at)
         VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(key) DO UPDATE SET
             value = excluded.value,
             updated_at = excluded.updated_at",
        params![key, value],
    )?;
    Ok(())
}

pub fn fetch_latest_session_end_ts(conn: &Connection) -> Result<Option<String>> {
    conn.query_row(
        "SELECT end_ts FROM sessions_v2 ORDER BY end_ts DESC LIMIT 1",
        [],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(Into::into)
}
