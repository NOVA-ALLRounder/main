use super::*;
use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};

pub(super) fn seed_request_memory(
    seed: &RequestMemorySeed,
    memory_scope: Option<&str>,
) -> Result<()> {
    let intent = json!({
        "command": seed.command,
        "params": seed.params,
        "confidence": seed.confidence,
    });
    let response_mode = seed
        .response_mode
        .clone()
        .unwrap_or_else(|| infer_request_memory_response_mode(&seed.command).to_string());
    crate::db::upsert_request_memory_scoped(
        memory_scope,
        &seed.request_text,
        Some(&intent),
        Some(&seed.response_text),
        &response_mode,
        &seed.source,
        seed.confidence,
    )
    .context("failed to seed request memory")
}

pub(super) fn seed_request_memory_with_scope(seed: &ScopedRequestMemorySeed) -> Result<()> {
    let intent = json!({
        "command": seed.command,
        "params": seed.params,
        "confidence": seed.confidence,
    });
    let response_mode = seed
        .response_mode
        .clone()
        .unwrap_or_else(|| infer_request_memory_response_mode(&seed.command).to_string());
    let scope = seed.scope.memory_scope();
    crate::db::upsert_request_memory_scoped(
        scope.as_deref(),
        &seed.request_text,
        Some(&intent),
        Some(&seed.response_text),
        &response_mode,
        &seed.source,
        seed.confidence,
    )
    .context("failed to seed scoped request memory")
}

pub(super) fn seed_execution_memory(
    seed: &ExecutionMemorySeed,
    memory_scope: Option<&str>,
) -> Result<()> {
    let params_key = infer_execution_params_key(&seed.command, &seed.params)?;
    let tool_path = seed
        .tool_path
        .clone()
        .unwrap_or_else(|| infer_execution_tool_path(&seed.command).to_string());
    let request_signature = crate::request_memory::build_request_signature(&seed.original_request);
    let signature = if request_signature.trim().is_empty() {
        None
    } else {
        Some(request_signature.as_str())
    };
    crate::db::upsert_execution_memory_scoped(
        memory_scope,
        &seed.command,
        &params_key,
        Some(&seed.params),
        signature,
        &seed.response_text,
        seed.ttl_seconds
            .unwrap_or_else(|| infer_execution_ttl(&seed.command)),
        &seed.source,
        &tool_path,
    )
    .context("failed to seed execution memory")
}

fn infer_request_memory_response_mode(command: &str) -> &'static str {
    match command {
        "help" | "help_local" | "greeting_local" => "reusable_response",
        "calendar_today" | "calendar_week" | "gmail_list" => "ttl_response_signature",
        "system_status" => "ttl_response_exact",
        _ => "intent_only",
    }
}

fn infer_execution_tool_path(command: &str) -> &'static str {
    match command {
        "calendar_today" => "integrations.calendar.list_today",
        "calendar_week" => "integrations.calendar.list_week",
        "gmail_list" => "integrations.gmail.list_messages",
        _ => "launch.eval",
    }
}

fn infer_execution_ttl(command: &str) -> i64 {
    match command {
        "gmail_list" => 20,
        "calendar_today" => 30,
        "calendar_week" => 120,
        "system_status" => 10,
        _ => 60,
    }
}

pub(super) fn infer_execution_params_key(command: &str, params: &Value) -> Result<String> {
    match command {
        "gmail_list" => {
            let count = params
                .get("count")
                .and_then(|value| value.as_u64())
                .or_else(|| {
                    params
                        .get("count")
                        .and_then(|value| value.as_str())
                        .and_then(|value| value.parse::<u64>().ok())
                })
                .unwrap_or(5)
                .clamp(1, 20);
            Ok(format!("count={}", count))
        }
        "calendar_today" | "calendar_week" => Ok("default".to_string()),
        _ => Err(anyhow!(
            "unsupported execution memory command '{}'",
            command
        )),
    }
}

pub(super) fn build_memory_scope(
    channel: Option<&str>,
    chat_type: Option<&str>,
    sender: Option<&str>,
) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(channel) = normalize_scope_part(channel) {
        parts.push(format!("channel_{}", channel));
    }
    if let Some(chat_type) = normalize_scope_part(chat_type) {
        parts.push(format!("type_{}", chat_type));
    }
    if let Some(sender) = normalize_scope_part(sender) {
        parts.push(format!("sender_{}", sender));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("__"))
    }
}

fn normalize_scope_part(value: Option<&str>) -> Option<String> {
    let normalized = value
        .unwrap_or_default()
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}
