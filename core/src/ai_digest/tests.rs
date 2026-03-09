use super::*;
use serial_test::serial;
use std::fs;
use tempfile::tempdir;

#[test]
fn detects_ai_digest_request_keywords() {
    assert!(looks_like_news_digest_request(
        "AI뉴스 5개 요약해서 노션에 정리해줘"
    ));
    assert!(looks_like_news_digest_request(
        "Please create an AI news digest and send to notion"
    ));
    assert!(looks_like_news_digest_request(
        "스포츠 뉴스 5개 선정해서 노션에 정리해줘"
    ));
    assert!(!looks_like_news_digest_request("open finder"));
}

#[test]
fn explicit_n8n_request_detection() {
    assert_eq!(
        extract_explicit_n8n_request("/n8n 스포츠 뉴스 5개 요약"),
        Some("스포츠 뉴스 5개 요약".to_string())
    );
    assert_eq!(
        extract_explicit_n8n_request("workflow: 경제 뉴스 3개 노션 정리"),
        Some("경제 뉴스 3개 노션 정리".to_string())
    );
    assert_eq!(
        extract_explicit_n8n_request("/n8n"),
        Some(DEFAULT_REQUEST_TEXT.to_string())
    );
    assert_eq!(
        extract_explicit_n8n_request("스포츠 뉴스 5개 노션에 정리해줘"),
        None
    );
}

#[test]
#[serial]
fn auto_route_defaults_to_web_and_telegram_only() {
    std::env::remove_var("ALLVIA_AI_DIGEST_AUTO_ROUTE_CHANNELS");
    std::env::remove_var("STEER_TELEGRAM_AUTO_ROUTE_AI_DIGEST");
    std::env::set_var(
        "STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL",
        "http://127.0.0.1:5678/webhook/test/programtrigger/ai-digest-program",
    );

    let web = infer_program_route("AI 뉴스 5개 요약해서 노션에 정리해줘", Some("web"))
        .expect("web route");
    let telegram = infer_program_route("AI 뉴스 5개 요약해서 노션에 정리해줘", Some("telegram"))
        .expect("telegram route");
    let api = infer_program_route("AI 뉴스 5개 요약해서 노션에 정리해줘", Some("api"));

    assert_eq!(web.kind, AiDigestProgramRouteKind::Auto);
    assert_eq!(telegram.kind, AiDigestProgramRouteKind::Auto);
    assert!(api.is_none());

    std::env::remove_var("STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL");
}

#[test]
#[serial]
fn infer_program_route_respects_channel_override_and_local_prefix() {
    std::env::set_var("ALLVIA_AI_DIGEST_AUTO_ROUTE_CHANNELS", "telegram");
    std::env::set_var(
        "STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL",
        "http://127.0.0.1:5678/webhook/test/programtrigger/ai-digest-program",
    );

    assert!(infer_program_route(
        "/local AI 뉴스 5개 요약해서 노션에 정리해줘",
        Some("telegram"),
    )
    .is_none());

    let telegram = infer_program_route("AI 뉴스 5개 요약해서 노션에 정리해줘", Some("telegram"))
        .expect("telegram route");
    let web = infer_program_route("AI 뉴스 5개 요약해서 노션에 정리해줘", Some("web"));

    assert_eq!(telegram.kind, AiDigestProgramRouteKind::Auto);
    assert!(web.is_none());

    std::env::remove_var("ALLVIA_AI_DIGEST_AUTO_ROUTE_CHANNELS");
    std::env::remove_var("STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL");
}

#[test]
fn local_prefix_strip_works() {
    assert_eq!(
        strip_local_execution_prefix("/local 스포츠 뉴스 5개 요약해줘"),
        "스포츠 뉴스 5개 요약해줘"
    );
    assert_eq!(
        strip_local_execution_prefix("LLM: 메모장 열고 테스트 입력"),
        "메모장 열고 테스트 입력"
    );
    assert_eq!(
        strip_local_execution_prefix("그냥 일반 요청"),
        "그냥 일반 요청"
    );
}

#[test]
fn extracts_notion_url_from_nested_payload() {
    let payload = serde_json::json!({
        "status": "completed",
        "result": {
            "notion_page_url": "https://www.notion.so/abc123def456"
        }
    });
    assert_eq!(
        extract_notion_url(&payload),
        Some("https://www.notion.so/abc123def456".to_string())
    );
}

#[test]
fn format_human_summary_contains_headlines_and_notion_link() {
    let payload = serde_json::json!({
        "status": "completed",
        "notion_page_url": "https://www.notion.so/page123",
        "top_headlines_text": "1. 헤드라인 A\n2. 헤드라인 B\n3. 헤드라인 C"
    });
    let result = AiDigestTriggerResult {
        ok: true,
        status_code: 200,
        webhook_url: "http://localhost:5678/webhook/test".to_string(),
        scope_marker: "RUN_SCOPE_TEST".to_string(),
        notion_url: None,
        response_json: Some(payload),
        response_text: None,
    };
    let summary = format_human_summary(&result);
    assert!(summary.contains("노션 링크: https://www.notion.so/page123"));
    assert!(summary.contains("핵심 뉴스:"));
    assert!(summary.contains("1. 헤드라인 A"));
}

#[test]
#[serial]
fn resolves_webhook_url_from_runbook_status() {
    let tmp = tempdir().expect("tempdir");
    let root = tmp.path().join("runbook");
    fs::create_dir_all(&root).expect("mkdir runbook");

    let run_dir = root.join("run_1");
    fs::create_dir_all(&run_dir).expect("mkdir run");
    fs::write(
        root.join("latest_run_dir.txt"),
        run_dir.to_string_lossy().to_string(),
    )
    .expect("write latest");
    fs::write(
        run_dir.join("status.json"),
        r#"{"n8n_workflow_id":"wf_123"}"#,
    )
    .expect("write status");

    std::env::set_var(
        "STEER_MASTER_RUNBOOK_ROOT",
        root.to_string_lossy().to_string(),
    );
    std::env::remove_var("STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL");
    std::env::remove_var("STEER_AI_DIGEST_WORKFLOW_ID");

    let url = resolve_program_webhook_url().expect("resolve webhook");
    assert_eq!(
        url,
        "http://localhost:5678/webhook/wf_123/programtrigger/ai-digest-program"
    );

    std::env::remove_var("STEER_MASTER_RUNBOOK_ROOT");
}
