use rusqlite::{params, Connection};
use serde_json::Value;

use crate::db::{normalize_memory_scope, row_bool, truncate_text};
use crate::request_memory::{build_request_signature, normalize_request_text};

use super::execution::ExecutionMemoryRecord;
use super::request::RequestMemoryRecord;

pub(super) fn normalize_request_memory_key(input: &str) -> String {
    normalize_request_text(input)
}

pub(super) fn split_scoped_storage_key(storage_key: &str) -> (Option<&str>, &str) {
    if let Some((scope, logical_key)) = storage_key.split_once("::") {
        if !scope.trim().is_empty() && !logical_key.trim().is_empty() {
            return (Some(scope), logical_key);
        }
    }
    (None, storage_key)
}

pub(super) fn logical_key_from_storage(storage_key: &str, memory_scope: &str) -> String {
    let (stored_scope, logical_key) = split_scoped_storage_key(storage_key);
    if stored_scope == Some(memory_scope) {
        logical_key.to_string()
    } else {
        storage_key.to_string()
    }
}

pub(super) fn compose_scoped_storage_key(
    memory_scope: &str,
    logical_key: &str,
    max_len: usize,
) -> String {
    truncate_text(&format!("{}::{}", memory_scope, logical_key), max_len)
}

pub(super) fn scoped_request_memory_storage_key(
    request_text: &str,
    memory_scope: Option<&str>,
) -> Option<(String, String)> {
    let logical_key = normalize_request_memory_key(request_text);
    if logical_key.is_empty() {
        return None;
    }
    let memory_scope = normalize_memory_scope(memory_scope);
    let storage_key = compose_scoped_storage_key(&memory_scope, &logical_key, 1024);
    Some((memory_scope, storage_key))
}

pub(super) fn scoped_execution_params_key(
    params_key: &str,
    memory_scope: Option<&str>,
) -> Option<(String, String, String)> {
    let logical_key = truncate_text(params_key, 255);
    if logical_key.trim().is_empty() {
        return None;
    }
    let memory_scope = normalize_memory_scope(memory_scope);
    let storage_key = compose_scoped_storage_key(&memory_scope, &logical_key, 255);
    Some((memory_scope, logical_key, storage_key))
}

pub(super) fn request_memory_intent_command(intent_json: Option<&Value>) -> Option<String> {
    intent_json
        .and_then(|value| value.get("command"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| truncate_text(value, 64))
}

pub(super) fn request_memory_intent_command_from_raw(intent_json: Option<&str>) -> Option<String> {
    let raw = intent_json?.trim();
    if raw.is_empty() {
        return None;
    }
    let parsed: Value = serde_json::from_str(raw).ok()?;
    request_memory_intent_command(Some(&parsed))
}

pub(super) fn optional_truncated_signature(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(truncate_text(trimmed, 255))
    }
}

pub(super) fn map_request_memory_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<RequestMemoryRecord> {
    let stored_key = row.get::<_, String>(0)?;
    let memory_scope = row.get::<_, String>(1)?;
    Ok(RequestMemoryRecord {
        normalized_request: logical_key_from_storage(&stored_key, &memory_scope),
        memory_scope,
        original_request: row.get(2)?,
        request_signature: row.get(3).ok(),
        intent_json: row.get(4).ok(),
        intent_command: row.get(5).ok(),
        response_text: row.get(6).ok(),
        response_mode: row.get(7)?,
        source: row.get(8)?,
        confidence: row.get(9).unwrap_or(0.0),
        positive_feedback_count: row.get(10).unwrap_or(0),
        negative_feedback_count: row.get(11).unwrap_or(0),
        last_feedback_at: row.get(12).ok(),
        suppressed: row_bool(row, 13, false),
        suppressed_reason: row.get(14).ok(),
        suppressed_at: row.get(15).ok(),
        use_count: row.get(16).unwrap_or(0),
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
        last_used_at: row.get(19)?,
    })
}

pub(super) fn map_execution_memory_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ExecutionMemoryRecord> {
    let memory_scope = row.get::<_, String>(1)?;
    let stored_params_key = row.get::<_, String>(3)?;
    Ok(ExecutionMemoryRecord {
        id: row.get(0)?,
        memory_scope: memory_scope.clone(),
        intent_command: row.get(2)?,
        params_key: logical_key_from_storage(&stored_params_key, &memory_scope),
        params_json: row.get(4).ok(),
        request_signature: row.get(5).ok(),
        response_text: row.get(6)?,
        source: row.get(7)?,
        tool_path: row.get(8)?,
        freshness_ttl_seconds: row.get(9).unwrap_or(0),
        success: row.get::<_, i64>(10).unwrap_or(1) != 0,
        positive_feedback_count: row.get(11).unwrap_or(0),
        negative_feedback_count: row.get(12).unwrap_or(0),
        last_feedback_at: row.get(13).ok(),
        suppressed: row_bool(row, 14, false),
        suppressed_reason: row.get(15).ok(),
        suppressed_at: row.get(16).ok(),
        use_count: row.get(17).unwrap_or(0),
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
        last_used_at: row.get(20)?,
    })
}

pub(crate) fn backfill_request_memory_scope_keys(conn: &Connection) {
    let _ = conn.execute(
        "UPDATE request_memory
         SET memory_scope = 'global'
         WHERE memory_scope IS NULL
            OR TRIM(memory_scope) = ''",
        [],
    );

    let mut stmt =
        match conn.prepare("SELECT rowid, normalized_request, memory_scope FROM request_memory") {
            Ok(stmt) => stmt,
            Err(error) => {
                eprintln!("Failed to prepare request_memory scope backfill: {}", error);
                return;
            }
        };
    let rows = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    }) {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!(
                "Failed to query request_memory scope backfill rows: {}",
                error
            );
            return;
        }
    };

    for row in rows.flatten() {
        let (rowid, stored_key, raw_scope) = row;
        let memory_scope = normalize_memory_scope(Some(&raw_scope));
        let logical_key = logical_key_from_storage(&stored_key, &memory_scope);
        let scoped_key = compose_scoped_storage_key(&memory_scope, &logical_key, 1024);
        if scoped_key == stored_key && memory_scope == raw_scope {
            continue;
        }
        if let Err(error) = conn.execute(
            "UPDATE request_memory
             SET normalized_request = ?1,
                 memory_scope = ?2
             WHERE rowid = ?3",
            params![scoped_key, memory_scope, rowid],
        ) {
            eprintln!(
                "Failed to backfill request_memory scope key row {}: {}",
                rowid, error
            );
        }
    }
}

pub(crate) fn backfill_execution_memory_scope_keys(conn: &Connection) {
    let _ = conn.execute(
        "UPDATE execution_memory
         SET memory_scope = 'global'
         WHERE memory_scope IS NULL
            OR TRIM(memory_scope) = ''",
        [],
    );

    let mut stmt =
        match conn.prepare("SELECT rowid, params_key, memory_scope FROM execution_memory") {
            Ok(stmt) => stmt,
            Err(error) => {
                eprintln!(
                    "Failed to prepare execution_memory scope backfill: {}",
                    error
                );
                return;
            }
        };
    let rows = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    }) {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!(
                "Failed to query execution_memory scope backfill rows: {}",
                error
            );
            return;
        }
    };

    for row in rows.flatten() {
        let (rowid, stored_key, raw_scope) = row;
        let memory_scope = normalize_memory_scope(Some(&raw_scope));
        let logical_key = logical_key_from_storage(&stored_key, &memory_scope);
        let scoped_key = compose_scoped_storage_key(&memory_scope, &logical_key, 255);
        if scoped_key == stored_key && memory_scope == raw_scope {
            continue;
        }
        if let Err(error) = conn.execute(
            "UPDATE execution_memory
             SET params_key = ?1,
                 memory_scope = ?2
             WHERE rowid = ?3",
            params![scoped_key, memory_scope, rowid],
        ) {
            eprintln!(
                "Failed to backfill execution_memory scope key row {}: {}",
                rowid, error
            );
        }
    }
}

pub(crate) fn backfill_request_memory_cache_columns(conn: &Connection) {
    let mut stmt = match conn.prepare(
        "SELECT normalized_request, original_request, intent_json
         FROM request_memory
         WHERE request_signature IS NULL
            OR TRIM(request_signature) = ''
            OR intent_command IS NULL
            OR TRIM(intent_command) = ''",
    ) {
        Ok(stmt) => stmt,
        Err(error) => {
            eprintln!("Failed to prepare request_memory cache backfill: {}", error);
            return;
        }
    };

    let rows = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2).ok().flatten(),
        ))
    }) {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!(
                "Failed to query request_memory cache backfill rows: {}",
                error
            );
            return;
        }
    };

    for row in rows.flatten() {
        let (normalized_request, original_request, intent_json) = row;
        let signature = optional_truncated_signature(&build_request_signature(&original_request));
        let intent_command = request_memory_intent_command_from_raw(intent_json.as_deref());
        if let Err(error) = conn.execute(
            "UPDATE request_memory
             SET request_signature = ?1,
                 intent_command = ?2
             WHERE normalized_request = ?3",
            params![signature, intent_command, normalized_request],
        ) {
            eprintln!(
                "Failed to backfill request_memory cache columns for {}: {}",
                original_request, error
            );
        }
    }
}

pub(super) fn normalize_feedback_signal(sentiment: &str) -> Option<&'static str> {
    match sentiment.trim().to_ascii_lowercase().as_str() {
        "positive" | "up" | "like" => Some("positive"),
        "negative" | "down" | "dislike" => Some("negative"),
        _ => None,
    }
}
