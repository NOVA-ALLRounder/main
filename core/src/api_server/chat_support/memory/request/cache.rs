use axum::Json;

use crate::api_server::ChatResponse;
use crate::{db, request_memory};

use super::persist::persist_chat_response_memory;
use super::policy::{
    allow_signature_command, allow_signature_response_mode, build_local_cached_response,
    request_cache_blocked, request_memory_response_fresh,
};

pub(crate) fn load_cached_request_response(
    memory_scope: Option<&str>,
    message: &str,
    command: &str,
) -> Option<String> {
    if request_cache_blocked(memory_scope, message, command) {
        return None;
    }

    if let Some(record) = db::get_request_memory_scoped(memory_scope, message)
        .ok()
        .flatten()
    {
        if let Some(response) = request_memory_response_fresh(&record, command) {
            return Some(response);
        }
    }

    let input_signature = request_memory::build_request_signature(message);
    if input_signature.is_empty() {
        return None;
    }

    let record =
        db::get_request_memory_by_signature_scoped(memory_scope, &input_signature, &[command])
            .ok()
            .flatten()?;
    if !allow_signature_response_mode(&record) {
        return None;
    }
    request_memory_response_fresh(&record, command)
}

pub(crate) fn build_local_chat_response(
    memory_scope: Option<&str>,
    message: &str,
    command: &str,
    response: String,
    source: &str,
) -> Json<ChatResponse> {
    persist_chat_response_memory(memory_scope, message, command, &response, source, 1.0);
    Json(ChatResponse {
        response,
        command: Some(command.to_string()),
        route_meta: None,
    })
}

pub(crate) fn load_local_cached_chat_response(
    memory_scope: Option<&str>,
    message: &str,
    command: &str,
) -> Option<Json<ChatResponse>> {
    if !allow_signature_command(command)
        && db::get_request_memory_scoped(memory_scope, message).is_err()
    {
        return None;
    }
    let response = load_cached_request_response(memory_scope, message, command)?;
    Some(Json(build_local_cached_response(
        memory_scope,
        message,
        command,
        &response,
    )))
}
