use axum::Json;

use crate::api_server::chat_support::{
    build_local_chat_response, load_local_cached_chat_response, respond_chat, ChatOpsFlags,
};

use super::local_only_flags;
use crate::api_server::{ChatRequest, ChatResponse};
use crate::monitor;

pub(crate) fn handle_local_chat_command(
    req: &ChatRequest,
    memory_scope_ref: Option<&str>,
    message: &str,
    message_lc: &str,
) -> Option<Json<ChatResponse>> {
    if message_lc == "help" || message == "도움말" || message == "명령어" {
        if let Some(cached) =
            load_local_cached_chat_response(memory_scope_ref, message, "help_local")
        {
            return Some(respond_chat(
                req,
                memory_scope_ref,
                message,
                cached.0,
                "request_memory",
                "success",
                Some(1.0),
                local_request_memory_hit_flags(),
                Some("local help response cache hit"),
            ));
        }
        return Some(respond_chat(
            req,
            memory_scope_ref,
            message,
            build_local_chat_response(
                memory_scope_ref,
                message,
                "help_local",
                "💡 바로 실행 가능한 명령:\n• analyze_patterns / 패턴 분석\n• n8n restart / n8n 재시작\n• telegram listener start / 텔레그램 리스너 시작\n• telegram listener status / 텔레그램 리스너 상태\n• 뉴스 5개 요약해서 노션에 정리해줘\n• /n8n 스포츠 뉴스 5개 요약해서 노션에 정리해줘 (명시 digest)\n• /local 메모장 열고 체크리스트 작성해줘 (로컬 실행 강제)\n• /capture (화면 분석)\n• system_status / 시스템 상태".to_string(),
                "api.chat.local",
            )
            .0,
            "local_command",
            "success",
            Some(1.0),
            local_only_flags(),
            None,
        ));
    }
    if message == "안녕" || message_lc == "hello" || message_lc == "hi" {
        return Some(handle_greeting(
            req,
            memory_scope_ref,
            message,
            "👋 안녕하세요! 간단 작업은 바로 실행할 수 있어요.\n원하면 `패턴 분석` 또는 `시스템 상태`라고 입력해보세요.",
        ));
    }
    if message == "야"
        || message == "야!"
        || message == "야?"
        || message_lc == "hey"
        || message_lc == "yo"
    {
        return Some(handle_greeting(
            req,
            memory_scope_ref,
            message,
            "응, 듣고 있어요. 바로 할 일을 말해줘.\n예: `오늘 일정 보여줘`, `패턴 분석`, `n8n 열어줘`",
        ));
    }
    if message_lc == "system_status" || message == "시스템 상태" || message == "코어 상태"
    {
        if let Some(cached) =
            load_local_cached_chat_response(memory_scope_ref, message, "system_status")
        {
            return Some(respond_chat(
                req,
                memory_scope_ref,
                message,
                cached.0,
                "request_memory",
                "success",
                Some(1.0),
                local_request_memory_hit_flags(),
                Some("local system status cache hit"),
            ));
        }
        let mut rm = monitor::ResourceMonitor::new();
        return Some(respond_chat(
            req,
            memory_scope_ref,
            message,
            build_local_chat_response(
                memory_scope_ref,
                message,
                "system_status",
                format!("📊 {}", rm.get_status()),
                "api.chat.local",
            )
            .0,
            "local_command",
            "success",
            Some(1.0),
            local_only_flags(),
            None,
        ));
    }

    None
}

fn handle_greeting(
    req: &ChatRequest,
    memory_scope_ref: Option<&str>,
    message: &str,
    greeting: &str,
) -> Json<ChatResponse> {
    if let Some(cached) =
        load_local_cached_chat_response(memory_scope_ref, message, "greeting_local")
    {
        return respond_chat(
            req,
            memory_scope_ref,
            message,
            cached.0,
            "request_memory",
            "success",
            Some(1.0),
            local_request_memory_hit_flags(),
            Some("local greeting cache hit"),
        );
    }
    respond_chat(
        req,
        memory_scope_ref,
        message,
        build_local_chat_response(
            memory_scope_ref,
            message,
            "greeting_local",
            greeting.to_string(),
            "api.chat.local",
        )
        .0,
        "local_command",
        "success",
        Some(1.0),
        local_only_flags(),
        None,
    )
}

fn local_request_memory_hit_flags() -> ChatOpsFlags {
    ChatOpsFlags {
        request_memory_hit: true,
        ..local_only_flags()
    }
}
