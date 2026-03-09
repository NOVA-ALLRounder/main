use serde_json::{json, Value};

use super::persist::persist_chat_response_memory;
use crate::api_server::ChatResponse;
use crate::db;

use super::super::freshness::{request_prefers_fresh_data, request_recently_reasked};
use super::super::policy::{
    feedback_allows_response_reuse, request_memory_min_confidence,
    request_memory_response_cache_ttl_secs, request_memory_response_mode_supports_signature,
    request_memory_signature_cache_supported,
};

pub(crate) fn request_memory_response_mode(command: &str) -> &'static str {
    match command {
        "help" | "help_local" | "greeting_local" => "reusable_response",
        "calendar_today" | "calendar_week" | "gmail_list" => "ttl_response_signature",
        "system_status" => "ttl_response_exact",
        _ => "intent_only",
    }
}

pub(crate) fn request_memory_response_is_successful(response: &str) -> bool {
    let trimmed = response.trim();
    if trimmed.is_empty() {
        return false;
    }

    !trimmed.starts_with("❌")
        && !trimmed.starts_with("⚠️")
        && !trimmed.starts_with("⛔")
        && !trimmed.starts_with("🤔")
        && !trimmed.starts_with("❓")
}

pub(crate) fn request_memory_should_store(command: &str, confidence: f64, response: &str) -> bool {
    !command.trim().is_empty()
        && !command.eq_ignore_ascii_case("unknown")
        && confidence >= request_memory_min_confidence()
        && request_memory_response_is_successful(response)
}

pub(super) fn request_memory_static_intent(command: &str) -> Value {
    json!({
        "command": command,
        "params": {},
        "confidence": 1.0
    })
}

pub(super) fn request_memory_response_fresh(
    record: &db::RequestMemoryRecord,
    command: &str,
) -> Option<String> {
    if record.intent_command.as_deref() != Some(command) {
        return None;
    }
    if !feedback_allows_response_reuse(
        record.positive_feedback_count,
        record.negative_feedback_count,
    ) {
        return None;
    }

    let response = record
        .response_text
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();

    let ttl_secs = request_memory_response_cache_ttl_secs(command, &record.response_mode)?;
    if ttl_secs <= 0 {
        return None;
    }

    if record.response_mode == "reusable_response" {
        return Some(response);
    }

    let updated_at = chrono::DateTime::parse_from_rfc3339(&record.updated_at).ok()?;
    let age_secs = chrono::Utc::now()
        .signed_duration_since(updated_at.with_timezone(&chrono::Utc))
        .num_seconds();
    if age_secs < 0 || age_secs > ttl_secs {
        return None;
    }

    Some(response)
}

pub(super) fn allow_signature_response_mode(record: &db::RequestMemoryRecord) -> bool {
    request_memory_response_mode_supports_signature(&record.response_mode)
}

pub(super) fn allow_signature_command(command: &str) -> bool {
    request_memory_signature_cache_supported(command)
}

pub(super) fn request_cache_blocked(
    memory_scope: Option<&str>,
    message: &str,
    command: &str,
) -> bool {
    request_prefers_fresh_data(message, command)
        || request_recently_reasked(memory_scope, message, command)
}

pub(super) fn build_local_cached_response(
    memory_scope: Option<&str>,
    message: &str,
    command: &str,
    response: &str,
) -> ChatResponse {
    persist_chat_response_memory(
        memory_scope,
        message,
        command,
        response,
        "api.chat.local_cache",
        1.0,
    );
    ChatResponse {
        response: response.to_string(),
        command: Some(command.to_string()),
        route_meta: None,
    }
}
