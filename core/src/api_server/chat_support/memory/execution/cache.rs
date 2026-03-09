use serde_json::Value;

use crate::db;

use super::super::freshness::{
    execution_memory_recent_chat_reask, repeat_request_fresh_window_secs,
    request_prefers_fresh_data, request_recently_reasked,
};
use super::super::policy::feedback_allows_response_reuse;
use super::persist::persist_execution_memory;
use super::policy::{
    execution_memory_params, execution_memory_supported, execution_memory_ttl_secs,
};

fn execution_memory_response_fresh(
    record: &db::ExecutionMemoryRecord,
    command: &str,
) -> Option<String> {
    if !record.success || record.intent_command != command {
        return None;
    }
    if !feedback_allows_response_reuse(
        record.positive_feedback_count,
        record.negative_feedback_count,
    ) {
        return None;
    }

    let response = record.response_text.trim();
    if response.is_empty() {
        return None;
    }

    let configured_ttl = execution_memory_ttl_secs(command).unwrap_or(record.freshness_ttl_seconds);
    let ttl_secs = if record.freshness_ttl_seconds > 0 {
        configured_ttl.min(record.freshness_ttl_seconds)
    } else {
        configured_ttl
    };
    if ttl_secs <= 0 {
        return None;
    }

    let updated_at = chrono::DateTime::parse_from_rfc3339(&record.updated_at).ok()?;
    let age_secs = chrono::Utc::now()
        .signed_duration_since(updated_at.with_timezone(&chrono::Utc))
        .num_seconds();
    if age_secs < 0 || age_secs > ttl_secs {
        return None;
    }

    Some(response.to_string())
}

pub(crate) fn load_cached_execution_response(
    memory_scope: Option<&str>,
    message: &str,
    command: &str,
    intent: &Value,
) -> Option<String> {
    if request_prefers_fresh_data(message, command)
        || request_recently_reasked(memory_scope, message, command)
    {
        return None;
    }

    if !execution_memory_supported(command) {
        return None;
    }

    let (params_key, _) = execution_memory_params(command, intent)?;
    let record = db::get_execution_memory_scoped(memory_scope, command, &params_key)
        .ok()
        .flatten()?;
    if let Some(window_secs) = repeat_request_fresh_window_secs() {
        if execution_memory_recent_chat_reask(&record, command, window_secs) {
            return None;
        }
    }

    let cached = execution_memory_response_fresh(&record, command)?;
    persist_execution_memory(
        memory_scope,
        message,
        command,
        intent,
        &cached,
        "api.chat.execution_cache",
    );
    Some(cached)
}
