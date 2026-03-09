use axum::Json;

use crate::api_server::chat_support::{
    chat_feedback_memory_scope, execution_memory_params, execution_memory_supported,
    request_memory_intent_for_feedback,
};
use crate::db;

use super::{ChatFeedbackRequest, ChatFeedbackResponse};

pub(crate) async fn handle_chat_feedback(
    Json(req): Json<ChatFeedbackRequest>,
) -> Json<ChatFeedbackResponse> {
    let sentiment = req.sentiment.trim().to_ascii_lowercase();
    if !matches!(sentiment.as_str(), "positive" | "negative") {
        return Json(ChatFeedbackResponse {
            ok: false,
            request_memory_updated: false,
            execution_memory_updated: false,
            reuse_suppressed: false,
        });
    }

    let memory_scope = chat_feedback_memory_scope(&req);
    let memory_scope_ref = memory_scope.as_deref();
    let request_memory_updated = db::record_request_memory_feedback_scoped(
        memory_scope_ref,
        &req.request_text,
        &req.response_text,
        &sentiment,
    )
    .unwrap_or(false);

    let intent = request_memory_intent_for_feedback(memory_scope_ref, &req.request_text);
    let command = req
        .command
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            intent
                .as_ref()
                .and_then(|value| value.get("command"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        });

    let execution_memory_updated = if let (Some(command), Some(intent)) = (command, intent) {
        if execution_memory_supported(&command) {
            if let Some((params_key, _)) = execution_memory_params(&command, &intent) {
                db::record_execution_memory_feedback_scoped(
                    memory_scope_ref,
                    &command,
                    &params_key,
                    &req.response_text,
                    &sentiment,
                )
                .unwrap_or(false)
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    Json(ChatFeedbackResponse {
        ok: request_memory_updated || execution_memory_updated,
        request_memory_updated,
        execution_memory_updated,
        reuse_suppressed: sentiment == "negative"
            && (request_memory_updated || execution_memory_updated),
    })
}
