use super::{RunResultContext, TelegramBot, User};
use chrono::Utc;
use serde_json::json;
use serial_test::serial;

#[test]
fn webhook_conflict_detector_matches_telegram_conflict_messages() {
    assert!(TelegramBot::is_webhook_conflict_error(
        "Telegram API Error: 409 Conflict: terminated by other getUpdates request"
    ));
    assert!(TelegramBot::is_webhook_conflict_error(
        "Conflict: can't use getUpdates method while webhook is active"
    ));
    assert!(!TelegramBot::is_webhook_conflict_error(
        "Telegram API Error: 401 Unauthorized"
    ));
}

#[test]
fn extract_result_context_from_steps_includes_notion_page_url() {
    let steps = vec![crate::session_store::SessionStep {
        step_index: 0,
        action_type: "notion_write".to_string(),
        description: "Notion page created: https://www.notion.so/abcd1234".to_string(),
        status: "success".to_string(),
        timestamp: Utc::now(),
        data: Some(json!({
            "page_id": "abcd1234",
            "page_url": "https://www.notion.so/abcd1234",
            "content_preview": "AI 뉴스 기사 요약\n1. 제목 A\n링크: https://example.com/a\n요약:\n- 핵심: 요약 A"
        })),
    }];
    let ctx = TelegramBot::extract_result_context_from_steps(&steps);
    assert_eq!(
        ctx.links,
        vec!["https://www.notion.so/abcd1234".to_string()]
    );
    assert!(!ctx.highlights.is_empty());
}

#[test]
#[serial]
fn infer_n8n_digest_request_auto_routes_news_to_notion() {
    std::env::remove_var("STEER_TELEGRAM_AUTO_ROUTE_AI_DIGEST");
    std::env::set_var(
        "STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL",
        "http://127.0.0.1:5678/webhook/test/programtrigger/ai-digest-program",
    );
    let routed = TelegramBot::infer_n8n_digest_request(
        "최근 ai 트렌드 중요한거 5개 llm으로 요약해서 노션에 정리해줘",
    );
    assert!(routed.is_some());
    std::env::remove_var("STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL");
}

#[test]
#[serial]
fn infer_n8n_digest_request_respects_local_execution_prefix() {
    std::env::set_var(
        "STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL",
        "http://127.0.0.1:5678/webhook/test/programtrigger/ai-digest-program",
    );
    let routed =
        TelegramBot::infer_n8n_digest_request("/local 스포츠 뉴스 5개 요약해서 노션에 정리해줘");
    assert!(routed.is_none());
    std::env::remove_var("STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL");
}

#[test]
fn build_run_report_includes_result_links_section() {
    let outcome = crate::controller::planner::RunGoalOutcome {
        run_id: "surf_test_1".to_string(),
        planner_complete: true,
        execution_complete: true,
        business_complete: true,
        status: "business_completed".to_string(),
        summary: Some("ok".to_string()),
    };
    let ctx = RunResultContext {
        links: vec!["https://www.notion.so/abcd1234".to_string()],
        highlights: vec!["제목 A — 요약 A (https://example.com/a)".to_string()],
    };
    let report = TelegramBot::build_run_report(
        &outcome,
        &[],
        &[],
        "최근 ai 트렌드 중요한거 5개 llm으로 요약해서 노션에 정리해줘",
        &ctx,
    );
    assert!(report.contains("노션 링크:"));
    assert!(report.contains("https://www.notion.so/abcd1234"));
    assert!(report.contains("핵심 요약:"));
}

#[test]
fn telegram_sender_key_prefers_user_id_and_chat_id() {
    let user = User {
        id: 42,
        username: Some("alice".to_string()),
    };
    assert_eq!(
        TelegramBot::telegram_sender_key(Some(&user), 9001),
        "telegram_user_42_chat_9001"
    );
    assert_eq!(
        TelegramBot::telegram_sender_key(None, 9001),
        "telegram_chat_9001"
    );
}

#[test]
fn chat_response_requires_goal_fallback_only_for_unknown_reply() {
    let unknown = crate::api_server::ChatResponse {
        response: "🤔 요청을 정확히 해석하지 못했어요.\n더 구체적으로 말해줘.".to_string(),
        command: None,
        route_meta: None,
    };
    let deterministic = crate::api_server::ChatResponse {
        response: "📅 오늘 일정이 없습니다.".to_string(),
        command: Some("calendar_today".to_string()),
        route_meta: None,
    };

    assert!(TelegramBot::chat_response_requires_goal_fallback(&unknown));
    assert!(!TelegramBot::chat_response_requires_goal_fallback(
        &deterministic
    ));
}
