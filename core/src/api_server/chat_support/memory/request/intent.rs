use serde_json::Value;

use crate::{db, request_memory};

use super::super::policy::{feedback_allows_response_reuse, request_memory_min_confidence};
use super::policy::allow_signature_command;

pub(crate) fn request_memory_intent_for_feedback(
    memory_scope: Option<&str>,
    request_text: &str,
) -> Option<Value> {
    let record = db::get_request_memory_scoped(memory_scope, request_text)
        .ok()
        .flatten()?;
    let raw = record.intent_json?;
    serde_json::from_str(&raw).ok()
}

pub(crate) fn load_cached_request_intent(
    memory_scope: Option<&str>,
    message: &str,
) -> Option<(Value, f64)> {
    if let Some(record) = db::get_request_memory_scoped(memory_scope, message)
        .ok()
        .flatten()
    {
        if record.confidence < request_memory_min_confidence() {
            return None;
        }
        if !feedback_allows_response_reuse(
            record.positive_feedback_count,
            record.negative_feedback_count,
        ) {
            return None;
        }
        let raw = record.intent_json?;
        let parsed: Value = serde_json::from_str(&raw).ok()?;
        return Some((parsed, record.confidence));
    }

    let input_signature = request_memory::build_request_signature(message);
    if input_signature.is_empty() {
        return None;
    }

    let allowed_commands = ["calendar_today", "calendar_week", "gmail_list"];
    let record = db::get_request_memory_by_signature_scoped(
        memory_scope,
        &input_signature,
        &allowed_commands,
    )
    .ok()
    .flatten()?;
    if record.confidence < request_memory_min_confidence() {
        return None;
    }
    if !feedback_allows_response_reuse(
        record.positive_feedback_count,
        record.negative_feedback_count,
    ) {
        return None;
    }
    let raw = record.intent_json?;
    let parsed: Value = serde_json::from_str(&raw).ok()?;
    let command = parsed.get("command").and_then(|v| v.as_str()).unwrap_or("");
    if !allow_signature_command(command) {
        return None;
    }
    Some((parsed, record.confidence))
}

pub(crate) fn load_deterministic_chat_intent(message: &str) -> Option<(Value, f64)> {
    let intent = crate::deterministic_intent::classify_chat_intent(message)?;
    let confidence = intent
        .get("confidence")
        .and_then(|value| value.as_f64())
        .unwrap_or(1.0);
    Some((intent, confidence))
}
