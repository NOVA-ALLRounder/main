use super::*;

#[test]
#[serial]
fn test_request_memory_roundtrip_normalizes_whitespace() {
    init().ok();
    clear_request_memory_for_tests();
    let intent = serde_json::json!({
        "command": "calendar_today",
        "params": {},
        "confidence": 0.91
    });
    upsert_request_memory(
        "  오늘   일정   보여줘  ",
        Some(&intent),
        Some("📅 오늘 일정이 없습니다."),
        "intent_only",
        "unit.test",
        0.91,
    )
    .expect("upsert request memory");

    let found = get_request_memory("오늘 일정 보여줘")
        .expect("get request memory")
        .expect("request memory row");
    assert_eq!(found.response_mode, "intent_only");
    assert!(found.confidence >= 0.9);
    assert_eq!(found.intent_command.as_deref(), Some("calendar_today"));
    assert_eq!(
        found.request_signature.as_deref(),
        Some("today calendar read")
    );
    assert!(found
        .intent_json
        .unwrap_or_default()
        .contains("calendar_today"));
}

#[test]
#[serial]
fn test_request_memory_roundtrip_ignores_punctuation() {
    init().ok();
    clear_request_memory_for_tests();
    let intent = serde_json::json!({
        "command": "calendar_today",
        "params": {},
        "confidence": 0.93
    });
    upsert_request_memory(
        "오늘 일정 보여줘?!",
        Some(&intent),
        Some("📅 오늘 일정이 없습니다."),
        "intent_only",
        "unit.test",
        0.93,
    )
    .expect("upsert request memory");

    let found = get_request_memory("오늘 일정 보여줘")
        .expect("get request memory")
        .expect("request memory row");
    assert!(found.confidence >= 0.93);
    assert!(found
        .intent_json
        .unwrap_or_default()
        .contains("calendar_today"));
}

#[test]
#[serial]
fn test_request_memory_is_scoped_by_actor_context() {
    init().ok();
    clear_request_memory_for_tests();
    let intent = serde_json::json!({
        "command": "calendar_today",
        "params": {},
        "confidence": 0.94
    });

    upsert_request_memory_scoped(
        Some("channel_web__sender_alice"),
        "오늘 일정 보여줘",
        Some(&intent),
        Some("📅 Alice"),
        "intent_only",
        "unit.test",
        0.94,
    )
    .expect("seed alice request memory");
    upsert_request_memory_scoped(
        Some("channel_web__sender_bob"),
        "오늘 일정 보여줘",
        Some(&intent),
        Some("📅 Bob"),
        "intent_only",
        "unit.test",
        0.94,
    )
    .expect("seed bob request memory");

    let alice = get_request_memory_scoped(Some("channel_web__sender_alice"), "오늘 일정 보여줘")
        .expect("alice lookup")
        .expect("alice row");
    let bob = get_request_memory_scoped(Some("channel_web__sender_bob"), "오늘 일정 보여줘")
        .expect("bob lookup")
        .expect("bob row");

    assert_eq!(alice.memory_scope, "channel_web_sender_alice");
    assert_eq!(bob.memory_scope, "channel_web_sender_bob");
    assert_eq!(alice.response_text.as_deref(), Some("📅 Alice"));
    assert_eq!(bob.response_text.as_deref(), Some("📅 Bob"));
    assert!(get_request_memory("오늘 일정 보여줘")
        .expect("global lookup")
        .is_none());
}

#[test]
#[serial]
fn test_request_memory_lookup_by_signature_prefers_supported_command() {
    init().ok();
    clear_request_memory_for_tests();
    let intent = serde_json::json!({
        "command": "calendar_today",
        "params": {},
        "confidence": 0.94
    });
    upsert_request_memory(
        "오늘 캘린더 보여줘",
        Some(&intent),
        Some("📅 오늘 일정이 없습니다."),
        "intent_only",
        "unit.test",
        0.94,
    )
    .expect("upsert request memory");

    let found = get_request_memory_by_signature("today calendar read", &["calendar_today"])
        .expect("lookup by signature")
        .expect("signature row");
    assert_eq!(found.intent_command.as_deref(), Some("calendar_today"));
    assert_eq!(
        found.request_signature.as_deref(),
        Some("today calendar read")
    );
}

#[test]
#[serial]
fn test_request_memory_lookup_by_signature_prefers_positive_feedback() {
    init().ok();
    clear_request_memory_for_tests();
    let positive_request = "allvia positive cache";
    let negative_request = "allvia negative cache";
    let command = format!("calendar_today_pref_{}", uuid::Uuid::new_v4());
    let intent = serde_json::json!({
        "command": command,
        "params": {},
        "confidence": 0.94
    });

    upsert_request_memory(
        &format!("{} 오늘 일정 보여줘", negative_request),
        Some(&intent),
        Some("📅 A"),
        "intent_only",
        "unit.test",
        0.94,
    )
    .expect("seed negative request");
    upsert_request_memory(
        &format!("{} 오늘 캘린더 알려줘", positive_request),
        Some(&intent),
        Some("📅 B"),
        "intent_only",
        "unit.test",
        0.94,
    )
    .expect("seed positive request");

    record_request_memory_feedback(
        &format!("{} 오늘 일정 보여줘", negative_request),
        "📅 A",
        "negative",
    )
    .expect("negative feedback");
    record_request_memory_feedback(
        &format!("{} 오늘 캘린더 알려줘", positive_request),
        "📅 B",
        "positive",
    )
    .expect("positive feedback");

    let found = get_request_memory_by_signature("today calendar read", &[command.as_str()])
        .expect("lookup by signature")
        .expect("signature row");
    assert!(found.original_request.contains(&positive_request));
    assert_eq!(found.positive_feedback_count, 1);
    assert_eq!(found.negative_feedback_count, 0);
}

#[test]
#[serial]
fn test_request_memory_lookup_by_signature_prefers_deterministic_source() {
    init().ok();
    clear_request_memory_for_tests();
    let command = format!("calendar_today_source_{}", uuid::Uuid::new_v4());
    let intent = serde_json::json!({
        "command": command,
        "params": {},
        "confidence": 0.94,
        "source": "deterministic"
    });

    upsert_request_memory(
        "allvia source llm 오늘 일정 보여줘",
        Some(&intent),
        Some("📅 LLM"),
        "intent_only",
        "api.chat.llm",
        0.94,
    )
    .expect("seed llm request");
    upsert_request_memory(
        "allvia source deterministic 오늘 캘린더 알려줘",
        Some(&intent),
        Some("📅 deterministic"),
        "intent_only",
        "api.chat.deterministic",
        0.94,
    )
    .expect("seed deterministic request");

    let found = get_request_memory_by_signature("today calendar read", &[command.as_str()])
        .expect("lookup by signature")
        .expect("signature row");
    assert_eq!(found.source, "api.chat.deterministic");
    assert!(found.original_request.contains("deterministic"));
}

#[test]
#[serial]
fn test_execution_memory_roundtrip_by_command_and_params() {
    init().ok();
    clear_execution_memory_for_tests();
    let params = serde_json::json!({ "count": 5 });
    let response = format!("📧 test-execution-memory-{}", uuid::Uuid::new_v4());

    upsert_execution_memory(
        "gmail_list",
        "count=5",
        Some(&params),
        Some("recent email 5 read"),
        &response,
        20,
        "unit.test",
        "integrations.gmail.list_messages",
    )
    .expect("upsert execution memory");

    let found = get_execution_memory("gmail_list", "count=5")
        .expect("get execution memory")
        .expect("execution memory row");
    assert_eq!(found.intent_command, "gmail_list");
    assert_eq!(found.params_key, "count=5");
    assert_eq!(found.response_text, response);
    assert_eq!(found.freshness_ttl_seconds, 20);
    assert_eq!(found.tool_path, "integrations.gmail.list_messages");
    assert!(found.success);
}

#[test]
#[serial]
fn test_execution_memory_is_scoped_by_actor_context() {
    init().ok();
    clear_execution_memory_for_tests();
    let params = serde_json::json!({ "count": 5 });

    upsert_execution_memory_scoped(
        Some("channel_web__sender_alice"),
        "gmail_list",
        "count=5",
        Some(&params),
        Some("recent email 5 read"),
        "📧 Alice",
        20,
        "unit.test",
        "integrations.gmail.list_messages",
    )
    .expect("seed alice execution memory");
    upsert_execution_memory_scoped(
        Some("channel_web__sender_bob"),
        "gmail_list",
        "count=5",
        Some(&params),
        Some("recent email 5 read"),
        "📧 Bob",
        20,
        "unit.test",
        "integrations.gmail.list_messages",
    )
    .expect("seed bob execution memory");

    let alice =
        get_execution_memory_scoped(Some("channel_web__sender_alice"), "gmail_list", "count=5")
            .expect("alice execution lookup")
            .expect("alice execution row");
    let bob = get_execution_memory_scoped(Some("channel_web__sender_bob"), "gmail_list", "count=5")
        .expect("bob execution lookup")
        .expect("bob execution row");

    assert_eq!(alice.memory_scope, "channel_web_sender_alice");
    assert_eq!(bob.memory_scope, "channel_web_sender_bob");
    assert_eq!(alice.params_key, "count=5");
    assert_eq!(bob.params_key, "count=5");
    assert_eq!(alice.response_text, "📧 Alice");
    assert_eq!(bob.response_text, "📧 Bob");
    assert!(get_execution_memory("gmail_list", "count=5")
        .expect("global execution lookup")
        .is_none());
}

#[test]
#[serial]
fn test_request_memory_feedback_updates_matching_response_only() {
    init().ok();
    clear_request_memory_for_tests();
    let request = format!("allvia-feedback-request-{}", uuid::Uuid::new_v4());
    let response = format!("response-{}", uuid::Uuid::new_v4());
    let intent = serde_json::json!({
        "command": "calendar_today",
        "params": {},
        "confidence": 0.91
    });

    upsert_request_memory(
        &request,
        Some(&intent),
        Some(&response),
        "intent_only",
        "unit.test",
        0.91,
    )
    .expect("seed request memory");

    assert!(
        record_request_memory_feedback(&request, &response, "negative").expect("record feedback")
    );
    let found = get_request_memory(&request)
        .expect("get request memory")
        .expect("request memory row");
    assert_eq!(found.negative_feedback_count, 1);
    assert_eq!(found.positive_feedback_count, 0);
    assert!(found.last_feedback_at.is_some());

    assert!(
        !record_request_memory_feedback(&request, "other-response", "positive")
            .expect("mismatched feedback")
    );
}

#[test]
#[serial]
fn test_execution_memory_feedback_updates_matching_response_only() {
    init().ok();
    clear_execution_memory_for_tests();
    let response = format!("response-{}", uuid::Uuid::new_v4());
    let command = format!("gmail_feedback_{}", uuid::Uuid::new_v4());
    let params_key = format!("count=5-{}", uuid::Uuid::new_v4());
    let params = serde_json::json!({ "count": 5 });
    upsert_execution_memory(
        &command,
        &params_key,
        Some(&params),
        Some("recent email 5 read"),
        &response,
        20,
        "unit.test",
        "integrations.gmail.list_messages",
    )
    .expect("seed execution memory");

    assert!(
        record_execution_memory_feedback(&command, &params_key, &response, "positive")
            .expect("record execution feedback")
    );
    let found = get_execution_memory(&command, &params_key)
        .expect("get execution memory")
        .expect("execution memory row");
    assert_eq!(found.positive_feedback_count, 1);
    assert_eq!(found.negative_feedback_count, 0);
    assert!(found.last_feedback_at.is_some());

    assert!(
        !record_execution_memory_feedback(&command, &params_key, "other", "negative")
            .expect("mismatched execution feedback")
    );
}

#[test]
#[serial]
fn test_request_memory_suppress_restore_and_delete_roundtrip() {
    init().ok();
    clear_request_memory_for_tests();
    let request = format!("allvia-memory-admin-request-{}", uuid::Uuid::new_v4());
    let intent = serde_json::json!({
        "command": "calendar_today",
        "params": {},
        "confidence": 0.95
    });
    upsert_request_memory(
        &request,
        Some(&intent),
        Some("📅 memory-admin"),
        "ttl_response_signature",
        "unit.test",
        0.95,
    )
    .expect("seed request memory");

    assert!(get_request_memory(&request)
        .expect("get request memory")
        .is_some());
    assert!(suppress_request_memory(&request, Some("bad cache")).expect("suppress request memory"));
    assert!(get_request_memory(&request)
        .expect("suppressed request lookup")
        .is_none());

    let listed = list_request_memory_records(10, true).expect("list request memory");
    let suppressed = listed
        .into_iter()
        .find(|record| record.original_request == request)
        .expect("suppressed request record");
    assert!(suppressed.suppressed);
    assert_eq!(suppressed.suppressed_reason.as_deref(), Some("bad cache"));

    assert!(restore_request_memory(&request).expect("restore request memory"));
    assert!(get_request_memory(&request)
        .expect("restored request memory")
        .is_some());

    assert!(delete_request_memory(&request).expect("delete request memory"));
    assert!(get_request_memory(&request)
        .expect("deleted request lookup")
        .is_none());
}

#[test]
#[serial]
fn test_execution_memory_suppress_restore_and_delete_roundtrip() {
    init().ok();
    clear_execution_memory_for_tests();
    let command = format!("gmail_admin_{}", uuid::Uuid::new_v4());
    upsert_execution_memory(
        &command,
        "count=5",
        Some(&serde_json::json!({ "count": 5 })),
        Some("recent email 5 read"),
        "📧 memory-admin",
        20,
        "unit.test",
        "integrations.gmail.list_messages",
    )
    .expect("seed execution memory");

    assert!(get_execution_memory(&command, "count=5")
        .expect("get execution memory")
        .is_some());
    assert!(
        suppress_execution_memory(&command, "count=5", Some("stale result"))
            .expect("suppress execution memory")
    );
    assert!(get_execution_memory(&command, "count=5")
        .expect("suppressed execution lookup")
        .is_none());

    let listed = list_execution_memory_records(10, true).expect("list execution memory");
    let suppressed = listed
        .into_iter()
        .find(|record| record.intent_command == command)
        .expect("suppressed execution record");
    assert!(suppressed.suppressed);
    assert_eq!(
        suppressed.suppressed_reason.as_deref(),
        Some("stale result")
    );

    assert!(restore_execution_memory(&command, "count=5").expect("restore execution memory"));
    assert!(get_execution_memory(&command, "count=5")
        .expect("restored execution lookup")
        .is_some());

    assert!(delete_execution_memory(&command, "count=5").expect("delete execution memory"));
    assert!(get_execution_memory(&command, "count=5")
        .expect("deleted execution lookup")
        .is_none());
}

#[test]
#[serial]
fn test_memory_ops_metrics_count_suppressed_records() {
    init().ok();
    clear_request_memory_for_tests();
    clear_execution_memory_for_tests();

    let request = format!("allvia-memory-metrics-request-{}", uuid::Uuid::new_v4());
    upsert_request_memory(
        &request,
        Some(&serde_json::json!({
            "command": "help_local",
            "params": {},
            "confidence": 0.95
        })),
        Some("HELP"),
        "reusable_response",
        "unit.test",
        0.95,
    )
    .expect("seed request memory");
    upsert_execution_memory(
        "gmail_metrics",
        "count=5",
        Some(&serde_json::json!({ "count": 5 })),
        Some("recent email 5 read"),
        "📧 metrics",
        20,
        "unit.test",
        "integrations.gmail.list_messages",
    )
    .expect("seed execution memory");

    suppress_request_memory(&request, Some("ops")).expect("suppress request");
    suppress_execution_memory("gmail_metrics", "count=5", Some("ops")).expect("suppress execution");

    let metrics = get_memory_ops_metrics().expect("memory ops metrics");
    assert!(metrics.request_suppressed >= 1);
    assert!(metrics.execution_suppressed >= 1);
}

#[test]
#[serial]
fn test_memory_admin_events_roundtrip() {
    init().ok();
    clear_memory_admin_events_for_tests();

    record_memory_admin_event(
        "request_memory",
        "suppress",
        Some("channel_web__sender_alice"),
        "오늘 일정 보여줘",
        Some("bad cache"),
        Some("web_settings"),
        true,
        Some("request_memory suppress succeeded."),
    )
    .expect("record request admin event");
    record_memory_admin_event(
        "execution_memory",
        "delete",
        Some("channel_web__sender_alice"),
        "gmail_list::count=5",
        None,
        Some("api.memory_admin"),
        false,
        Some("execution_memory delete target was not found."),
    )
    .expect("record execution admin event");

    let events = list_memory_admin_events(10).expect("list memory admin events");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].kind, "execution_memory");
    assert_eq!(events[0].action, "delete");
    assert_eq!(
        events[0].memory_scope.as_deref(),
        Some("channel_web_sender_alice")
    );
    assert!(!events[0].ok);
    assert_eq!(events[1].kind, "request_memory");
    assert_eq!(events[1].reason.as_deref(), Some("bad cache"));
    assert_eq!(events[1].actor.as_deref(), Some("web_settings"));
    assert!(events[1].ok);
}
