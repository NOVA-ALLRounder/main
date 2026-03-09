use super::*;

#[tokio::test]
#[serial]
async fn handle_chat_auto_routes_ai_digest_on_web_without_llm() {
    crate::db::init().ok();
    reset_memory_tables();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let addr = listener.local_addr().expect("listener addr");
    let app = axum::Router::new().route(
        "/",
        axum::routing::post(|| async {
            axum::Json(json!({
                "status": "ok",
                "notion_url": "https://www.notion.so/test-ai-digest",
                "top_headlines_text": "1. 헤드라인 A\n2. 헤드라인 B"
            }))
        }),
    );
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    std::env::remove_var("ALLVIA_AI_DIGEST_AUTO_ROUTE_CHANNELS");
    std::env::set_var(
        "STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL",
        format!("http://{addr}/"),
    );

    let state = AppState {
        llm_client: None,
        current_goal: Arc::new(Mutex::new(None)),
    };
    let request = ChatRequest {
        message: "AI 뉴스 5개 요약해서 노션에 정리해줘".to_string(),
        channel: Some("web".to_string()),
        chat_type: Some("direct".to_string()),
        sender: Some("web-test".to_string()),
        mentioned: None,
    };

    let Json(response) = handle_chat(State(state), Json(request)).await;

    assert_eq!(response.command.as_deref(), Some("ai_digest_program"));
    assert_eq!(
        response
            .route_meta
            .as_ref()
            .map(|meta| meta.route_kind.as_str()),
        Some("ai_digest_auto")
    );
    assert_eq!(
        response.route_meta.as_ref().map(|meta| meta.ai_digest_used),
        Some(true)
    );
    assert!(response
        .response
        .contains("노션 링크: https://www.notion.so/test-ai-digest"));
    assert!(response.response.contains("헤드라인 A"));

    std::env::remove_var("STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL");
}

#[tokio::test]
#[serial]
async fn handle_chat_recomputes_local_prefix_commands_after_strip() {
    crate::db::init().ok();
    reset_memory_tables();

    let state = AppState {
        llm_client: None,
        current_goal: Arc::new(Mutex::new(None)),
    };
    let request = ChatRequest {
        message: "/local help".to_string(),
        channel: Some("web".to_string()),
        chat_type: Some("direct".to_string()),
        sender: Some("prefix-user".to_string()),
        mentioned: None,
    };

    let Json(response) = handle_chat(State(state), Json(request)).await;

    assert_eq!(response.command.as_deref(), Some("help_local"));
    assert!(response
        .response
        .contains("뉴스 5개 요약해서 노션에 정리해줘"));
}

#[tokio::test]
#[serial]
async fn handle_chat_uses_deterministic_workflow_intent_without_llm() {
    crate::db::init().ok();
    crate::db::clear_recommendations_for_tests();

    let state = AppState {
        llm_client: None,
        current_goal: Arc::new(Mutex::new(None)),
    };
    let request = ChatRequest {
        message: "노션에 회의록 정리하는 워크플로우 만들어줘".to_string(),
        channel: None,
        chat_type: None,
        sender: None,
        mentioned: None,
    };

    let Json(response) = handle_chat(State(state), Json(request)).await;

    assert_eq!(response.command.as_deref(), Some("build_workflow"));
    assert_eq!(
        response
            .route_meta
            .as_ref()
            .map(|meta| meta.route_kind.as_str()),
        Some("deterministic")
    );
    assert_eq!(
        response
            .route_meta
            .as_ref()
            .map(|meta| meta.deterministic_used),
        Some(true)
    );
    assert!(response.response.contains("워크플로우 제안을"));
    let stored = crate::db::get_request_memory("노션에 회의록 정리하는 워크플로우 만들어줘")
        .expect("request memory lookup")
        .expect("request memory row");
    assert_eq!(stored.source, "api.chat.deterministic");
}

#[tokio::test]
#[serial]
async fn handle_chat_scopes_local_memory_by_sender() {
    crate::db::init().ok();
    reset_memory_tables();

    let state = AppState {
        llm_client: None,
        current_goal: Arc::new(Mutex::new(None)),
    };

    let request_alice = ChatRequest {
        message: "도움말".to_string(),
        channel: Some("web".to_string()),
        chat_type: Some("direct".to_string()),
        sender: Some("alice".to_string()),
        mentioned: None,
    };
    let request_bob = ChatRequest {
        message: "도움말".to_string(),
        channel: Some("web".to_string()),
        chat_type: Some("direct".to_string()),
        sender: Some("bob".to_string()),
        mentioned: None,
    };

    let Json(response_alice) = handle_chat(State(state.clone()), Json(request_alice)).await;
    let Json(response_bob) = handle_chat(State(state), Json(request_bob)).await;

    assert_eq!(response_alice.command.as_deref(), Some("help_local"));
    assert_eq!(response_bob.command.as_deref(), Some("help_local"));

    let alice = crate::db::get_request_memory_scoped(
        Some("channel_web__type_direct__sender_alice"),
        "도움말",
    )
    .expect("alice scoped lookup")
    .expect("alice scoped row");
    let bob = crate::db::get_request_memory_scoped(
        Some("channel_web__type_direct__sender_bob"),
        "도움말",
    )
    .expect("bob scoped lookup")
    .expect("bob scoped row");

    assert_eq!(alice.memory_scope, "channel_web_type_direct_sender_alice");
    assert_eq!(bob.memory_scope, "channel_web_type_direct_sender_bob");
    assert!(crate::db::get_request_memory("도움말")
        .expect("global lookup")
        .is_none());
}

#[tokio::test]
#[serial]
async fn handle_chat_records_launch_ops_for_local_cache_hit() {
    crate::db::init().ok();
    reset_memory_tables();

    let state = AppState {
        llm_client: None,
        current_goal: Arc::new(Mutex::new(None)),
    };
    let request = ChatRequest {
        message: "도움말".to_string(),
        channel: Some("web".to_string()),
        chat_type: Some("direct".to_string()),
        sender: Some("launch-ops-user".to_string()),
        mentioned: None,
    };

    let Json(first) = handle_chat(State(state.clone()), Json(request.clone())).await;
    let Json(second) = handle_chat(State(state), Json(request)).await;

    assert_eq!(first.command.as_deref(), Some("help_local"));
    assert_eq!(second.command.as_deref(), Some("help_local"));

    let events = crate::db::list_launch_ops_events(4).expect("launch ops events");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].route_kind, "request_memory");
    assert!(events[0].request_memory_hit);
    assert!(events[0].local_only);
    assert_eq!(events[1].route_kind, "local_command");
    assert_eq!(
        second
            .route_meta
            .as_ref()
            .map(|meta| meta.route_kind.as_str()),
        Some("request_memory")
    );
    assert_eq!(
        second
            .route_meta
            .as_ref()
            .map(|meta| meta.request_memory_hit),
        Some(true)
    );
}

#[tokio::test]
#[serial]
async fn memory_records_handler_returns_recent_admin_events() {
    crate::db::init().ok();
    reset_memory_tables();

    crate::db::upsert_request_memory(
        "오늘 일정 보여줘",
        Some(&serde_json::json!({
            "command": "calendar_today",
            "params": {},
            "confidence": 0.95
        })),
        Some("📅 일정 2건"),
        "ttl_response_signature",
        "unit.test",
        0.95,
    )
    .expect("seed request memory");

    let (status, Json(response)) =
        suppress_request_memory_handler(Json(RequestMemoryAdminActionRequest {
            request_text: "오늘 일정 보여줘".to_string(),
            memory_scope: None,
            reason: Some("launch cache issue".to_string()),
            actor: Some("settings_test".to_string()),
        }))
        .await;

    assert_eq!(status, StatusCode::OK);
    assert!(response.ok);

    let Json(records) = list_memory_records_handler(Query(MemoryRecordsQuery {
        limit: Some(10),
        include_suppressed: Some(true),
    }))
    .await;

    assert!(!records.recent_admin_events.is_empty());
    let event = &records.recent_admin_events[0];
    assert_eq!(event.kind, "request_memory");
    assert_eq!(event.action, "suppress");
    assert_eq!(event.actor.as_deref(), Some("settings_test"));
    assert_eq!(event.reason.as_deref(), Some("launch cache issue"));
    assert!(event.ok);
}

#[tokio::test]
#[serial]
async fn handle_chat_records_release_nl_run_for_chat_request_but_skips_local_help_noise() {
    crate::db::init().ok();
    crate::db::clear_recommendations_for_tests();
    reset_memory_tables();

    let state = AppState {
        llm_client: None,
        current_goal: Arc::new(Mutex::new(None)),
    };

    let help_request = ChatRequest {
        message: "도움말".to_string(),
        channel: Some("web".to_string()),
        chat_type: Some("direct".to_string()),
        sender: Some("nl-run-help".to_string()),
        mentioned: None,
    };
    let workflow_request = ChatRequest {
        message: "노션에 회의록 정리하는 워크플로우 만들어줘".to_string(),
        channel: Some("web".to_string()),
        chat_type: Some("direct".to_string()),
        sender: Some("nl-run-workflow".to_string()),
        mentioned: None,
    };

    let _ = handle_chat(State(state.clone()), Json(help_request)).await;
    let Json(response) = handle_chat(State(state), Json(workflow_request)).await;

    assert_eq!(response.command.as_deref(), Some("build_workflow"));

    let runs = crate::db::list_nl_runs(10).expect("list nl runs");
    assert_eq!(
        runs.len(),
        1,
        "local help should not create a release nl_run"
    );
    assert_eq!(runs[0].intent, "build_workflow");
    assert_eq!(runs[0].status, "approval_required");
    assert!(runs[0].prompt.contains("워크플로우 만들어줘"));
}
