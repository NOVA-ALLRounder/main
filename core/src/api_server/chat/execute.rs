use axum::Json;
use serde_json::Value;

use crate::api_server::chat_execution::execute_chat_command;
use crate::api_server::chat_support::{
    chat_ops_outcome_from_response, chat_response_memory_source, persist_execution_memory,
    request_memory_response_mode, request_memory_should_store, respond_chat, ChatOpsFlags,
};
use crate::api_server::{ChatRequest, ChatResponse};
use crate::db;

pub(crate) async fn execute_and_respond(
    req: &ChatRequest,
    memory_scope: Option<&str>,
    message: &str,
    command: &str,
    intent: &Value,
    confidence: f64,
    route_kind: &str,
    ops_flags: ChatOpsFlags,
) -> Json<ChatResponse> {
    let response = execute_chat_command(command, intent, message).await;

    if let Err(e) = db::insert_chat_message("assistant", &response) {
        eprintln!("Failed to save AI chat: {}", e);
    }

    persist_execution_memory(
        memory_scope,
        message,
        command,
        intent,
        &response,
        "api.chat.execution",
    );

    if request_memory_should_store(command, confidence, &response) {
        let _ = db::upsert_request_memory_scoped(
            memory_scope,
            message,
            Some(intent),
            Some(&response),
            request_memory_response_mode(command),
            chat_response_memory_source(intent),
            confidence,
        );
    }

    let final_response = ChatResponse {
        response: response.clone(),
        command: response_command(command),
        route_meta: None,
    };

    let effective_route_kind = if command == "unknown" {
        "unknown"
    } else {
        route_kind
    };
    let outcome = chat_ops_outcome_from_response(&response);
    respond_chat(
        req,
        memory_scope,
        message,
        final_response,
        effective_route_kind,
        outcome,
        Some(confidence),
        ops_flags,
        None,
    )
}

fn response_command(command: &str) -> Option<String> {
    if command == "unknown" {
        None
    } else {
        Some(command.to_string())
    }
}
