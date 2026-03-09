use serde_json::Value;

use crate::db;

use super::policy::{
    request_memory_response_mode, request_memory_should_store, request_memory_static_intent,
};

pub(crate) fn persist_chat_response_memory(
    memory_scope: Option<&str>,
    message: &str,
    command: &str,
    response: &str,
    source: &str,
    confidence: f64,
) {
    if let Err(e) = db::insert_chat_message("assistant", response) {
        eprintln!("Failed to save assistant chat: {}", e);
    }

    if request_memory_should_store(command, confidence, response) {
        let intent = request_memory_static_intent(command);
        let _ = db::upsert_request_memory_scoped(
            memory_scope,
            message,
            Some(&intent),
            Some(response),
            request_memory_response_mode(command),
            source,
            confidence,
        );
    }
}

pub(crate) fn chat_response_memory_source(intent: &Value) -> &'static str {
    match intent.get("source").and_then(|value| value.as_str()) {
        Some("deterministic") => "api.chat.deterministic",
        _ => "api.chat.llm",
    }
}
