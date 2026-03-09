use axum::Json;

use crate::api_server::chat_support::{respond_chat, ChatOpsFlags};
use crate::api_server::{AppState, ChatRequest, ChatResponse};

pub(crate) async fn handle_vision_chat_command(
    state: &AppState,
    req: &ChatRequest,
    memory_scope_ref: Option<&str>,
    message: &str,
) -> Option<Json<ChatResponse>> {
    if !(message.trim() == "/capture"
        || message.contains("화면 분석해줘")
        || message.contains("analyze screen"))
    {
        return None;
    }

    if let Some(llm) = &state.llm_client {
        return Some(match crate::visual_driver::VisualDriver::capture_screen() {
            Ok((b64, _scale)) => {
                let prompt = "Describe what is on the user's screen briefly. Identify active applications and context.";
                match llm.analyze_screen(prompt, &b64).await {
                    Ok(desc) => respond_chat(
                        req,
                        memory_scope_ref,
                        message,
                        ChatResponse {
                            response: format!("👁️ 화면 분석 결과:\n{}", desc),
                            command: None,
                            route_meta: None,
                        },
                        "vision_command",
                        "success",
                        Some(1.0),
                        ChatOpsFlags::default(),
                        None,
                    ),
                    Err(e) => respond_chat(
                        req,
                        memory_scope_ref,
                        message,
                        ChatResponse {
                            response: format!("❌ Vision API 오류: {}", e),
                            command: None,
                            route_meta: None,
                        },
                        "vision_command",
                        "error",
                        Some(1.0),
                        ChatOpsFlags::default(),
                        Some("vision model analysis failed"),
                    ),
                }
            }
            Err(e) => respond_chat(
                req,
                memory_scope_ref,
                message,
                ChatResponse {
                    response: format!("❌ 화면 캡처 실패: {}", e),
                    command: None,
                    route_meta: None,
                },
                "vision_command",
                "error",
                Some(1.0),
                ChatOpsFlags::default(),
                Some("screen capture failed"),
            ),
        });
    }

    Some(respond_chat(
        req,
        memory_scope_ref,
        message,
        ChatResponse {
            response: "❌ LLM 클라이언트가 초기화되지 않았습니다.".to_string(),
            command: None,
            route_meta: None,
        },
        "vision_command",
        "error",
        None,
        ChatOpsFlags::default(),
        Some("vision command requires llm client"),
    ))
}
