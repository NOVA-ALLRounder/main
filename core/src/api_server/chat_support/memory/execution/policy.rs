use serde_json::{json, Value};

use super::super::policy::{request_memory_min_confidence, request_memory_response_cache_ttl_secs};
use super::super::request::request_memory_response_is_successful;

pub(crate) fn parse_u32_param(
    value: Option<&Value>,
    default_value: u32,
    min_value: u32,
    max_value: u32,
) -> u32 {
    value
        .and_then(|raw| {
            raw.as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .or_else(|| raw.as_str().and_then(|v| v.trim().parse::<u32>().ok()))
        })
        .unwrap_or(default_value)
        .clamp(min_value, max_value)
}

pub(crate) fn execution_memory_supported(command: &str) -> bool {
    matches!(command, "calendar_today" | "calendar_week" | "gmail_list")
}

pub(super) fn execution_memory_tool_path(command: &str) -> &'static str {
    match command {
        "calendar_today" => "integrations.calendar.list_today",
        "calendar_week" => "integrations.calendar.list_week",
        "gmail_list" => "integrations.gmail.list_messages",
        _ => "api.chat",
    }
}

pub(super) fn execution_memory_ttl_secs(command: &str) -> Option<i64> {
    request_memory_response_cache_ttl_secs(command, "ttl_response_signature")
}

pub(crate) fn execution_memory_params(command: &str, intent: &Value) -> Option<(String, Value)> {
    match command {
        "gmail_list" => {
            let count = parse_u32_param(intent["params"].get("count"), 5, 1, 20);
            Some((format!("count={}", count), json!({ "count": count })))
        }
        "calendar_today" | "calendar_week" => Some(("default".to_string(), json!({}))),
        _ => None,
    }
}

pub(super) fn execution_memory_should_store(
    command: &str,
    response: &str,
    confidence: f64,
) -> bool {
    execution_memory_supported(command)
        && request_memory_response_is_successful(response)
        && confidence >= request_memory_min_confidence()
        && execution_memory_ttl_secs(command).unwrap_or(0) > 0
}
