use axum::Json;
use serde_json::Value;

use crate::api_server::chat_support::{
    chat_ops_outcome_from_response, load_cached_request_intent, load_deterministic_chat_intent,
    respond_chat, run_ai_digest_chat_response, ChatOpsFlags,
};
use crate::api_server::AppState;
use crate::{context_pruning, db};

use super::{ChatRequest, ChatResponse};

pub(crate) struct ResolvedChatIntent {
    pub intent: Value,
    pub confidence: f64,
    pub route_kind: &'static str,
    pub flags: ChatOpsFlags,
}

pub(crate) async fn resolve_chat_intent(
    state: &AppState,
    req: &ChatRequest,
    memory_scope_ref: Option<&str>,
    message: &str,
    auto_ai_digest_request: Option<&str>,
) -> Result<ResolvedChatIntent, Json<ChatResponse>> {
    if let Some((cached_intent, cached_confidence)) =
        load_cached_request_intent(memory_scope_ref, message)
    {
        return Ok(ResolvedChatIntent {
            intent: cached_intent,
            confidence: cached_confidence,
            route_kind: "intent_memory",
            flags: ChatOpsFlags {
                intent_memory_hit: true,
                ..Default::default()
            },
        });
    }

    if let Some((deterministic_intent, deterministic_confidence)) =
        load_deterministic_chat_intent(message)
    {
        return Ok(ResolvedChatIntent {
            intent: deterministic_intent,
            confidence: deterministic_confidence,
            route_kind: "deterministic",
            flags: ChatOpsFlags {
                deterministic_used: true,
                ..Default::default()
            },
        });
    }

    let Some(brain) = &state.llm_client else {
        if let Some(request_text) = auto_ai_digest_request {
            let response = run_ai_digest_chat_response(request_text).await;
            let outcome = chat_ops_outcome_from_response(&response.response);
            return Err(respond_chat(
                req,
                memory_scope_ref,
                message,
                response,
                "ai_digest_auto",
                outcome,
                None,
                ChatOpsFlags {
                    ai_digest_used: true,
                    ..Default::default()
                },
                Some("llm unavailable, used digest auto fallback"),
            ));
        }
        return Err(respond_chat(
            req,
            memory_scope_ref,
            message,
            ChatResponse {
                response: "⚠️ LLM 클라이언트가 없습니다.".to_string(),
                command: None,
                route_meta: None,
            },
            "llm_unavailable",
            "error",
            None,
            ChatOpsFlags::default(),
            None,
        ));
    };

    let history =
        db::get_recent_chat_history(context_pruning::history_fetch_limit()).unwrap_or_default();
    match brain.parse_intent_with_history(message, &history).await {
        Ok(intent) => Ok(ResolvedChatIntent {
            confidence: intent["confidence"].as_f64().unwrap_or(0.0),
            intent,
            route_kind: "llm",
            flags: ChatOpsFlags {
                llm_used: true,
                ..Default::default()
            },
        }),
        Err(e) => {
            if let Some(request_text) = auto_ai_digest_request {
                let response = run_ai_digest_chat_response(request_text).await;
                let outcome = chat_ops_outcome_from_response(&response.response);
                return Err(respond_chat(
                    req,
                    memory_scope_ref,
                    message,
                    response,
                    "ai_digest_auto",
                    outcome,
                    None,
                    ChatOpsFlags {
                        llm_used: true,
                        ai_digest_used: true,
                        ..Default::default()
                    },
                    Some("llm parse failed, used digest auto fallback"),
                ));
            }
            let err_text = e.to_string();
            let response = if err_text.to_lowercase().contains("insufficient_quota") {
                "⚠️ OpenAI 사용량 한도를 초과했습니다. 잠시 후 다시 시도하거나 API 키/요금제를 확인해주세요.\n\n지금도 가능한 로컬 명령:\n• 패턴 분석\n• 시스템 상태\n• n8n 재시작".to_string()
            } else {
                format!("❌ 오류: {}", err_text)
            };
            Err(respond_chat(
                req,
                memory_scope_ref,
                message,
                ChatResponse {
                    response,
                    command: None,
                    route_meta: None,
                },
                "llm_error",
                "error",
                None,
                ChatOpsFlags {
                    llm_used: true,
                    ..Default::default()
                },
                Some("llm parse_intent_with_history failed"),
            ))
        }
    }
}
