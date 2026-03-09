use rusqlite::{params, params_from_iter, Result};
use serde_json::Value;

use crate::db::{get_db_lock, normalize_admin_limit, truncate_text};
use crate::request_memory::build_request_signature;

use super::support::{
    map_request_memory_row, normalize_feedback_signal, optional_truncated_signature,
    request_memory_intent_command, scoped_request_memory_storage_key,
};

#[derive(Debug, Clone, serde::Serialize)]
pub struct RequestMemoryRecord {
    pub normalized_request: String,
    pub memory_scope: String,
    pub original_request: String,
    pub request_signature: Option<String>,
    pub intent_json: Option<String>,
    pub intent_command: Option<String>,
    pub response_text: Option<String>,
    pub response_mode: String,
    pub source: String,
    pub confidence: f64,
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
pub fn clear_request_memory_for_tests() {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let _ = conn.execute("DELETE FROM request_memory", []);
    }
}

pub fn upsert_request_memory(
    request_text: &str,
    intent_json: Option<&Value>,
    response_text: Option<&str>,
    response_mode: &str,
    source: &str,
    confidence: f64,
) -> Result<()> {
    upsert_request_memory_scoped(
        None,
        request_text,
        intent_json,
        response_text,
        response_mode,
        source,
        confidence,
    )
}

pub fn upsert_request_memory_scoped(
    memory_scope: Option<&str>,
    request_text: &str,
    intent_json: Option<&Value>,
    response_text: Option<&str>,
    response_mode: &str,
    source: &str,
    confidence: f64,
) -> Result<()> {
    let Some((memory_scope, normalized_request)) =
        scoped_request_memory_storage_key(request_text, memory_scope)
    else {
        return Ok(());
    };

    let original_request = truncate_text(request_text, 800);
    let request_signature = optional_truncated_signature(&build_request_signature(request_text));
    let intent_json_str = intent_json.map(|value| value.to_string());
    let intent_command = request_memory_intent_command(intent_json);
    let response_text = response_text.map(|value| truncate_text(value, 4000));
    let now = chrono::Utc::now().to_rfc3339();

    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        conn.execute(
            "INSERT INTO request_memory (
                normalized_request, memory_scope, original_request, request_signature, intent_json, intent_command,
                response_text, response_mode, source, confidence,
                positive_feedback_count, negative_feedback_count, last_feedback_at,
                suppressed, suppressed_reason, suppressed_at,
                use_count, created_at, updated_at, last_used_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, 0, NULL, 0, NULL, NULL, 1, ?11, ?11, ?11)
            ON CONFLICT(normalized_request) DO UPDATE SET
                memory_scope = excluded.memory_scope,
                original_request = excluded.original_request,
                request_signature = COALESCE(excluded.request_signature, request_memory.request_signature),
                intent_json = COALESCE(excluded.intent_json, request_memory.intent_json),
                intent_command = COALESCE(excluded.intent_command, request_memory.intent_command),
                response_text = COALESCE(excluded.response_text, request_memory.response_text),
                response_mode = excluded.response_mode,
                source = excluded.source,
                confidence = CASE
                    WHEN excluded.confidence > request_memory.confidence THEN excluded.confidence
                    ELSE request_memory.confidence
                END,
                positive_feedback_count = CASE
                    WHEN excluded.response_text IS NOT NULL
                         AND request_memory.response_text IS NOT NULL
                         AND excluded.response_text != request_memory.response_text THEN 0
                    ELSE request_memory.positive_feedback_count
                END,
                negative_feedback_count = CASE
                    WHEN excluded.response_text IS NOT NULL
                         AND request_memory.response_text IS NOT NULL
                         AND excluded.response_text != request_memory.response_text THEN 0
                    ELSE request_memory.negative_feedback_count
                END,
                last_feedback_at = CASE
                    WHEN excluded.response_text IS NOT NULL
                         AND request_memory.response_text IS NOT NULL
                         AND excluded.response_text != request_memory.response_text THEN NULL
                    ELSE request_memory.last_feedback_at
                END,
                suppressed = request_memory.suppressed,
                suppressed_reason = request_memory.suppressed_reason,
                suppressed_at = request_memory.suppressed_at,
                use_count = request_memory.use_count + 1,
                updated_at = excluded.updated_at,
                last_used_at = excluded.last_used_at",
            params![
                normalized_request,
                memory_scope,
                original_request,
                request_signature,
                intent_json_str,
                intent_command,
                response_text,
                truncate_text(response_mode, 32),
                truncate_text(source, 64),
                confidence.clamp(0.0, 1.0),
                now,
            ],
        )?;
    }
    Ok(())
}

pub fn get_request_memory(request_text: &str) -> Result<Option<RequestMemoryRecord>> {
    get_request_memory_scoped(None, request_text)
}

pub fn get_request_memory_scoped(
    memory_scope: Option<&str>,
    request_text: &str,
) -> Result<Option<RequestMemoryRecord>> {
    let Some((memory_scope, normalized_request)) =
        scoped_request_memory_storage_key(request_text, memory_scope)
    else {
        return Ok(None);
    };

    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(
            "SELECT normalized_request, memory_scope, original_request, request_signature, intent_json, intent_command,
                    response_text, response_mode, source, confidence,
                    positive_feedback_count, negative_feedback_count, last_feedback_at,
                    suppressed, suppressed_reason, suppressed_at,
                    use_count, created_at, updated_at, last_used_at
             FROM request_memory
             WHERE normalized_request = ?1
               AND memory_scope = ?2
               AND suppressed = 0
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![normalized_request, memory_scope])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(map_request_memory_row(row)?));
        }
    }
    Ok(None)
}

pub fn get_request_memory_by_signature(
    request_signature: &str,
    allowed_commands: &[&str],
) -> Result<Option<RequestMemoryRecord>> {
    get_request_memory_by_signature_scoped(None, request_signature, allowed_commands)
}

pub fn get_request_memory_by_signature_scoped(
    memory_scope: Option<&str>,
    request_signature: &str,
    allowed_commands: &[&str],
) -> Result<Option<RequestMemoryRecord>> {
    let request_signature = optional_truncated_signature(request_signature);
    if request_signature.is_none() || allowed_commands.is_empty() {
        return Ok(None);
    }
    let memory_scope = crate::db::normalize_memory_scope(memory_scope);

    let placeholders = (0..allowed_commands.len())
        .map(|idx| format!("?{}", idx + 3))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT normalized_request, memory_scope, original_request, request_signature, intent_json, intent_command,
                response_text, response_mode, source, confidence,
                positive_feedback_count, negative_feedback_count, last_feedback_at,
                suppressed, suppressed_reason, suppressed_at,
                use_count, created_at, updated_at, last_used_at
         FROM request_memory
         WHERE request_signature = ?1
           AND memory_scope = ?2
           AND suppressed = 0
           AND intent_command IN ({})
         ORDER BY
             (positive_feedback_count - negative_feedback_count) DESC,
             positive_feedback_count DESC,
             negative_feedback_count ASC,
             CASE
                 WHEN source LIKE 'api.chat.execution%' THEN 4
                 WHEN source = 'api.chat.deterministic' THEN 3
                 WHEN source IN ('api.chat.local', 'api.chat.local_cache') THEN 2
                 WHEN source IN ('api.chat.llm', 'api.chat') THEN 1
                 ELSE 0
             END DESC,
             use_count DESC,
             confidence DESC,
             last_used_at DESC
         LIMIT 1",
        placeholders
    );

    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = conn.prepare(&sql)?;
        let params = params_from_iter(
            std::iter::once(request_signature.unwrap())
                .chain(std::iter::once(memory_scope))
                .chain(
                    allowed_commands
                        .iter()
                        .map(|command| (*command).to_string()),
                ),
        );
        let mut rows = stmt.query(params)?;
        if let Some(row) = rows.next()? {
            return Ok(Some(map_request_memory_row(row)?));
        }
    }
    Ok(None)
}

pub fn list_request_memory_records(
    limit: i64,
    include_suppressed: bool,
) -> Result<Vec<RequestMemoryRecord>> {
    let limit = normalize_admin_limit(limit);
    let mut out = Vec::new();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let mut stmt = if include_suppressed {
            conn.prepare(
                "SELECT normalized_request, memory_scope, original_request, request_signature, intent_json, intent_command,
                        response_text, response_mode, source, confidence,
                        positive_feedback_count, negative_feedback_count, last_feedback_at,
                        suppressed, suppressed_reason, suppressed_at,
                        use_count, created_at, updated_at, last_used_at
                 FROM request_memory
                 ORDER BY suppressed DESC, last_used_at DESC
                 LIMIT ?1",
            )?
        } else {
            conn.prepare(
                "SELECT normalized_request, memory_scope, original_request, request_signature, intent_json, intent_command,
                        response_text, response_mode, source, confidence,
                        positive_feedback_count, negative_feedback_count, last_feedback_at,
                        suppressed, suppressed_reason, suppressed_at,
                        use_count, created_at, updated_at, last_used_at
                 FROM request_memory
                 WHERE suppressed = 0
                 ORDER BY last_used_at DESC
                 LIMIT ?1",
            )?
        };
        let rows = stmt.query_map(params![limit], map_request_memory_row)?;
        for row in rows.flatten() {
            out.push(row);
        }
    }
    Ok(out)
}

pub fn suppress_request_memory(request_text: &str, reason: Option<&str>) -> Result<bool> {
    suppress_request_memory_scoped(None, request_text, reason)
}

pub fn suppress_request_memory_scoped(
    memory_scope: Option<&str>,
    request_text: &str,
    reason: Option<&str>,
) -> Result<bool> {
    let Some((memory_scope, normalized_request)) =
        scoped_request_memory_storage_key(request_text, memory_scope)
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
            "UPDATE request_memory
             SET suppressed = 1,
                 suppressed_reason = COALESCE(?1, suppressed_reason),
                 suppressed_at = ?2,
                 updated_at = ?2
             WHERE normalized_request = ?3
               AND memory_scope = ?4",
            params![reason, now, normalized_request, memory_scope],
        )?;
        return Ok(rows > 0);
    }
    Ok(false)
}

pub fn restore_request_memory(request_text: &str) -> Result<bool> {
    restore_request_memory_scoped(None, request_text)
}

pub fn restore_request_memory_scoped(
    memory_scope: Option<&str>,
    request_text: &str,
) -> Result<bool> {
    let Some((memory_scope, normalized_request)) =
        scoped_request_memory_storage_key(request_text, memory_scope)
    else {
        return Ok(false);
    };
    let now = chrono::Utc::now().to_rfc3339();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let rows = conn.execute(
            "UPDATE request_memory
             SET suppressed = 0,
                 suppressed_reason = NULL,
                 suppressed_at = NULL,
                 updated_at = ?1
             WHERE normalized_request = ?2
               AND memory_scope = ?3",
            params![now, normalized_request, memory_scope],
        )?;
        return Ok(rows > 0);
    }
    Ok(false)
}

pub fn delete_request_memory(request_text: &str) -> Result<bool> {
    delete_request_memory_scoped(None, request_text)
}

pub fn delete_request_memory_scoped(
    memory_scope: Option<&str>,
    request_text: &str,
) -> Result<bool> {
    let Some((memory_scope, normalized_request)) =
        scoped_request_memory_storage_key(request_text, memory_scope)
    else {
        return Ok(false);
    };
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let rows = conn.execute(
            "DELETE FROM request_memory
             WHERE normalized_request = ?1
               AND memory_scope = ?2",
            params![normalized_request, memory_scope],
        )?;
        return Ok(rows > 0);
    }
    Ok(false)
}

pub fn record_request_memory_feedback(
    request_text: &str,
    response_text: &str,
    sentiment: &str,
) -> Result<bool> {
    record_request_memory_feedback_scoped(None, request_text, response_text, sentiment)
}

pub fn record_request_memory_feedback_scoped(
    memory_scope: Option<&str>,
    request_text: &str,
    response_text: &str,
    sentiment: &str,
) -> Result<bool> {
    let Some(sentiment) = normalize_feedback_signal(sentiment) else {
        return Ok(false);
    };
    let Some((memory_scope, normalized_request)) =
        scoped_request_memory_storage_key(request_text, memory_scope)
    else {
        return Ok(false);
    };
    let response_text = truncate_text(response_text, 4000);
    if response_text.is_empty() {
        return Ok(false);
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut lock = get_db_lock();
    if let Some(conn) = lock.as_mut() {
        let positive_delta = if sentiment == "positive" { 1 } else { 0 };
        let negative_delta = if sentiment == "negative" { 1 } else { 0 };
        let rows = conn.execute(
            "UPDATE request_memory
             SET positive_feedback_count = positive_feedback_count + ?1,
                 negative_feedback_count = negative_feedback_count + ?2,
                 last_feedback_at = ?3,
                 updated_at = ?3
             WHERE normalized_request = ?4
               AND memory_scope = ?5
               AND response_text = ?6",
            params![
                positive_delta,
                negative_delta,
                now,
                normalized_request,
                memory_scope,
                response_text
            ],
        )?;
        return Ok(rows > 0);
    }
    Ok(false)
}
