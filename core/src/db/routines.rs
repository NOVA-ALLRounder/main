use rusqlite::{params, Result};
use std::str::FromStr;

use super::{with_read_conn, with_write_conn_if_available};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Routine {
    pub id: i64,
    pub name: String,
    pub cron_expression: String,
    pub prompt: String,
    pub enabled: bool,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LearnedRoutine {
    pub id: i64,
    pub name: String,
    pub steps_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DashboardStats {
    pub total_sessions: i64,
    pub total_time_mins: i64,
    pub top_apps: Vec<(String, i64)>,
    pub rec_pending: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PolicyConfigReport {
    pub tool_allowlist: Vec<String>,
    pub tool_denylist: Vec<String>,
    pub shell_allowlist: Vec<String>,
    pub shell_denylist: Vec<String>,
    pub write_lock_default: bool,
}

fn map_routine_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Routine> {
    Ok(Routine {
        id: row.get(0)?,
        name: row.get(1)?,
        cron_expression: row.get(2)?,
        prompt: row.get(3)?,
        enabled: row.get(4)?,
        last_run: row.get(5)?,
        next_run: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn map_learned_routine_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LearnedRoutine> {
    Ok(LearnedRoutine {
        id: row.get(0)?,
        name: row.get(1)?,
        steps_json: row.get(2)?,
        created_at: row.get(3)?,
    })
}

pub fn create_routine(name: &str, cron: &str, prompt: &str) -> Result<i64> {
    if let Some(id) = with_write_conn_if_available(|conn| {
        let created_at = chrono::Utc::now().to_rfc3339();
        let next_run = match cron::Schedule::from_str(cron) {
            Ok(schedule) => schedule
                .upcoming(chrono::Utc)
                .next()
                .map(|dt: chrono::DateTime<chrono::Utc>| dt.to_rfc3339()),
            Err(_) => None,
        };

        conn.execute(
            "INSERT INTO routines (name, cron_expression, prompt, created_at, next_run) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![name, cron, prompt, created_at, next_run],
        )?;
        Ok(conn.last_insert_rowid())
    })? {
        return Ok(id);
    }
    Err(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(1),
        Some("DB not initialized".to_string()),
    ))
}

pub fn get_due_routines() -> Result<Vec<Routine>> {
    match with_read_conn(|conn| {
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT id, name, cron_expression, prompt, enabled, last_run, next_run, created_at
             FROM routines
             WHERE enabled = 1 AND next_run <= ?1",
        )?;
        let rows = stmt.query_map(params![now], map_routine_row)?;

        let mut routines = Vec::new();
        for routine in rows {
            routines.push(routine?);
        }
        Ok(routines)
    }) {
        Ok(routines) => Ok(routines),
        Err(rusqlite::Error::InvalidQuery) => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

pub fn claim_routine_execution(routine_id: i64, owner: &str) -> Result<bool> {
    if let Some(claimed) = with_write_conn_if_available(|conn| {
        let now_dt = chrono::Utc::now();
        let now = now_dt.to_rfc3339();
        let stale_before = now_dt - chrono::Duration::minutes(parse_routine_claim_stale_minutes());

        let tx = conn.transaction()?;
        let mut found = false;
        let mut enabled_raw: i64 = 0;
        let mut next_run: Option<String> = None;
        let mut claimed_at: Option<String> = None;
        {
            let mut stmt = tx.prepare(
                "SELECT enabled, next_run, run_claimed_at
                 FROM routines
                 WHERE id = ?1",
            )?;
            let mut rows = stmt.query(params![routine_id])?;
            if let Some(row) = rows.next()? {
                found = true;
                enabled_raw = row.get(0)?;
                next_run = row.get(1)?;
                claimed_at = row.get(2)?;
            }
        }
        if !found {
            tx.rollback()?;
            return Ok(false);
        }
        if enabled_raw == 0 {
            tx.rollback()?;
            return Ok(false);
        }
        if let Some(next) = next_run.as_deref().and_then(parse_ts_utc) {
            if next > now_dt {
                tx.rollback()?;
                return Ok(false);
            }
        }
        if let Some(claimed) = claimed_at.as_deref().and_then(parse_ts_utc) {
            if claimed > stale_before {
                tx.rollback()?;
                return Ok(false);
            }
        }
        tx.execute(
            "UPDATE routines
             SET run_claimed_at = ?1, run_claim_owner = ?2
             WHERE id = ?3",
            params![now, owner, routine_id],
        )?;
        tx.commit()?;
        Ok(true)
    })? {
        return Ok(claimed);
    }
    Ok(false)
}

pub fn release_routine_execution(routine_id: i64, owner: Option<&str>) -> Result<()> {
    with_write_conn_if_available(|conn| {
        if let Some(owner_value) = owner {
            conn.execute(
                "UPDATE routines
                 SET run_claimed_at = NULL, run_claim_owner = NULL
                 WHERE id = ?1 AND (run_claim_owner = ?2 OR run_claim_owner IS NULL)",
                params![routine_id, owner_value],
            )?;
        } else {
            conn.execute(
                "UPDATE routines
                 SET run_claimed_at = NULL, run_claim_owner = NULL
                 WHERE id = ?1",
                params![routine_id],
            )?;
        }
        Ok(())
    })?;
    Ok(())
}

pub fn update_routine_execution(id: i64, next: Option<String>) -> Result<()> {
    if let Some(()) = with_write_conn_if_available(|conn| {
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE routines SET last_run = ?1, next_run = ?2 WHERE id = ?3",
            params![now, next, id],
        )?;
        Ok(())
    })? {
        return Ok(());
    }
    Ok(())
}

pub fn get_active_routines() -> Result<Vec<Routine>> {
    match with_read_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, name, cron_expression, prompt, enabled, last_run, next_run, created_at
             FROM routines
             WHERE enabled = 1",
        )?;
        let rows = stmt.query_map([], map_routine_row)?;

        let mut routines = Vec::new();
        for routine in rows {
            routines.push(routine?);
        }
        Ok(routines)
    }) {
        Ok(routines) => Ok(routines),
        Err(rusqlite::Error::InvalidQuery) => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

pub fn get_all_routines() -> Result<Vec<Routine>> {
    match with_read_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, name, cron_expression, prompt, enabled, last_run, next_run, created_at
             FROM routines
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], map_routine_row)?;

        let mut routines = Vec::new();
        for routine in rows {
            routines.push(routine?);
        }
        Ok(routines)
    }) {
        Ok(routines) => Ok(routines),
        Err(rusqlite::Error::InvalidQuery) => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

pub fn toggle_routine(id: i64, enabled: bool) -> Result<()> {
    if let Some(()) = with_write_conn_if_available(|conn| {
        conn.execute(
            "UPDATE routines SET enabled = ?1 WHERE id = ?2",
            params![enabled, id],
        )?;
        Ok(())
    })? {
        return Ok(());
    }
    Err(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(1),
        Some("DB not initialized".to_string()),
    ))
}

pub fn is_exec_allowlisted(command: &str, cwd: Option<&str>) -> Result<bool> {
    if let Some(allowed) = with_write_conn_if_available(|conn| {
        let mut stmt =
            conn.prepare("SELECT id, pattern, cwd FROM exec_allowlist ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| {
            let cwd: Option<String> = row.get(2)?;
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, cwd))
        })?;
        for row in rows {
            let (id, pattern, entry_cwd) = row?;
            if let Some(required_cwd) = entry_cwd {
                if let Some(cwd_val) = cwd {
                    if required_cwd != cwd_val {
                        continue;
                    }
                } else {
                    continue;
                }
            }
            if exec_pattern_match(&pattern, command) {
                let now = chrono::Utc::now().to_rfc3339();
                let _ = conn.execute(
                    "UPDATE exec_allowlist
                     SET last_used_at = ?1, uses_count = uses_count + 1
                     WHERE id = ?2",
                    params![now, id],
                );
                return Ok(true);
            }
        }
        Ok(false)
    })? {
        return Ok(allowed);
    }
    Ok(false)
}

pub(crate) fn exec_pattern_match_with_flags(
    pattern: &str,
    command: &str,
    allow_global: bool,
    allow_regex: bool,
) -> bool {
    let trimmed = pattern.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed == "*" || trimmed.eq_ignore_ascii_case("all") {
        return allow_global;
    }
    if let Some(rest) = trimmed.strip_prefix("re:") {
        if !allow_regex {
            return false;
        }
        if let Ok(re) = regex::Regex::new(rest) {
            return re.is_match(command);
        }
    }
    if trimmed.starts_with('/') && trimmed.ends_with('/') && trimmed.len() > 2 {
        if !allow_regex {
            return false;
        }
        let body = &trimmed[1..trimmed.len() - 1];
        if let Ok(re) = regex::Regex::new(body) {
            return re.is_match(command);
        }
    }
    if trimmed.ends_with('*') {
        let prefix = trimmed.trim_end_matches('*');
        return command.starts_with(prefix);
    }
    command == trimmed
}

pub(crate) fn validate_exec_allowlist_pattern_with_flags(
    pattern: &str,
    allow_global: bool,
    allow_regex: bool,
) -> Result<()> {
    let trimmed = pattern.trim();
    if trimmed.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "exec allowlist pattern rejected: empty pattern".to_string(),
        ));
    }
    if trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(rusqlite::Error::InvalidParameterName(
            "exec allowlist pattern rejected: multiline pattern is not allowed".to_string(),
        ));
    }
    if trimmed == "*" || trimmed.eq_ignore_ascii_case("all") {
        if !allow_global {
            return Err(rusqlite::Error::InvalidParameterName(
                "exec allowlist pattern rejected: global wildcard requires STEER_EXEC_ALLOWLIST_ALLOW_GLOBAL=1".to_string(),
            ));
        }
        return Ok(());
    }
    if trimmed.starts_with("re:")
        || (trimmed.starts_with('/') && trimmed.ends_with('/') && trimmed.len() > 2)
    {
        if !allow_regex {
            return Err(rusqlite::Error::InvalidParameterName(
                "exec allowlist pattern rejected: regex requires STEER_EXEC_ALLOWLIST_ALLOW_REGEX=1".to_string(),
            ));
        }
        return Ok(());
    }
    Ok(())
}

pub(crate) fn validate_exec_allowlist_pattern(pattern: &str) -> Result<()> {
    validate_exec_allowlist_pattern_with_flags(
        pattern,
        crate::env_flag("STEER_EXEC_ALLOWLIST_ALLOW_GLOBAL"),
        crate::env_flag("STEER_EXEC_ALLOWLIST_ALLOW_REGEX"),
    )
}

pub fn save_learned_routine(name: &str, steps_json: &str) -> Result<()> {
    if let Some(()) = with_write_conn_if_available(|conn| {
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO learned_routines (name, steps_json, created_at) VALUES (?1, ?2, ?3)",
            params![name, steps_json, created_at],
        )?;
        Ok(())
    })? {
        return Ok(());
    }
    Err(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(1),
        Some("DB not initialized".to_string()),
    ))
}

pub fn get_learned_routine(name: &str) -> Result<Option<LearnedRoutine>> {
    match with_read_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, name, steps_json, created_at FROM learned_routines WHERE name = ?1",
        )?;
        let mut rows = stmt.query(params![name])?;

        if let Some(row) = rows.next()? {
            Ok(Some(map_learned_routine_row(row)?))
        } else {
            Ok(None)
        }
    }) {
        Ok(routine) => Ok(routine),
        Err(rusqlite::Error::InvalidQuery) => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn list_learned_routines() -> Result<Vec<LearnedRoutine>> {
    match with_read_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, name, steps_json, created_at
             FROM learned_routines
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], map_learned_routine_row)?;

        let mut routines = Vec::new();
        for row in rows {
            routines.push(row?);
        }
        Ok(routines)
    }) {
        Ok(routines) => Ok(routines),
        Err(rusqlite::Error::InvalidQuery) => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

pub fn delete_learned_routine(id: i64) -> Result<()> {
    with_write_conn_if_available(|conn| {
        conn.execute("DELETE FROM learned_routines WHERE id = ?1", params![id])?;
        Ok(())
    })?;
    Ok(())
}

pub fn get_dashboard_stats() -> Result<DashboardStats> {
    match with_read_conn(|conn| {
        let (total_sessions, total_time_mins): (i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(duration_sec)/60, 0) FROM sessions_v2",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or_default();

        let now = chrono::Utc::now().to_rfc3339();
        let rec_pending: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM recommendations
                 WHERE status = 'pending'
                   AND (snoozed_until IS NULL OR TRIM(snoozed_until) = '' OR snoozed_until <= ?1)",
                params![now],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let mut top_apps = Vec::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT app_name, count(*) as c
             FROM (SELECT app as app_name FROM events_v2)
             GROUP BY app_name
             ORDER BY c DESC
             LIMIT 3",
        ) {
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            });
            if let Ok(iter) = rows {
                for value in iter.flatten() {
                    top_apps.push(value);
                }
            }
        }

        Ok(DashboardStats {
            total_sessions,
            total_time_mins,
            top_apps,
            rec_pending,
        })
    }) {
        Ok(stats) => Ok(stats),
        Err(rusqlite::Error::InvalidQuery) => Ok(DashboardStats {
            total_sessions: 0,
            total_time_mins: 0,
            top_apps: vec![],
            rec_pending: 0,
        }),
        Err(error) => Err(error),
    }
}

pub fn get_active_policy_config() -> PolicyConfigReport {
    PolicyConfigReport {
        tool_allowlist: std::env::var("TOOL_ALLOWLIST")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        tool_denylist: std::env::var("TOOL_DENYLIST")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        shell_allowlist: std::env::var("SHELL_ALLOWLIST")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        shell_denylist: std::env::var("SHELL_DENYLIST")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        write_lock_default: true,
    }
}

fn parse_routine_claim_stale_minutes() -> i64 {
    std::env::var("STEER_ROUTINE_CLAIM_STALE_MINUTES")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value >= 1)
        .unwrap_or(120)
}

fn parse_ts_utc(value: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

fn exec_pattern_match(pattern: &str, command: &str) -> bool {
    exec_pattern_match_with_flags(
        pattern,
        command,
        crate::env_flag("STEER_EXEC_ALLOWLIST_ALLOW_GLOBAL"),
        crate::env_flag("STEER_EXEC_ALLOWLIST_ALLOW_REGEX"),
    )
}
