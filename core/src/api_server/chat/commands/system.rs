use axum::Json;
use std::sync::atomic::Ordering;

use crate::api_server::chat_support::{chat_ops_outcome_from_response, respond_chat};
use crate::api_server::{
    run_analysis_internal, telegram_listener_started_flag, try_spawn_telegram_listener, AppState,
    TelegramListenerStartOutcome,
};

use super::local_only_flags;
use crate::api_server::{ChatRequest, ChatResponse};

pub(crate) async fn handle_system_chat_command(
    state: &AppState,
    req: &ChatRequest,
    memory_scope_ref: Option<&str>,
    message: &str,
    message_lc: &str,
) -> Option<Json<ChatResponse>> {
    if message_lc == "analyze_patterns" || message == "패턴 분석" {
        let results = run_analysis_internal();
        let response_text = if results.is_empty() {
            "🔍 분석 완료! 새로운 패턴을 찾지 못했습니다.\n(하지만 시연을 위해 데모 항목을 생성했습니다. 오른쪽을 확인하세요!)".to_string()
        } else {
            format!(
                "🔍 분석 완료! {}개의 패턴을 찾았습니다:\n{}",
                results.len(),
                results.join("\n")
            )
        };

        return Some(respond_chat(
            req,
            memory_scope_ref,
            message,
            ChatResponse {
                response: response_text,
                command: Some("analyze_patterns".to_string()),
                route_meta: None,
            },
            "system_command",
            "success",
            Some(1.0),
            local_only_flags(),
            None,
        ));
    }

    if message_lc == "telegram listener status"
        || message_lc == "telegram_listen status"
        || message_lc == "telegram-listen status"
        || message_lc == "telegram status"
        || message.contains("텔레그램 리스너 상태")
    {
        let active = telegram_listener_started_flag().load(Ordering::SeqCst);
        return Some(respond_chat(
            req,
            memory_scope_ref,
            message,
            ChatResponse {
                response: if active {
                    "🤖 Telegram listener 상태: running".to_string()
                } else {
                    "⚪️ Telegram listener 상태: stopped".to_string()
                },
                command: Some("telegram_listener_status".to_string()),
                route_meta: None,
            },
            "system_command",
            "success",
            Some(1.0),
            local_only_flags(),
            None,
        ));
    }

    if message_lc == "telegram_listen"
        || message_lc == "telegram-listen"
        || message_lc == "telegram listener start"
        || message_lc == "telegram listen"
        || message.contains("텔레그램 리스너 시작")
    {
        let started_flag = telegram_listener_started_flag();
        if started_flag.load(Ordering::SeqCst) {
            return Some(respond_chat(
                req,
                memory_scope_ref,
                message,
                ChatResponse {
                    response: "ℹ️ Telegram listener가 이미 실행 중입니다.".to_string(),
                    command: Some("telegram_listener_start".to_string()),
                    route_meta: None,
                },
                "system_command",
                "success",
                Some(1.0),
                local_only_flags(),
                None,
            ));
        }

        let llm = match state.llm_client.clone() {
            Some(v) => v,
            None => {
                return Some(respond_chat(
                    req,
                    memory_scope_ref,
                    message,
                    ChatResponse {
                        response:
                            "❌ LLM 클라이언트가 없어 Telegram listener를 시작할 수 없습니다."
                                .to_string(),
                        command: Some("telegram_listener_start".to_string()),
                        route_meta: None,
                    },
                    "system_command",
                    "error",
                    Some(1.0),
                    local_only_flags(),
                    Some("telegram listener requires llm client"),
                ));
            }
        };
        let response = match try_spawn_telegram_listener(llm) {
            Ok(TelegramListenerStartOutcome::Started) => ChatResponse {
                response: "🤖 Telegram listener 시작됨 (long polling)".to_string(),
                command: Some("telegram_listener_start".to_string()),
                route_meta: None,
            },
            Ok(TelegramListenerStartOutcome::AlreadyRunning) => ChatResponse {
                response: "ℹ️ Telegram listener가 이미 실행 중입니다.".to_string(),
                command: Some("telegram_listener_start".to_string()),
                route_meta: None,
            },
            Err("missing_telegram_token") => ChatResponse {
                response: "❌ TELEGRAM_BOT_TOKEN이 없어 listener를 시작할 수 없습니다.".to_string(),
                command: Some("telegram_listener_start".to_string()),
                route_meta: None,
            },
            Err(_) => ChatResponse {
                response: "❌ Telegram listener 시작 중 알 수 없는 오류가 발생했습니다."
                    .to_string(),
                command: Some("telegram_listener_start".to_string()),
                route_meta: None,
            },
        };
        let outcome = chat_ops_outcome_from_response(&response.response);
        return Some(respond_chat(
            req,
            memory_scope_ref,
            message,
            response,
            "system_command",
            outcome,
            Some(1.0),
            local_only_flags(),
            None,
        ));
    }

    if message_lc == "n8n restart" || message.contains("n8n 재시작") {
        let api = match crate::n8n_api::N8nApi::from_env() {
            Ok(v) => v,
            Err(e) => {
                return Some(respond_chat(
                    req,
                    memory_scope_ref,
                    message,
                    ChatResponse {
                        response: format!("❌ n8n 초기화 실패: {}", e),
                        command: None,
                        route_meta: None,
                    },
                    "system_command",
                    "error",
                    Some(1.0),
                    local_only_flags(),
                    Some("n8n api init failed"),
                ));
            }
        };
        return Some(match api.restart_server().await {
            Ok(_) => respond_chat(
                req,
                memory_scope_ref,
                message,
                ChatResponse {
                    response: "🔄 n8n 서버를 재시작했습니다.".to_string(),
                    command: Some("n8n_restart".to_string()),
                    route_meta: None,
                },
                "system_command",
                "success",
                Some(1.0),
                local_only_flags(),
                None,
            ),
            Err(e) => respond_chat(
                req,
                memory_scope_ref,
                message,
                ChatResponse {
                    response: format!("❌ n8n 재시작 실패: {}", e),
                    command: None,
                    route_meta: None,
                },
                "system_command",
                "error",
                Some(1.0),
                local_only_flags(),
                Some("n8n restart failed"),
            ),
        });
    }

    None
}
