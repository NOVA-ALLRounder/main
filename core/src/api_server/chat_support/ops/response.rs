use axum::{extract::State, Json};

use crate::ai_digest;
use crate::api_server::{handle_chat, AppState, ChatRequest, ChatResponse, ChatRouteMeta};

use super::{telemetry::record_chat_response_effects, ChatOpsFlags};

fn build_chat_route_meta(
    memory_scope: Option<&str>,
    route_kind: &str,
    outcome: &str,
    confidence: Option<f64>,
    flags: ChatOpsFlags,
    note: Option<&str>,
) -> ChatRouteMeta {
    ChatRouteMeta {
        route_kind: route_kind.to_string(),
        outcome: outcome.to_string(),
        confidence,
        note: note.map(|value| value.to_string()),
        memory_scope: memory_scope.map(|value| value.to_string()),
        freshness_bypassed: flags.freshness_bypassed,
        intent_memory_hit: flags.intent_memory_hit,
        request_memory_hit: flags.request_memory_hit,
        execution_memory_hit: flags.execution_memory_hit,
        deterministic_used: flags.deterministic_used,
        llm_used: flags.llm_used,
        ai_digest_used: flags.ai_digest_used,
        local_only: flags.local_only,
    }
}

pub(crate) fn respond_chat(
    req: &ChatRequest,
    memory_scope: Option<&str>,
    message: &str,
    mut response: ChatResponse,
    route_kind: &str,
    outcome: &str,
    confidence: Option<f64>,
    flags: ChatOpsFlags,
    note: Option<&str>,
) -> Json<ChatResponse> {
    response.route_meta = Some(build_chat_route_meta(
        memory_scope,
        route_kind,
        outcome,
        confidence,
        flags,
        note,
    ));
    record_chat_response_effects(
        req,
        memory_scope,
        message,
        &response,
        route_kind,
        outcome,
        confidence,
        flags,
        note,
    );
    Json(response)
}

pub async fn process_chat_request(state: AppState, req: ChatRequest) -> ChatResponse {
    handle_chat(State(state), Json(req)).await.0
}

pub(crate) async fn run_ai_digest_chat_response(request_text: &str) -> ChatResponse {
    let response = match ai_digest::trigger_program_webhook_human_summary(request_text, None).await
    {
        Ok(summary) => summary,
        Err(e) => format!("❌ News Digest 트리거 실패: {}", e),
    };
    ChatResponse {
        response,
        command: Some("ai_digest_program".to_string()),
        route_meta: None,
    }
}
