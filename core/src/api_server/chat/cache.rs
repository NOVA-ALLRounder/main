use axum::Json;
use serde_json::Value;

use crate::api_server::chat_support::{
    chat_ops_outcome_from_response, execution_recently_reasked, load_cached_execution_response,
    load_cached_request_response, request_memory_response_mode, request_prefers_fresh_data,
    request_recently_reasked, respond_chat, run_ai_digest_chat_response, ChatOpsFlags,
};
use crate::api_server::{ChatRequest, ChatResponse};
use crate::db;

pub(crate) fn apply_freshness_flags(
    message: &str,
    command: &str,
    intent: &Value,
    flags: &mut ChatOpsFlags,
    memory_scope: Option<&str>,
) {
    flags.freshness_bypassed = request_prefers_fresh_data(message, command)
        || request_recently_reasked(memory_scope, message, command)
        || execution_recently_reasked(memory_scope, command, intent);
}

pub(crate) fn handle_cached_chat_response(
    req: &ChatRequest,
    memory_scope: Option<&str>,
    message: &str,
    command: &str,
    intent: &Value,
    confidence: f64,
    ops_flags: ChatOpsFlags,
) -> Option<Json<ChatResponse>> {
    if let Some(cached_response) =
        load_cached_execution_response(memory_scope, message, command, intent)
    {
        if let Err(e) = db::insert_chat_message("assistant", &cached_response) {
            eprintln!("Failed to save cached execution chat: {}", e);
        }
        let _ = db::upsert_request_memory_scoped(
            memory_scope,
            message,
            Some(intent),
            Some(&cached_response),
            request_memory_response_mode(command),
            "api.chat.execution_cache",
            confidence,
        );
        let mut flags = ops_flags;
        flags.execution_memory_hit = true;
        return Some(respond_chat(
            req,
            memory_scope,
            message,
            ChatResponse {
                response: cached_response,
                command: response_command(command),
                route_meta: None,
            },
            "execution_memory",
            "success",
            Some(confidence),
            flags,
            None,
        ));
    }

    if let Some(cached_response) = load_cached_request_response(memory_scope, message, command) {
        if let Err(e) = db::insert_chat_message("assistant", &cached_response) {
            eprintln!("Failed to save cached assistant chat: {}", e);
        }
        let _ = db::upsert_request_memory_scoped(
            memory_scope,
            message,
            Some(intent),
            Some(&cached_response),
            request_memory_response_mode(command),
            "api.chat.response_cache",
            confidence,
        );
        let mut flags = ops_flags;
        flags.request_memory_hit = true;
        return Some(respond_chat(
            req,
            memory_scope,
            message,
            ChatResponse {
                response: cached_response,
                command: response_command(command),
                route_meta: None,
            },
            "request_memory",
            "success",
            Some(confidence),
            flags,
            None,
        ));
    }

    None
}

pub(crate) async fn handle_auto_ai_digest_fallback(
    req: &ChatRequest,
    memory_scope: Option<&str>,
    message: &str,
    request_text: &str,
    confidence: f64,
    mut flags: ChatOpsFlags,
    note: &'static str,
) -> Json<ChatResponse> {
    flags.ai_digest_used = true;
    let response = run_ai_digest_chat_response(request_text).await;
    let outcome = chat_ops_outcome_from_response(&response.response);
    respond_chat(
        req,
        memory_scope,
        message,
        response,
        "ai_digest_auto",
        outcome,
        Some(confidence),
        flags,
        Some(note),
    )
}

fn response_command(command: &str) -> Option<String> {
    if command == "unknown" {
        None
    } else {
        Some(command.to_string())
    }
}
