use std::collections::HashSet;

use crate::{db, request_memory};

use super::execution;
use super::policy::{
    command_supports_live_refresh, feedback_allows_response_reuse,
    request_memory_signature_cache_supported,
};

pub(crate) fn request_prefers_fresh_data(message: &str, command: &str) -> bool {
    if !command_supports_live_refresh(command) {
        return false;
    }

    let normalized = request_memory::normalize_request_text(message);
    if normalized.is_empty() {
        return false;
    }

    let tokens = normalized.split_whitespace().collect::<HashSet<_>>();
    let token_hit = [
        "새로고침",
        "refresh",
        "realtime",
        "실시간",
        "최신으로",
        "지금",
        "방금",
    ]
    .iter()
    .any(|token| tokens.contains(token));
    let phrase_hit = ["real time", "latest status", "up to date"]
        .iter()
        .any(|phrase| normalized.contains(phrase));

    token_hit || phrase_hit || (tokens.contains("다시") && tokens.contains("확인"))
}

pub(super) fn repeat_request_fresh_window_secs() -> Option<i64> {
    std::env::var("ALLVIA_REPEAT_REQUEST_FRESH_WINDOW_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .or(Some(15))
        .filter(|window| *window > 0)
}

fn timestamp_is_within_window(timestamp: &str, window_secs: i64) -> bool {
    let recorded_at = match chrono::DateTime::parse_from_rfc3339(timestamp) {
        Ok(value) => value.with_timezone(&chrono::Utc),
        Err(_) => return false,
    };
    let age_secs = chrono::Utc::now()
        .signed_duration_since(recorded_at)
        .num_seconds();
    age_secs >= 0 && age_secs <= window_secs
}

fn chat_memory_source_allows_repeat_bypass(source: &str) -> bool {
    source.starts_with("api.chat.")
}

fn request_memory_recent_chat_reask(
    record: &db::RequestMemoryRecord,
    command: &str,
    window_secs: i64,
) -> bool {
    record.intent_command.as_deref() == Some(command)
        && chat_memory_source_allows_repeat_bypass(&record.source)
        && feedback_allows_response_reuse(
            record.positive_feedback_count,
            record.negative_feedback_count,
        )
        && timestamp_is_within_window(&record.last_used_at, window_secs)
}

pub(super) fn execution_memory_recent_chat_reask(
    record: &db::ExecutionMemoryRecord,
    command: &str,
    window_secs: i64,
) -> bool {
    record.intent_command == command
        && chat_memory_source_allows_repeat_bypass(&record.source)
        && feedback_allows_response_reuse(
            record.positive_feedback_count,
            record.negative_feedback_count,
        )
        && timestamp_is_within_window(&record.last_used_at, window_secs)
}

pub(crate) fn request_recently_reasked(
    memory_scope: Option<&str>,
    message: &str,
    command: &str,
) -> bool {
    if !command_supports_live_refresh(command) {
        return false;
    }
    let Some(window_secs) = repeat_request_fresh_window_secs() else {
        return false;
    };

    if let Some(record) = db::get_request_memory_scoped(memory_scope, message)
        .ok()
        .flatten()
    {
        if request_memory_recent_chat_reask(&record, command, window_secs) {
            return true;
        }
    }

    if !request_memory_signature_cache_supported(command) {
        return false;
    }

    let input_signature = request_memory::build_request_signature(message);
    if input_signature.is_empty() {
        return false;
    }

    db::get_request_memory_by_signature_scoped(memory_scope, &input_signature, &[command])
        .ok()
        .flatten()
        .is_some_and(|record| request_memory_recent_chat_reask(&record, command, window_secs))
}

pub(crate) fn execution_recently_reasked(
    memory_scope: Option<&str>,
    command: &str,
    intent: &serde_json::Value,
) -> bool {
    if !execution::execution_memory_supported(command) {
        return false;
    }
    let Some(window_secs) = repeat_request_fresh_window_secs() else {
        return false;
    };
    let Some((params_key, _)) = execution::execution_memory_params(command, intent) else {
        return false;
    };
    db::get_execution_memory_scoped(memory_scope, command, &params_key)
        .ok()
        .flatten()
        .is_some_and(|record| execution_memory_recent_chat_reask(&record, command, window_secs))
}
