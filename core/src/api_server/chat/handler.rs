use axum::{extract::State, Json};

use crate::api_server::chat_support::{
    chat_ops_outcome_from_response, chat_request_memory_scope, respond_chat,
    run_ai_digest_chat_response, ChatOpsFlags,
};
use crate::api_server::AppState;
use crate::{ai_digest, chat_sanitize, db};

use super::cache::{
    apply_freshness_flags, handle_auto_ai_digest_fallback, handle_cached_chat_response,
};
use super::commands::{
    handle_local_chat_command, handle_system_chat_command, handle_vision_chat_command,
};
use super::demo::handle_demo_vision_command;
use super::execute::execute_and_respond;
use super::intent::{resolve_chat_intent, ResolvedChatIntent};
use super::{ChatRequest, ChatResponse};

pub(crate) async fn handle_chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Json<ChatResponse> {
    let memory_scope = chat_request_memory_scope(&req);
    let memory_scope_ref = memory_scope.as_deref();
    let gate = crate::chat_gate::ChatGateConfig::from_env();
    let gate_ctx = crate::chat_gate::ChatGateContext {
        channel: req.channel.clone(),
        chat_type: req.chat_type.clone(),
        sender: req.sender.clone(),
        mentioned: req.mentioned,
    };
    if !gate.is_allowed(&gate_ctx) {
        return respond_chat(
            &req,
            memory_scope_ref,
            &req.message,
            ChatResponse {
                response: "⛔️ 이 채널에서는 현재 요청을 처리할 수 없습니다.".to_string(),
                command: None,
                route_meta: None,
            },
            "gate_blocked",
            "blocked",
            None,
            ChatOpsFlags::default(),
            Some("chat gate policy blocked this channel"),
        );
    }

    let sanitized = chat_sanitize::sanitize_chat_input(&req.message);
    if !sanitized.flags.is_empty() {
        eprintln!("⚠️ Chat sanitize flags: {:?}", sanitized.flags);
    }
    let mut message = sanitized.text.trim().to_string();
    if message.is_empty() {
        return respond_chat(
            &req,
            memory_scope_ref,
            &req.message,
            ChatResponse {
                response: "❓ 메시지가 비어있어요. 다시 입력해주세요.".to_string(),
                command: None,
                route_meta: None,
            },
            "empty_message",
            "error",
            None,
            ChatOpsFlags::default(),
            Some("sanitization removed all content"),
        );
    }

    // [Memory] Save User Message
    if let Err(e) = db::insert_chat_message("user", &message) {
        eprintln!("Failed to save user chat: {}", e);
    }
    let mut message_lc = message.to_lowercase();

    if let Some(response) =
        handle_system_chat_command(&state, &req, memory_scope_ref, &message, &message_lc).await
    {
        return response;
    }

    let ai_digest_route = ai_digest::infer_program_route(&message, req.channel.as_deref());
    if let Some(route) = ai_digest_route
        .as_ref()
        .filter(|route| route.kind == ai_digest::AiDigestProgramRouteKind::Explicit)
    {
        let response = run_ai_digest_chat_response(&route.request_text).await;
        let outcome = chat_ops_outcome_from_response(&response.response);
        return respond_chat(
            &req,
            memory_scope_ref,
            &message,
            response,
            "ai_digest_explicit",
            outcome,
            Some(1.0),
            ChatOpsFlags {
                ai_digest_used: true,
                ..Default::default()
            },
            None,
        );
    }
    let auto_ai_digest_request = ai_digest_route.and_then(|route| {
        (route.kind == ai_digest::AiDigestProgramRouteKind::Auto).then_some(route.request_text)
    });
    message = ai_digest::strip_local_execution_prefix(&message);
    message_lc = message.to_lowercase();

    if let Some(response) = handle_local_chat_command(&req, memory_scope_ref, &message, &message_lc)
    {
        return response;
    }

    if let Some(response) =
        handle_vision_chat_command(&state, &req, memory_scope_ref, &message).await
    {
        return response;
    }

    if let Some(response) =
        handle_demo_vision_command(&state, &req, memory_scope_ref, &message).await
    {
        return response;
    }

    let ResolvedChatIntent {
        intent,
        confidence,
        route_kind: ops_route_kind,
        flags: mut ops_flags,
    } = match resolve_chat_intent(
        &state,
        &req,
        memory_scope_ref,
        &message,
        auto_ai_digest_request.as_deref(),
    )
    .await
    {
        Ok(resolved) => resolved,
        Err(response) => return response,
    };

    let command = intent["command"].as_str().unwrap_or("unknown").to_string();
    apply_freshness_flags(
        &message,
        &command,
        &intent,
        &mut ops_flags,
        memory_scope_ref,
    );

    if confidence < 0.5 {
        if let Some(request_text) = auto_ai_digest_request.as_deref() {
            return handle_auto_ai_digest_fallback(
                &req,
                memory_scope_ref,
                &message,
                request_text,
                confidence,
                ops_flags,
                "low confidence route used digest auto fallback",
            )
            .await;
        }
        return respond_chat(
            &req,
            memory_scope_ref,
            &message,
            ChatResponse {
                response: "❓ 무슨 말인지 잘 모르겠어요. 다시 말씀해주세요.".to_string(),
                command: None,
                route_meta: None,
            },
            "low_confidence",
            "error",
            Some(confidence),
            ops_flags,
            None,
        );
    }

    if let Some(response) = handle_cached_chat_response(
        &req,
        memory_scope_ref,
        &message,
        &command,
        &intent,
        confidence,
        ops_flags,
    ) {
        return response;
    }

    if command == "unknown" {
        if let Some(request_text) = auto_ai_digest_request.as_deref() {
            return handle_auto_ai_digest_fallback(
                &req,
                memory_scope_ref,
                &message,
                request_text,
                confidence,
                ops_flags,
                "unknown command route used digest auto fallback",
            )
            .await;
        }
    }

    execute_and_respond(
        &req,
        memory_scope_ref,
        &message,
        &command,
        &intent,
        confidence,
        ops_route_kind,
        ops_flags,
    )
    .await
}
