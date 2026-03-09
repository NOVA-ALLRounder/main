use rusqlite::{params, Result};
use serde_json::Value;

use crate::db::{get_db_lock, normalize_admin_limit, truncate_text};

use super::support::{
    map_execution_memory_row, normalize_feedback_signal, optional_truncated_signature,
    scoped_execution_params_key,
};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecutionMemoryRecord {
    pub id: i64,
    pub memory_scope: String,
    pub intent_command: String,
    pub params_key: String,
    pub params_json: Option<String>,
    pub request_signature: Option<String>,
    pub response_text: String,
    pub source: String,
    pub tool_path: String,
    pub freshness_ttl_seconds: i64,
    pub success: bool,
    pub positive_feedback_count: i64,
    pub negative_feedback_count: i64,
    pub last_feedback_at: Option<String>,
    pub suppressed: bool,
    pub suppressed_reason: Option<String>,
    pub suppressed_at: Option<String>,
    pub use_count: i64,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: String,
}

#[cfg(test)]
pub fn clear_execution_memory_for_tests() {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let _ = conn.execute("DELETE FROM execution_memory", []);
    }
}

pub fn upsert_execution_memory(
    intent_command: &str,
    params_key: &str,
    params_json: Option<&Value>,
    request_signature: Option<&str>,
    response_text: &str,
    freshness_ttl_seconds: i64,
    source: &str,
    tool_path: &str,
) -> Result<()> {
    upsert_execution_memory_scoped(
        None,
        intent_command,
        params_key,
        params_json,
        request_signature,
        response_text,
        freshness_ttl_seconds,
        source,
        tool_path,
    )
}

pub fn upsert_execution_memory_scoped(
    memory_scope: Option<&str>,
    intent_command: &str,
    params_key: &str,
    params_json: Option<&Value>,
    request_signature: Option<&str>,
    response_text: &str,
    freshness_ttl_seconds: i64,
    source: &str,
    tool_path: &str,
) -> Result<()> {
    let intent_command = truncate_text(intent_command, 64);
    let response_text = truncate_text(response_text, 4000);
    let Some((memory_scope, _logical_params_key, params_key)) =
        scoped_execution_params_key(params_key, memory_scope)
    else {
        return Ok(());
    };
    if intent_command.trim().is_empty() || response_text.is_empty() {
        return Ok(());
    }

    let params_json = params_json.map(|value| truncate_text(&value.to_string(), 1000));
    let request_signature = request_signature.and_then(optional_truncated_signature);
    let now = chrono::Utc::now().to_rfc3339();

    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "INSERT INTO execution_memory (
                memory_scope, intent_command, params_key, params_json, request_signature, response_text,
                source, tool_path, freshness_ttl_seconds, success,
                positive_feedback_count, negative_feedback_count, last_feedback_at,
                suppressed, suppressed_reason, suppressed_at,
                use_count, created_at, updated_at, last_used_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, 0, 0, NULL, 0, NULL, NULL, 1, ?10, ?10, ?10)
            ON CONFLICT(intent_command, params_key) DO UPDATE SET
                memory_scope = excluded.memory_scope,
                params_json = COALESCE(excluded.params_json, execution_memory.params_json),
                request_signature = COALESCE(excluded.request_signature, execution_memory.request_signature),
                response_text = excluded.response_text,
                source = excluded.source,
                tool_path = excluded.tool_path,
                freshness_ttl_seconds = excluded.freshness_ttl_seconds,
                success = excluded.success,
                positive_feedback_count = CASE
                    WHEN excluded.response_text != execution_memory.response_text THEN 0
                    ELSE execution_memory.positive_feedback_count
                END,
                negative_feedback_count = CASE
                    WHEN excluded.response_text != execution_memory.response_text THEN 0
                    ELSE execution_memory.negative_feedback_count
                END,
                last_feedback_at = CASE
                    WHEN excluded.response_text != execution_memory.response_text THEN NULL
                    ELSE execution_memory.last_feedback_at
                END,
                suppressed = execution_memory.suppressed,
                suppressed_reason = execution_memory.suppressed_reason,
                suppressed_at = execution_memory.suppressed_at,
                use_count = execution_memory.use_count + 1,
                updated_at = excluded.updated_at,
                last_used_at = excluded.last_used_at",
            params![
                memory_scope,
                intent_command,
                params_key,
                params_json,
                request_signature,
                response_text,
                truncate_text(source, 64),
                truncate_text(tool_path, 128),
                freshness_ttl_seconds.max(0),
                now,
            ],
        )?;
    }
    Ok(())
}

pub fn get_execution_memory(
    intent_command: &str,
    params_key: &str,
) -> Result<Option<ExecutionMemoryRecord>> {
    get_execution_memory_scoped(None, intent_command, params_key)
}

pub fn get_execution_memory_scoped(
    memory_scope: Option<&str>,
    intent_command: &str,
    params_key: &str,
) -> Result<Option<ExecutionMemoryRecord>> {
    let intent_command = truncate_text(intent_command, 64);
    let Some((memory_scope, _logical_params_key, params_key)) =
        scoped_execution_params_key(params_key, memory_scope)
    else {
        return Ok(None);
    };
    if intent_command.trim().is_empty() {
        return Ok(None);
    }

    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT id, memory_scope, intent_command, params_key, params_json, request_signature,
                    response_text, source, tool_path, freshness_ttl_seconds, success,
                    positive_feedback_count, negative_feedback_count, last_feedback_at,
                    suppressed, suppressed_reason, suppressed_at,
                    use_count, created_at, updated_at, last_used_at
             FROM execution_memory
             WHERE memory_scope = ?1
               AND intent_command = ?2
               AND params_key = ?3
               AND suppressed = 0
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![memory_scope, intent_command, params_key])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(map_execution_memory_row(row)?));
        }
    }
    Ok(None)
}

pub fn list_execution_memory_records(
    limit: i64,
    include_suppressed: bool,
) -> Result<Vec<ExecutionMemoryRecord>> {
    let limit = normalize_admin_limit(limit);
    let mut out = Vec::new();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = if include_suppressed {
            conn.prepare(
                "SELECT id, memory_scope, intent_command, params_key, params_json, request_signature,
                        response_text, source, tool_path, freshness_ttl_seconds, success,
                        positive_feedback_count, negative_feedback_count, last_feedback_at,
                        suppressed, suppressed_reason, suppressed_at,
                        use_count, created_at, updated_at, last_used_at
                 FROM execution_memory
                 ORDER BY suppressed DESC, last_used_at DESC
                 LIMIT ?1",
            )?
        } else {
            conn.prepare(
                "SELECT id, memory_scope, intent_command, params_key, params_json, request_signature,
                        response_text, source, tool_path, freshness_ttl_seconds, success,
                        positive_feedback_count, negative_feedback_count, last_feedback_at,
                        suppressed, suppressed_reason, suppressed_at,
                        use_count, created_at, updated_at, last_used_at
                 FROM execution_memory
                 WHERE suppressed = 0
                 ORDER BY last_used_at DESC
                 LIMIT ?1",
            )?
        };
        let rows = stmt.query_map(params![limit], map_execution_memory_row)?;
        for row in rows.flatten() {
            out.push(row);
        }
    }
    Ok(out)
}

pub fn suppress_execution_memory(
    intent_command: &str,
    params_key: &str,
    reason: Option<&str>,
) -> Result<bool> {
    suppress_execution_memory_scoped(None, intent_command, params_key, reason)
}

pub fn suppress_execution_memory_scoped(
    memory_scope: Option<&str>,
    intent_command: &str,
    params_key: &str,
    reason: Option<&str>,
) -> Result<bool> {
    let intent_command = truncate_text(intent_command, 64);
    let Some((memory_scope, _logical_params_key, params_key)) =
        scoped_execution_params_key(params_key, memory_scope)
    else {
        return Ok(false);
    };
    let now = chrono::Utc::now().to_rfc3339();
    let reason = reason
        .map(|value| truncate_text(value, 500))
        .filter(|value| !value.is_empty());
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let rows = conn.execute(
            "UPDATE execution_memory
             SET suppressed = 1,
                 suppressed_reason = COALESCE(?1, suppressed_reason),
                 suppressed_at = ?2,
                 updated_at = ?2
             WHERE memory_scope = ?3
               AND intent_command = ?4
               AND params_key = ?5",
            params![reason, now, memory_scope, intent_command, params_key],
        )?;
        return Ok(rows > 0);
    }
    Ok(false)
}

pub fn restore_execution_memory(intent_command: &str, params_key: &str) -> Result<bool> {
    restore_execution_memory_scoped(None, intent_command, params_key)
}

pub fn restore_execution_memory_scoped(
    memory_scope: Option<&str>,
    intent_command: &str,
    params_key: &str,
) -> Result<bool> {
    let intent_command = truncate_text(intent_command, 64);
    let Some((memory_scope, _logical_params_key, params_key)) =
        scoped_execution_params_key(params_key, memory_scope)
    else {
        return Ok(false);
    };
    let now = chrono::Utc::now().to_rfc3339();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let rows = conn.execute(
            "UPDATE execution_memory
             SET suppressed = 0,
                 suppressed_reason = NULL,
                 suppressed_at = NULL,
                 updated_at = ?1
             WHERE memory_scope = ?2
               AND intent_command = ?3
               AND params_key = ?4",
            params![now, memory_scope, intent_command, params_key],
        )?;
        return Ok(rows > 0);
    }
    Ok(false)
}

pub fn delete_execution_memory(intent_command: &str, params_key: &str) -> Result<bool> {
    delete_execution_memory_scoped(None, intent_command, params_key)
}

pub fn delete_execution_memory_scoped(
    memory_scope: Option<&str>,
    intent_command: &str,
    params_key: &str,
) -> Result<bool> {
    let intent_command = truncate_text(intent_command, 64);
    let Some((memory_scope, _logical_params_key, params_key)) =
        scoped_execution_params_key(params_key, memory_scope)
    else {
        return Ok(false);
    };
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let rows = conn.execute(
            "DELETE FROM execution_memory
             WHERE memory_scope = ?1
               AND intent_command = ?2
               AND params_key = ?3",
            params![memory_scope, intent_command, params_key],
        )?;
        return Ok(rows > 0);
    }
    Ok(false)
}

pub fn record_execution_memory_feedback(
    intent_command: &str,
    params_key: &str,
    response_text: &str,
    sentiment: &str,
) -> Result<bool> {
    record_execution_memory_feedback_scoped(
        None,
        intent_command,
        params_key,
        response_text,
        sentiment,
    )
}

pub fn record_execution_memory_feedback_scoped(
    memory_scope: Option<&str>,
    intent_command: &str,
    params_key: &str,
    response_text: &str,
    sentiment: &str,
) -> Result<bool> {
    let Some(sentiment) = normalize_feedback_signal(sentiment) else {
        return Ok(false);
    };
    let intent_command = truncate_text(intent_command, 64);
    let Some((memory_scope, _logical_params_key, params_key)) =
        scoped_execution_params_key(params_key, memory_scope)
    else {
        return Ok(false);
    };
    let response_text = truncate_text(response_text, 4000);
    if intent_command.trim().is_empty() || response_text.is_empty() {
        return Ok(false);
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let positive_delta = if sentiment == "positive" { 1 } else { 0 };
        let negative_delta = if sentiment == "negative" { 1 } else { 0 };
        let rows = conn.execute(
            "UPDATE execution_memory
             SET positive_feedback_count = positive_feedback_count + ?1,
                 negative_feedback_count = negative_feedback_count + ?2,
                 last_feedback_at = ?3,
                 updated_at = ?3
             WHERE memory_scope = ?4
               AND intent_command = ?5
               AND params_key = ?6
               AND response_text = ?7",
            params![
                positive_delta,
                negative_delta,
                now,
                memory_scope,
                intent_command,
                params_key,
                response_text
            ],
        )?;
        return Ok(rows > 0);
    }
    Ok(false)
}
