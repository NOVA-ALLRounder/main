use super::*;
use serde_json::json;
use serial_test::serial;

#[test]
#[serial]
fn generate_launch_eval_candidates_uses_real_records_without_leaking_response_text() {
    crate::db::init().ok();
    crate::db::clear_request_memory_for_tests();
    crate::db::clear_execution_memory_for_tests();
    crate::db::clear_launch_ops_events_for_tests();

    let request_intent = json!({
        "command": "calendar_today",
        "params": {},
        "confidence": 0.94
    });
    crate::db::upsert_request_memory_scoped(
        Some("channel_web__type_direct__sender_ops"),
        "오늘 일정 보여줘",
        Some(&request_intent),
        Some("PRIVATE_REQUEST_RESPONSE"),
        "ttl_response_signature",
        "api.chat.deterministic",
        0.94,
    )
    .expect("seed request memory");

    crate::db::upsert_execution_memory_scoped(
        Some("channel_web__type_direct__sender_ops"),
        "gmail_list",
        "count=5",
        Some(&json!({ "count": 5 })),
        Some("recent email 5 read"),
        "PRIVATE_EXECUTION_RESPONSE",
        20,
        "api.chat.execution",
        "integrations.gmail.list_messages",
    )
    .expect("seed execution memory");

    crate::db::record_launch_ops_event(
        Some("web"),
        Some("channel_web__type_direct__sender_ops"),
        "AI 뉴스 5개 요약해서 노션에 정리해줘",
        "ai_digest_auto_fallback",
        Some("ai_digest_program"),
        "success",
        Some(0.88),
        false,
        false,
        false,
        false,
        false,
        true,
        true,
        false,
        Some("launch eval candidate"),
    )
    .expect("seed launch ops event");

    let candidates = generate_launch_eval_candidates(10);
    assert!(candidates
        .iter()
        .any(|candidate| candidate.scenario_kind == "request_memory_reuse"));
    assert!(candidates
        .iter()
        .any(|candidate| candidate.scenario_kind == "execution_memory_reuse"));
    assert!(candidates
        .iter()
        .any(|candidate| candidate.command.as_deref() == Some("ai_digest_program")));
    assert!(candidates
        .iter()
        .all(|candidate| !candidate.yaml.contains("PRIVATE_REQUEST_RESPONSE")));
    assert!(candidates
        .iter()
        .all(|candidate| !candidate.yaml.contains("PRIVATE_EXECUTION_RESPONSE")));
    assert!(candidates
        .iter()
        .any(|candidate| candidate.yaml.contains("LAUNCH_EVAL_REQUEST_CACHE_")));
    assert!(candidates
        .iter()
        .any(|candidate| candidate.yaml.contains("LAUNCH_EVAL_EXECUTION_CACHE_")));
}

#[test]
#[serial]
fn generate_launch_eval_candidates_includes_task_run_business_contract_candidates() {
    crate::db::init().ok();
    crate::db::clear_request_memory_for_tests();
    crate::db::clear_execution_memory_for_tests();
    crate::db::clear_launch_ops_events_for_tests();

    let run_id = format!("launch-prod-{}", uuid::Uuid::new_v4());
    crate::db::create_task_run(
        &run_id,
        "surf_goal",
        "qed4950@gmail.com으로 요약 메일을 보내고 Notion에 저장한 뒤 텔레그램으로 알려줘",
        "running",
    )
    .expect("create task run");
    let details = serde_json::to_string(&vec![
            "Summary: qed4950@gmail.com으로 메일 전송, 노션 저장, 텔레그램 전송 완료".to_string(),
            "EVIDENCE target=mail|event=send|status=sent_confirmed|recipient=qed4950@gmail.com|body_len=42".to_string(),
            "EVIDENCE target=notion|event=write|status=confirmed|page_id=abc123".to_string(),
            "EVIDENCE target=telegram|event=send|status=sent|message_id=msg_123".to_string(),
            "notion: https://www.notion.so/private-page".to_string(),
        ])
        .expect("serialize details");
    crate::db::update_task_run_outcome(
        &run_id,
        true,
        true,
        true,
        "business_completed",
        Some("qed4950@gmail.com에게 보냈고 노션/텔레그램 완료"),
        Some(&details),
    )
    .expect("update task run");
    crate::db::upsert_task_run_artifact(
        &run_id,
        "artifact_assertion",
        "artifact.mail_sent_confirmed",
        "true",
        Some("{\"passed\":true}"),
    )
    .expect("artifact assertion");
    crate::db::upsert_task_run_artifact(
        &run_id,
        "artifact_assertion",
        "artifact.notion_write_confirmed",
        "true",
        Some("{\"passed\":true}"),
    )
    .expect("artifact assertion");

    let temp_dir = std::env::temp_dir().join(format!(
        "allvia-launch-eval-task-run-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");
    std::fs::create_dir_all(temp_dir.join("configs")).expect("create config dir");
    std::fs::write(
        temp_dir.join("configs").join("launch_eval.yaml"),
        "report_dir: reports/launch_eval\nscenarios: []\n",
    )
    .expect("write minimal config");

    let candidates = generate_launch_eval_candidates_with_filter(
        &temp_dir,
        20,
        LaunchEvalCandidateProvenanceFilter::Real,
    );
    let candidate = candidates
        .iter()
        .find(|candidate| {
            candidate.source_kind == "task_run"
                && candidate
                    .rationale
                    .iter()
                    .any(|item| item == &format!("run_id={}", run_id))
        })
        .expect("task run candidate");
    assert_eq!(candidate.provenance, "real");
    assert!(!candidate.yaml.contains("qed4950@gmail.com"));
    assert!(!candidate.yaml.contains("private-page"));
    assert!(candidate.yaml.contains("launch-eval@example.com"));
    assert!(candidate
        .yaml
        .contains("https://www.notion.so/launch-eval-page"));
}

#[test]
fn load_launch_eval_config_merges_include_paths() {
    let temp_dir = std::env::temp_dir().join(format!(
        "allvia-launch-eval-config-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");
    let base_path = temp_dir.join("base.yaml");
    let include_path = temp_dir.join("included.yaml");

    std::fs::write(
        &include_path,
        r#"
report_dir: reports/launch_eval
scenarios:
  - id: included-chat
    kind: chat
    request:
      message: "도움말"
    expect:
      command: "help_local"
"#,
    )
    .expect("write include");
    std::fs::write(
        &base_path,
        r#"
report_dir: reports/launch_eval
include_paths:
  - included.yaml
scenarios:
  - id: base-chat
    kind: chat
    request:
      message: "도움말"
    expect:
      command: "help_local"
"#,
    )
    .expect("write base");

    let config = load_launch_eval_config(&base_path).expect("load config");
    let ids = config.scenarios.iter().map(scenario_id).collect::<Vec<_>>();
    assert!(ids.contains(&"base-chat".to_string()));
    assert!(ids.contains(&"included-chat".to_string()));
}

#[test]
fn synthetic_candidates_are_loaded_from_curated_config_without_generated_include() {
    let temp_dir = std::env::temp_dir().join(format!(
        "allvia-launch-eval-synthetic-{}",
        uuid::Uuid::new_v4()
    ));
    let configs_dir = temp_dir.join("configs");
    std::fs::create_dir_all(&configs_dir).expect("create configs dir");
    std::fs::write(
        configs_dir.join("launch_eval.generated.yaml"),
        r#"
report_dir: reports/launch_eval
scenarios:
  - id: generated-real
    kind: chat
    request:
      message: "generated real scenario"
    expect:
      command: "help_local"
"#,
    )
    .expect("write generated include");
    std::fs::write(
        configs_dir.join("launch_eval.yaml"),
        r#"
report_dir: reports/launch_eval
include_paths:
  - launch_eval.generated.yaml
scenarios:
  - id: curated-dogfood
    kind: chat
    request:
      message: "dogfood curated scenario"
    expect:
      command: "help_local"
"#,
    )
    .expect("write curated config");

    let candidates = generate_launch_eval_candidates_with_filter(
        &temp_dir,
        8,
        LaunchEvalCandidateProvenanceFilter::Synthetic,
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].provenance, "synthetic");
    assert_eq!(candidates[0].id, "curated-dogfood");
}

#[test]
#[serial]
fn write_launch_eval_candidate_snapshot_writes_generated_yaml() {
    crate::db::init().ok();
    crate::db::clear_request_memory_for_tests();
    crate::db::clear_execution_memory_for_tests();
    crate::db::clear_launch_ops_events_for_tests();

    let request_intent = json!({
        "command": "calendar_today",
        "params": {},
        "confidence": 0.94
    });
    crate::db::upsert_request_memory_scoped(
        Some("channel_web__type_direct__sender_ops"),
        "오늘 일정 보여줘",
        Some(&request_intent),
        Some("PRIVATE_REQUEST_RESPONSE"),
        "ttl_response_signature",
        "api.chat.deterministic",
        0.94,
    )
    .expect("seed request memory");

    let temp_dir = std::env::temp_dir().join(format!(
        "allvia-launch-eval-snapshot-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");
    let snapshot = write_launch_eval_candidate_snapshot(&temp_dir, Some("generated.yaml"), 5)
        .expect("write snapshot");

    let raw = std::fs::read_to_string(&snapshot.output_path).expect("read snapshot");
    assert!(raw.contains("scenarios:"));
    assert!(raw.contains("request_memory_reuse"));
    assert!(!raw.contains("PRIVATE_REQUEST_RESPONSE"));
    assert!(snapshot.scenario_count >= 1);
}

#[test]
fn synthetic_snapshot_uses_dogfood_default_path() {
    let temp_dir = std::env::temp_dir().join(format!(
        "allvia-launch-eval-dogfood-snapshot-{}",
        uuid::Uuid::new_v4()
    ));
    let configs_dir = temp_dir.join("configs");
    std::fs::create_dir_all(&configs_dir).expect("create configs dir");
    std::fs::write(
        configs_dir.join("launch_eval.yaml"),
        r#"
report_dir: reports/launch_eval
scenarios:
  - id: dogfood-chat
    kind: chat
    request:
      message: "도그푸드 일정 보여줘"
    expect:
      command: "calendar_today"
"#,
    )
    .expect("write curated config");

    let snapshot = write_launch_eval_candidate_snapshot_with_filter(
        &temp_dir,
        None,
        5,
        LaunchEvalCandidateProvenanceFilter::Synthetic,
    )
    .expect("write synthetic snapshot");

    assert!(snapshot
        .output_path
        .ends_with("configs/launch_eval.dogfood.generated.yaml"));
    assert_eq!(snapshot.provenance_filter, "synthetic");
    assert_eq!(snapshot.scenario_count, 1);
}

#[test]
fn read_launch_eval_candidate_snapshot_info_reports_existing_file() {
    let temp_dir = std::env::temp_dir().join(format!(
        "allvia-launch-eval-snapshot-info-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");
    let snapshot_path = temp_dir.join("launch_eval.generated.yaml");
    std::fs::write(
        &snapshot_path,
        r#"report_dir: reports/launch_eval
scenarios:
  - kind: chat
    id: snapshot-info
    request:
      message: "도움말"
    expect:
      command: "help_local"
"#,
    )
    .expect("write snapshot info fixture");

    let info =
        read_launch_eval_candidate_snapshot_info(&temp_dir, Some("launch_eval.generated.yaml"));
    assert!(info.exists);
    assert_eq!(info.scenario_count, 1);
    assert_eq!(info.scenario_ids, vec!["snapshot-info".to_string()]);
    assert!(info.updated_at.is_some());
}

#[test]
#[serial]
fn request_memory_policy_case_detects_negative_feedback_block() {
    crate::db::init().ok();
    crate::db::clear_request_memory_for_tests();

    let scenario = LaunchEvalScenario::RequestMemoryPolicy {
        id: "request-feedback-block".to_string(),
        description: Some("negative feedback should block request cache reuse".to_string()),
        seed: RequestMemorySeed {
            request_text: "오늘 일정 보여줘".to_string(),
            command: "calendar_today".to_string(),
            params: json!({}),
            response_text: "📅 TODAY_CACHE".to_string(),
            confidence: 0.95,
            source: "launch.eval".to_string(),
            response_mode: None,
        },
        request: LaunchEvalChatInput {
            message: "오늘 일정 보여줘".to_string(),
            channel: Some("web".to_string()),
            chat_type: Some("direct".to_string()),
            sender: Some("launch-eval-request-policy".to_string()),
            mentioned: None,
        },
        mode: RequestMemoryPolicyMode::Response,
        feedback: Some("negative".to_string()),
        suppress: false,
        expect_cached: false,
        expect_command: None,
        expect_response_contains: Vec::new(),
    };

    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let result = runtime.block_on(run_scenario(&scenario));
    assert!(result.passed, "{:?}", result.errors);
}

#[test]
#[serial]
fn execution_memory_policy_case_detects_fresh_bypass() {
    crate::db::init().ok();
    crate::db::clear_execution_memory_for_tests();

    let scenario = LaunchEvalScenario::ExecutionMemoryPolicy {
        id: "execution-fresh-bypass".to_string(),
        description: Some("fresh wording should bypass execution cache".to_string()),
        seed: ExecutionMemorySeed {
            original_request: "최근 이메일 5개 보여줘".to_string(),
            command: "gmail_list".to_string(),
            params: json!({ "count": 5 }),
            response_text: "📧 EXEC_CACHE".to_string(),
            confidence: 0.95,
            source: "launch.eval".to_string(),
            ttl_seconds: Some(20),
            tool_path: None,
        },
        request: LaunchEvalChatInput {
            message: "지금 최근 이메일 5개 새로고침".to_string(),
            channel: Some("web".to_string()),
            chat_type: Some("direct".to_string()),
            sender: Some("launch-eval-exec-policy".to_string()),
            mentioned: None,
        },
        lookup_params: None,
        feedback: None,
        suppress: false,
        expect_cached: false,
        expect_response_contains: Vec::new(),
    };

    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let result = runtime.block_on(run_scenario(&scenario));
    assert!(result.passed, "{:?}", result.errors);
}

#[test]
fn business_contract_case_detects_missing_write_artifacts() {
    let scenario = LaunchEvalScenario::BusinessContract {
        id: "business-contract-missing-artifacts".to_string(),
        description: Some("missing mail/notion/telegram artifacts should fail".to_string()),
        plan: LaunchEvalBusinessPlan {
            intent: crate::nl_automation::IntentType::GenericTask,
            descriptions: vec![
                "Mail에서 qed4950@gmail.com으로 결과를 보내세요.".to_string(),
                "Notion에 요약을 작성하세요.".to_string(),
                "텔레그램으로 전송하세요.".to_string(),
            ],
            slots: std::collections::HashMap::new(),
        },
        logs: vec!["Summary: requested integrations done".to_string()],
        expect_ok: false,
        expect_detail_contains: vec![
            "contract_missing_mail_send_confirmation".to_string(),
            "contract_missing_notion_write_confirmation".to_string(),
            "contract_missing_telegram_send_confirmation".to_string(),
        ],
        expect_assertions: vec![
            LaunchEvalAssertionExpectation {
                key: "artifact.mail_sent_confirmed".to_string(),
                passed: false,
            },
            LaunchEvalAssertionExpectation {
                key: "artifact.notion_write_confirmed".to_string(),
                passed: false,
            },
        ],
    };

    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let result = runtime.block_on(run_scenario(&scenario));
    assert!(result.passed, "{:?}", result.errors);
}
