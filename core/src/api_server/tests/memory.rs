use super::*;

#[test]
#[serial]
fn request_memory_signature_cache_reuses_calendar_today_for_safe_paraphrase() {
    crate::db::init().ok();
    reset_memory_tables();
    let seed_prefix = format!("allvia-cache-calendar-today-{}", uuid::Uuid::new_v4());
    let intent = json!({
        "command": "calendar_today",
        "params": {},
        "confidence": 0.92
    });

    crate::db::upsert_request_memory(
        &format!("{} 오늘 일정 보여줘", seed_prefix),
        Some(&intent),
        Some("📅 오늘 일정이 없습니다."),
        "intent_only",
        "unit.test",
        0.92,
    )
    .expect("seed request memory");

    let cached = load_cached_request_intent(None, &format!("{} 오늘 캘린더 알려줘", seed_prefix))
        .expect("signature cache hit");
    assert_eq!(cached.0["command"].as_str(), Some("calendar_today"));
}

#[test]
#[serial]
fn request_memory_signature_cache_keeps_time_scope_distinct() {
    crate::db::init().ok();
    reset_memory_tables();
    let seed_prefix = format!("allvia-cache-calendar-week-{}", uuid::Uuid::new_v4());
    let intent = json!({
        "command": "calendar_week",
        "params": {},
        "confidence": 0.93
    });

    crate::db::upsert_request_memory(
        &format!("{} 이번 주 일정 보여줘", seed_prefix),
        Some(&intent),
        Some("📅 이번 주 일정이 없습니다."),
        "intent_only",
        "unit.test",
        0.93,
    )
    .expect("seed request memory");

    let cached = load_cached_request_intent(None, &format!("{} 오늘 일정 알려줘", seed_prefix));
    assert!(cached.is_none(), "today request must not reuse week intent");
}

#[test]
fn request_memory_signature_cache_skips_parametric_commands() {
    crate::db::init().ok();
    let seed_prefix = format!("allvia-cache-build-{}", uuid::Uuid::new_v4());
    let intent = json!({
        "command": "build_workflow",
        "params": {
            "prompt": "회의록을 노션에 저장"
        },
        "confidence": 0.96
    });

    crate::db::upsert_request_memory(
        &format!("{} 회의록 노션 워크플로우 만들어줘", seed_prefix),
        Some(&intent),
        Some("📝 워크플로우 제안을 생성했습니다."),
        "intent_only",
        "unit.test",
        0.96,
    )
    .expect("seed request memory");

    let cached = load_cached_request_intent(
        None,
        &format!("{} 리포트 슬랙 워크플로우 생성해줘", seed_prefix),
    );
    assert!(
        cached.is_none(),
        "parametric commands must stay exact-match only"
    );
}

#[test]
#[serial]
fn request_memory_response_cache_reuses_fresh_calendar_response_for_paraphrase() {
    crate::db::init().ok();
    reset_memory_tables();
    let seed_prefix = format!("allvia-response-calendar-{}", uuid::Uuid::new_v4());
    let intent = json!({
        "command": "calendar_today",
        "params": {},
        "confidence": 0.95
    });

    crate::db::upsert_request_memory(
        &format!("{} 오늘 일정 보여줘", seed_prefix),
        Some(&intent),
        Some("📅 오늘 일정이 없습니다."),
        request_memory_response_mode("calendar_today"),
        "unit.test",
        0.95,
    )
    .expect("seed response cache");

    let cached = load_cached_request_response(
        None,
        &format!("{} 오늘 캘린더 알려줘", seed_prefix),
        "calendar_today",
    );
    assert_eq!(cached.as_deref(), Some("📅 오늘 일정이 없습니다."));
}

#[test]
#[serial]
fn request_memory_response_cache_bypasses_recent_equivalent_chat_repeat() {
    crate::db::init().ok();
    reset_memory_tables();
    let seed_prefix = format!("allvia-response-repeat-{}", uuid::Uuid::new_v4());
    let intent = json!({
        "command": "calendar_today",
        "params": {},
        "confidence": 0.95,
        "source": "deterministic"
    });

    crate::db::upsert_request_memory(
        &format!("{} 오늘 일정 보여줘", seed_prefix),
        Some(&intent),
        Some("📅 오늘 일정이 없습니다."),
        request_memory_response_mode("calendar_today"),
        "api.chat.deterministic",
        0.95,
    )
    .expect("seed recent chat response");

    let cached = load_cached_request_response(
        None,
        &format!("{} 오늘 캘린더 알려줘", seed_prefix),
        "calendar_today",
    );
    assert!(
        cached.is_none(),
        "recent equivalent chat request should bypass response cache"
    );
}

#[test]
#[serial]
fn request_memory_response_cache_bypasses_when_user_requests_fresh_data() {
    crate::db::init().ok();
    reset_memory_tables();
    let seed_prefix = format!("allvia-response-refresh-{}", uuid::Uuid::new_v4());
    let intent = json!({
        "command": "calendar_today",
        "params": {},
        "confidence": 0.95
    });

    crate::db::upsert_request_memory(
        &format!("{} 오늘 일정 보여줘", seed_prefix),
        Some(&intent),
        Some("📅 오늘 일정이 없습니다."),
        request_memory_response_mode("calendar_today"),
        "unit.test",
        0.95,
    )
    .expect("seed response cache");

    let cached = load_cached_request_response(
        None,
        &format!("{} 지금 오늘 일정 새로고침해서 보여줘", seed_prefix),
        "calendar_today",
    );
    assert!(
        cached.is_none(),
        "freshness request must bypass response cache"
    );
}

#[test]
#[serial]
fn request_memory_response_cache_respects_zero_ttl() {
    crate::db::init().ok();
    reset_memory_tables();
    let seed_prefix = format!("allvia-response-ttl-zero-{}", uuid::Uuid::new_v4());
    let intent = json!({
        "command": "gmail_list",
        "params": {},
        "confidence": 0.95
    });

    crate::db::upsert_request_memory(
        &format!("{} 최근 이메일 5개 보여줘", seed_prefix),
        Some(&intent),
        Some("📭 새 메일이 없습니다."),
        request_memory_response_mode("gmail_list"),
        "unit.test",
        0.95,
    )
    .expect("seed response cache");

    std::env::set_var("ALLVIA_RESPONSE_CACHE_TTL_GMAIL_LIST", "0");
    let cached = load_cached_request_response(
        None,
        &format!("{} 최근 이메일 5개 알려줘", seed_prefix),
        "gmail_list",
    );
    std::env::remove_var("ALLVIA_RESPONSE_CACHE_TTL_GMAIL_LIST");

    assert!(cached.is_none(), "zero ttl must disable response reuse");
}

#[test]
#[serial]
fn request_memory_response_cache_reuses_help_local_exact_match() {
    crate::db::init().ok();
    reset_memory_tables();
    let request = format!("allvia-help-cache-{}", uuid::Uuid::new_v4());
    let response = "💡 바로 실행 가능한 명령".to_string();

    persist_chat_response_memory(
        None,
        &request,
        "help_local",
        &response,
        "api.chat.local",
        1.0,
    );

    let cached = load_cached_request_response(None, &request, "help_local");
    assert_eq!(cached.as_deref(), Some("💡 바로 실행 가능한 명령"));
}

#[test]
#[serial]
fn request_memory_negative_feedback_suppresses_response_reuse() {
    crate::db::init().ok();
    reset_memory_tables();
    let request = format!("allvia-feedback-suppress-{}", uuid::Uuid::new_v4());
    let intent = json!({
        "command": "calendar_today",
        "params": {},
        "confidence": 0.95
    });
    let response = "📅 오늘 일정이 없습니다.";

    crate::db::upsert_request_memory(
        &request,
        Some(&intent),
        Some(response),
        request_memory_response_mode("calendar_today"),
        "unit.test",
        0.95,
    )
    .expect("seed request memory");
    crate::db::record_request_memory_feedback(&request, response, "negative")
        .expect("negative feedback");

    let cached = load_cached_request_response(None, &request, "calendar_today");
    assert!(
        cached.is_none(),
        "negative feedback must suppress response reuse"
    );
}

#[test]
#[serial]
fn request_memory_suppressed_record_does_not_reuse_response() {
    crate::db::init().ok();
    reset_memory_tables();
    let request = format!("allvia-suppress-request-{}", uuid::Uuid::new_v4());
    let response = format!("📅 suppressed-request-{}", uuid::Uuid::new_v4());
    let intent = json!({
        "command": "calendar_today",
        "params": {},
        "confidence": 0.95
    });
    crate::db::upsert_request_memory(
        &request,
        Some(&intent),
        Some(&response),
        request_memory_response_mode("calendar_today"),
        "unit.test",
        0.95,
    )
    .expect("seed request memory");
    crate::db::suppress_request_memory(&request, Some("ops suppress"))
        .expect("suppress request memory");

    let cached = load_cached_request_response(None, &request, "calendar_today");
    assert!(
        cached.is_none(),
        "suppressed request memory must not be reused"
    );
}

#[test]
#[serial]
fn request_memory_negative_feedback_suppresses_intent_reuse() {
    crate::db::init().ok();
    reset_memory_tables();
    let request = format!("allvia-intent-suppress-{}", uuid::Uuid::new_v4());
    let intent = json!({
        "command": "calendar_today",
        "params": {},
        "confidence": 0.95
    });
    let response = "📅 오늘 일정이 없습니다.";

    crate::db::upsert_request_memory(
        &request,
        Some(&intent),
        Some(response),
        "intent_only",
        "unit.test",
        0.95,
    )
    .expect("seed request memory");
    crate::db::record_request_memory_feedback(&request, response, "negative")
        .expect("negative feedback");

    let cached = load_cached_request_intent(None, &request);
    assert!(
        cached.is_none(),
        "negative feedback must suppress intent reuse"
    );
}

#[test]
#[serial]
fn execution_memory_reuses_gmail_default_count_across_paraphrase() {
    crate::db::init().ok();
    reset_memory_tables();
    let seed_prefix = format!("allvia-exec-gmail-{}", uuid::Uuid::new_v4());
    let explicit_intent = json!({
        "command": "gmail_list",
        "params": { "count": 5 },
        "confidence": 0.95
    });
    let implicit_intent = json!({
        "command": "gmail_list",
        "params": {},
        "confidence": 0.95
    });
    let response = format!("📧 execution-cache-{}", uuid::Uuid::new_v4());

    persist_execution_memory(
        None,
        &format!("{} 최근 이메일 5개 보여줘", seed_prefix),
        "gmail_list",
        &explicit_intent,
        &response,
        "unit.test",
    );

    let cached = load_cached_execution_response(
        None,
        &format!("{} 최근 이메일 보여줘", seed_prefix),
        "gmail_list",
        &implicit_intent,
    );
    assert_eq!(cached.as_deref(), Some(response.as_str()));
}

#[test]
#[serial]
fn execution_memory_bypasses_when_user_requests_fresh_data() {
    crate::db::init().ok();
    reset_memory_tables();
    let seed_prefix = format!("allvia-exec-refresh-{}", uuid::Uuid::new_v4());
    let intent = json!({
        "command": "gmail_list",
        "params": { "count": 5 },
        "confidence": 0.95
    });
    let response = format!("📧 execution-refresh-{}", uuid::Uuid::new_v4());

    persist_execution_memory(
        None,
        &format!("{} 최근 이메일 5개 보여줘", seed_prefix),
        "gmail_list",
        &intent,
        &response,
        "unit.test",
    );

    let cached = load_cached_execution_response(
        None,
        &format!("{} 지금 최근 이메일 5개 새로고침", seed_prefix),
        "gmail_list",
        &intent,
    );
    assert!(
        cached.is_none(),
        "freshness request must bypass execution cache"
    );
}

#[test]
#[serial]
fn execution_memory_bypasses_recent_chat_repeat() {
    crate::db::init().ok();
    reset_memory_tables();
    let request = format!(
        "allvia-exec-repeat-{} 최근 이메일 5개 보여줘",
        uuid::Uuid::new_v4()
    );
    let intent = json!({
        "command": "gmail_list",
        "params": { "count": 5 },
        "confidence": 0.95
    });
    let response = format!("📧 execution-repeat-{}", uuid::Uuid::new_v4());

    persist_execution_memory(
        None,
        &request,
        "gmail_list",
        &intent,
        &response,
        "api.chat.execution",
    );

    let cached = load_cached_execution_response(None, &request, "gmail_list", &intent);
    assert!(
        cached.is_none(),
        "recent chat repeat should bypass execution cache"
    );
}

#[test]
#[serial]
fn execution_memory_keeps_gmail_count_distinct() {
    crate::db::init().ok();
    reset_memory_tables();
    let seed_prefix = format!("allvia-exec-gmail-count-{}", uuid::Uuid::new_v4());
    let five_intent = json!({
        "command": "gmail_list",
        "params": { "count": 5 },
        "confidence": 0.95
    });
    let ten_intent = json!({
        "command": "gmail_list",
        "params": { "count": 10 },
        "confidence": 0.95
    });
    let response = format!("📧 execution-cache-five-{}", uuid::Uuid::new_v4());

    persist_execution_memory(
        None,
        &format!("{} 최근 이메일 5개 보여줘", seed_prefix),
        "gmail_list",
        &five_intent,
        &response,
        "unit.test",
    );

    let cached = load_cached_execution_response(
        None,
        &format!("{} 최근 이메일 10개 보여줘", seed_prefix),
        "gmail_list",
        &ten_intent,
    );
    assert!(
        cached.is_none(),
        "different counts must not share execution cache"
    );
}

#[test]
#[serial]
fn execution_memory_negative_feedback_suppresses_reuse() {
    crate::db::init().ok();
    reset_memory_tables();
    let request = format!(
        "allvia-exec-feedback-{} 최근 이메일 17개 보여줘",
        uuid::Uuid::new_v4()
    );
    let intent = json!({
        "command": "gmail_list",
        "params": { "count": 17 },
        "confidence": 0.95
    });
    let response = format!("📧 execution-negative-{}", uuid::Uuid::new_v4());

    persist_execution_memory(
        None,
        &request,
        "gmail_list",
        &intent,
        &response,
        "unit.test",
    );
    crate::db::record_execution_memory_feedback("gmail_list", "count=17", &response, "negative")
        .expect("execution negative feedback");
    let found = crate::db::get_execution_memory("gmail_list", "count=17")
        .expect("get execution memory")
        .expect("execution memory row");
    assert_eq!(found.negative_feedback_count, 1);

    let cached = load_cached_execution_response(None, &request, "gmail_list", &intent);
    assert!(
        cached.is_none(),
        "negative feedback must suppress execution reuse"
    );
}

#[test]
#[serial]
fn execution_memory_suppressed_record_does_not_reuse_response() {
    crate::db::init().ok();
    reset_memory_tables();
    let request = format!(
        "allvia-suppress-execution-{} 최근 이메일 5개 보여줘",
        uuid::Uuid::new_v4()
    );
    let intent = json!({
        "command": "gmail_list",
        "params": { "count": 5 },
        "confidence": 0.95
    });
    let response = format!("📧 suppressed-execution-{}", uuid::Uuid::new_v4());

    persist_execution_memory(
        None,
        &request,
        "gmail_list",
        &intent,
        &response,
        "unit.test",
    );
    crate::db::suppress_execution_memory("gmail_list", "count=5", Some("ops suppress"))
        .expect("suppress execution memory");

    let cached = load_cached_execution_response(None, &request, "gmail_list", &intent);
    assert!(
        cached.is_none(),
        "suppressed execution memory must not be reused"
    );
}
