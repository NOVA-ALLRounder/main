use super::*;

#[test]
#[serial]
fn test_release_nl_run_metrics_ignore_local_chat_noise() {
    init().ok();
    clear_nl_runs_for_tests();

    insert_nl_run(
        "help_local",
        "도움말",
        "completed",
        Some("HELP"),
        Some(r#"{"source":"api.chat","route_kind":"local_command","command":"help_local"}"#),
    )
    .expect("insert local help");
    insert_nl_run(
        "build_workflow",
        "노션에 회의록 정리하는 워크플로우 만들어줘",
        "approval_required",
        Some("workflow queued"),
        Some(r#"{"source":"api.chat","route_kind":"deterministic","command":"build_workflow"}"#),
    )
    .expect("insert workflow request");

    let metrics = get_release_nl_run_metrics(20).expect("release metrics");
    assert_eq!(metrics.total, 1);
    assert_eq!(metrics.approval_required, 1);
    assert_eq!(metrics.completed, 0);
}

#[test]
#[serial]
fn test_release_nl_run_metrics_cap_duplicate_prompts_instead_of_single_dedupe() {
    init().ok();
    clear_nl_runs_for_tests();

    for idx in 0..5 {
        insert_nl_run(
            "flight_search",
            "서울에서 도쿄 항공권을 찾아줘",
            if idx % 2 == 0 {
                "completed"
            } else {
                "approval_required"
            },
            Some("flight search"),
            Some(r#"{"source":"api.chat","route_kind":"llm","command":"flight_search"}"#),
        )
        .expect("insert flight request");
    }

    let metrics = get_release_nl_run_metrics(20).expect("release metrics");
    assert_eq!(metrics.total, 3);
    assert_eq!(metrics.completed, 2);
    assert_eq!(metrics.approval_required, 1);
}

#[test]
#[serial]
fn test_exec_approval_metrics_capture_backlog_and_decisions() {
    init().ok();
    clear_exec_approvals_for_tests();

    create_exec_approval("dangerous pending", None, 3600).expect("seed pending approval");
    create_exec_approval("dangerous expired", None, -60).expect("seed expired approval");

    let allow_once =
        create_exec_approval("safe allow once", None, 3600).expect("seed allow-once approval");
    resolve_exec_approval(&allow_once.id, "approved", Some("test"), Some("allow-once"))
        .expect("resolve allow-once");

    let allow_always =
        create_exec_approval("safe allow always", None, 3600).expect("seed allow-always approval");
    resolve_exec_approval(
        &allow_always.id,
        "approved",
        Some("test"),
        Some("allow-always"),
    )
    .expect("resolve allow-always");

    let deny = create_exec_approval("blocked command", None, 3600).expect("seed denied approval");
    resolve_exec_approval(&deny.id, "rejected", Some("test"), Some("deny")).expect("resolve deny");

    let metrics = get_exec_approval_metrics(20).expect("exec approval metrics");
    assert_eq!(metrics.total, 5);
    assert_eq!(metrics.pending, 2);
    assert_eq!(metrics.approved, 2);
    assert_eq!(metrics.rejected, 1);
    assert_eq!(metrics.expired_pending, 1);
    assert_eq!(metrics.allow_once, 1);
    assert_eq!(metrics.allow_always, 1);
    assert_eq!(metrics.deny, 1);
    assert!((metrics.approval_rate - 66.6).abs() < 1.0);
    assert!(metrics.oldest_pending_created_at.is_some());
    assert!(metrics.last_created_at.is_some());
    assert!(metrics.last_resolved_at.is_some());
}

#[test]
#[serial]
fn test_launch_ops_metrics_aggregate_recent_routes() {
    init().ok();
    clear_launch_ops_events_for_tests();

    record_launch_ops_event(
        Some("web"),
        Some("channel_web__sender_alice"),
        "오늘 일정 보여줘",
        "request_memory",
        Some("calendar_today"),
        "success",
        Some(0.94),
        false,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
        None,
    )
    .expect("record request memory hit");
    record_launch_ops_event(
        Some("web"),
        Some("channel_web__sender_alice"),
        "최근 메일 5개 보여줘",
        "execution_memory",
        Some("gmail_list"),
        "success",
        Some(0.92),
        true,
        false,
        false,
        true,
        true,
        false,
        false,
        false,
        Some("freshness bypass after recent re-ask"),
    )
    .expect("record execution memory hit");
    record_launch_ops_event(
        Some("telegram"),
        Some("channel_telegram__sender_bob"),
        "뉴스 5개 요약해서 노션에 정리해줘",
        "ai_digest_auto",
        Some("ai_digest_program"),
        "success",
        Some(0.83),
        false,
        false,
        false,
        false,
        false,
        false,
        true,
        false,
        Some("fallback=auto_digest"),
    )
    .expect("record ai digest route");
    record_launch_ops_event(
        Some("web"),
        Some("channel_web__sender_alice"),
        "???",
        "low_confidence",
        None,
        "error",
        Some(0.31),
        false,
        false,
        false,
        false,
        false,
        true,
        false,
        false,
        Some("confidence below threshold"),
    )
    .expect("record low confidence route");

    let metrics = get_launch_ops_metrics(20).expect("launch ops metrics");
    assert_eq!(metrics.total_requests, 4);
    assert_eq!(metrics.intent_memory_hits, 1);
    assert_eq!(metrics.request_memory_hits, 1);
    assert_eq!(metrics.execution_memory_hits, 1);
    assert_eq!(metrics.ai_digest_auto_routes, 1);
    assert_eq!(metrics.low_confidence_routes, 1);
    assert_eq!(metrics.error_routes, 1);
    assert_eq!(metrics.freshness_bypasses, 1);
    assert!(metrics.cached_response_hit_rate >= 49.0);
    assert!(metrics
        .route_breakdown
        .iter()
        .any(|entry| entry.route_kind == "request_memory"));

    let events = list_launch_ops_events(3).expect("launch ops events");
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].route_kind, "low_confidence");
    assert_eq!(events[1].route_kind, "ai_digest_auto");
}

#[test]
#[serial]
fn test_sync_release_nl_runs_from_launch_ops_is_idempotent_and_filters_noise() {
    init().ok();
    clear_nl_runs_for_tests();
    clear_launch_ops_events_for_tests();

    record_launch_ops_event(
        Some("web"),
        Some("channel_web__sender_noise"),
        "도움말",
        "local_command",
        Some("help_local"),
        "success",
        Some(0.99),
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        true,
        Some("noise"),
    )
    .expect("record noisy local event");

    record_launch_ops_event(
        Some("web"),
        Some("channel_web__sender_alice"),
        "노션에 회의록 정리하는 워크플로우 만들어줘",
        "deterministic",
        Some("build_workflow"),
        "success",
        Some(0.91),
        false,
        false,
        false,
        false,
        true,
        false,
        false,
        false,
        Some("approval queued"),
    )
    .expect("record workflow event");

    record_launch_ops_event(
        Some("web"),
        Some("channel_web__sender_alice"),
        "오늘 일정 보여줘",
        "request_memory",
        Some("calendar_today"),
        "success",
        Some(0.95),
        false,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
        None,
    )
    .expect("record calendar event");

    let inserted_first = sync_release_nl_runs_from_launch_ops(20).expect("first sync");
    let inserted_second = sync_release_nl_runs_from_launch_ops(20).expect("second sync");

    assert_eq!(inserted_first, 2);
    assert_eq!(inserted_second, 0);

    let runs = list_nl_runs(10).expect("list nl runs");
    assert_eq!(runs.len(), 2);
    assert!(runs
        .iter()
        .any(|run| run.intent == "build_workflow" && run.status == "approval_required"));
    assert!(runs
        .iter()
        .any(|run| run.intent == "calendar_today" && run.status == "completed"));
}
