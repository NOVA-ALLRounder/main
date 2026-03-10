use super::Planner;
use crate::platform::AppRole;
use crate::session_store::Session;
use chrono::Utc;
use unicode_normalization::UnicodeNormalization;

fn base_session(goal: &str) -> Session {
    Session::new(goal, Some("planner_test"))
}

fn app_name(role: AppRole) -> &'static str {
    Planner::app_name_for_role(role)
}

fn opened_app(role: AppRole) -> String {
    format!("Opened app: {}", app_name(role))
}

#[test]
fn summarize_execution_success_for_non_mail_goal() {
    let goal = "Notes를 열고 간단한 메모를 작성하고 done 하세요.";
    let mut session = base_session(goal);
    session.add_step("open_app", &opened_app(AppRole::NotesApp), "success", None);
    session.add_step("type", "Typed '회의 준비'", "success", None);
    let history = vec![
        opened_app(AppRole::NotesApp),
        "Typed '회의 준비'".to_string(),
    ];

    let summary = Planner::summarize_execution(
        goal,
        &session,
        &history,
        true,
        &super::PlannerTimingStats::default(),
    );
    assert!(summary.planner_complete);
    assert!(summary.execution_complete);
    assert!(summary.business_complete);
}

#[test]
fn summarize_execution_fails_if_any_step_failed() {
    let goal = "TextEdit를 열고 문서를 작성하세요.";
    let mut session = base_session(goal);
    session.add_step(
        "open_app",
        &opened_app(AppRole::TextEditor),
        "success",
        None,
    );
    session.add_step("type", "Type failed: blocked by dialog", "failed", None);
    let history = vec![opened_app(AppRole::TextEditor)];

    let summary = Planner::summarize_execution(
        goal,
        &session,
        &history,
        true,
        &super::PlannerTimingStats::default(),
    );
    assert!(summary.planner_complete);
    assert!(!summary.execution_complete);
    assert!(!summary.business_complete);
}

#[test]
fn summarize_execution_treats_shortcut_permission_failure_as_non_blocking() {
    let goal = "노트에서 최근 TODO 정리해줘";
    let mut session = base_session(goal);
    session.add_step("open_app", &opened_app(AppRole::NotesApp), "success", None);
    session.add_step(
            "shortcut",
            "Shortcut 'n' + [\"command\"] | driver execution failed: Shortcut Failed: AppleScript Error: not allowed to send keystrokes (1002)",
            "failed",
            None,
        );
    session.add_step("type", "Typed '오늘 할 일 5개 정리'", "success", None);
    let history = vec![
        opened_app(AppRole::NotesApp),
        "Typed '오늘 할 일 5개 정리'".to_string(),
    ];

    let summary = Planner::summarize_execution(
        goal,
        &session,
        &history,
        true,
        &super::PlannerTimingStats::default(),
    );
    assert!(summary.execution_complete);
    assert!(summary.business_complete);
    assert_eq!(summary.blocking_failed_steps, 0);
}

#[test]
fn summarize_execution_treats_recovered_comparison_type_failure_as_non_blocking() {
    let goal = "OpenClaw와 유사한 macOS 자동화/로컬 에이전트 제품 비교표를 만들어. 근거 링크와 스크린샷도 포함해.";
    let mut session = base_session(goal);
    session.add_step(
        "open_url",
        "Opened URL 'https://www.google.com/search?q=openclaw+alternatives'",
        "success",
        None,
    );
    session.add_step(
        "read",
        "Read '후보 제품 추출' -> 후보 제품 5개",
        "success",
        None,
    );
    session.add_step(
        "open_app",
        &opened_app(AppRole::TextEditor),
        "success",
        None,
    );
    session.add_step(
            "type",
            "Typed 'comparison body' | driver execution failed: Type Failed: AppleScript Error: not allowed to send keystrokes (1002) | Native fallback timed out",
            "failed",
            Some(serde_json::json!({"proof": "textedit_append_text"})),
        );
    session.add_step(
        "report",
        "Reported progress: PRODUCT_COMPARISON_REPORT_EMITTED",
        "success",
        None,
    );
    let history = vec![
        "Opened URL 'https://www.google.com/search?q=openclaw+alternatives'".to_string(),
        "READ_RESULT: 후보 제품 추출 완료".to_string(),
        opened_app(AppRole::TextEditor),
        "EXECUTION_ERROR: Critical type action failed: ... keystroke ... (1002) ...".to_string(),
        "Reported progress: PRODUCT_COMPARISON_REPORT_EMITTED".to_string(),
    ];

    let summary = Planner::summarize_execution(
        goal,
        &session,
        &history,
        true,
        &super::PlannerTimingStats::default(),
    );
    assert!(summary.execution_complete);
    assert!(summary.business_complete);
    assert_eq!(summary.blocking_failed_steps, 0);
}

#[test]
fn summarize_execution_requires_mail_send_confirmation() {
    let goal = "Mail을 열고 이메일을 보내세요.";
    let mut session = base_session(goal);
    session.add_step(
        "open_app",
        &opened_app(AppRole::MailClient),
        "success",
        None,
    );
    session.add_step("type", "Typed 'subject'", "success", None);
    let history = vec![
        opened_app(AppRole::MailClient),
        "Typed 'subject'".to_string(),
    ];

    let summary = Planner::summarize_execution(
        goal,
        &session,
        &history,
        true,
        &super::PlannerTimingStats::default(),
    );
    assert!(summary.mail_send_required);
    assert!(!summary.mail_send_confirmed);
    assert!(!summary.business_complete);
}

#[test]
fn summarize_execution_passes_when_mail_send_confirmed() {
    let goal = "Mail로 보고서를 보내세요.";
    let mut session = base_session(goal);
    session.add_step(
        "open_app",
        &opened_app(AppRole::MailClient),
        "success",
        None,
    );
    session.add_step(
        "mail_send",
        "Mail send completed",
        "success",
        Some(serde_json::json!({
            "send_status": "sent_confirmed",
            "recipient": "qed4950@gmail.com",
            "body_len": 24
        })),
    );
    let history = vec![
        opened_app(AppRole::MailClient),
        "Mail send completed".to_string(),
    ];

    let summary = Planner::summarize_execution(
        goal,
        &session,
        &history,
        true,
        &super::PlannerTimingStats::default(),
    );
    assert!(summary.mail_send_required);
    assert!(summary.mail_send_confirmed);
    assert!(summary.business_complete);
}

#[test]
fn summarize_execution_accepts_pending_then_no_draft_mail_send() {
    let goal = "Mail로 보고서를 보내세요.";
    let mut session = base_session(goal);
    session.add_step(
        "open_app",
        &opened_app(AppRole::MailClient),
        "success",
        None,
    );
    session.add_step(
        "mail_send",
        "Mail send blocked: sent_pending|1|1",
        "failed",
        Some(serde_json::json!({"send_status": "sent_pending"})),
    );
    session.add_step(
        "mail_send",
        "Mail send blocked: no_draft|0|0",
        "failed",
        Some(serde_json::json!({"send_status": "no_draft"})),
    );
    let history = vec![
        opened_app(AppRole::MailClient),
        "Mail send blocked: sent_pending|1|1".to_string(),
        "Mail send blocked: no_draft|0|0".to_string(),
    ];

    let summary = Planner::summarize_execution(
        goal,
        &session,
        &history,
        true,
        &super::PlannerTimingStats::default(),
    );
    assert!(summary.execution_complete);
    assert!(summary.mail_send_required);
    assert!(summary.mail_send_confirmed);
    assert!(summary.business_complete);
}

#[test]
fn summarize_execution_rejects_sent_confirmed_without_body_or_recipient_proof() {
    let goal = "Mail로 보고서를 보내세요.";
    let mut session = base_session(goal);
    session.add_step(
        "open_app",
        &opened_app(AppRole::MailClient),
        "success",
        None,
    );
    session.add_step(
        "mail_send",
        "Mail send completed",
        "success",
        Some(serde_json::json!({"send_status": "sent_confirmed"})),
    );
    let history = vec![
        opened_app(AppRole::MailClient),
        "Mail send completed".to_string(),
    ];

    let summary = Planner::summarize_execution(
        goal,
        &session,
        &history,
        true,
        &super::PlannerTimingStats::default(),
    );
    assert!(summary.mail_send_required);
    assert!(!summary.mail_send_confirmed);
    assert!(!summary.business_complete);
}

#[test]
fn summarize_execution_requires_textedit_save_when_goal_mentions_save() {
    let goal = "TextEdit에서 문서를 작성하고 저장하세요.";
    let mut session = base_session(goal);
    session.add_step(
        "open_app",
        &opened_app(AppRole::TextEditor),
        "success",
        None,
    );
    session.add_step("type", "Typed 'status: in-progress'", "success", None);
    let history = vec![
        opened_app(AppRole::TextEditor),
        "Typed 'status: in-progress'".to_string(),
    ];

    let summary = Planner::summarize_execution(
        goal,
        &session,
        &history,
        true,
        &super::PlannerTimingStats::default(),
    );
    assert!(summary.textedit_write_required);
    assert!(summary.textedit_write_confirmed);
    assert!(summary.textedit_save_required);
    assert!(!summary.textedit_save_confirmed);
    assert!(!summary.business_complete);
}

#[test]
fn summarize_execution_passes_when_textedit_save_confirmed() {
    let goal = "TextEdit에서 문서를 작성하고 저장하세요.";
    let mut session = base_session(goal);
    session.add_step(
        "open_app",
        &opened_app(AppRole::TextEditor),
        "success",
        None,
    );
    session.add_step(
        "type",
        "Typed 'status: in-progress' (textedit body)",
        "success",
        Some(serde_json::json!({"proof": "textedit_append_text"})),
    );
    session.add_step(
        "shortcut",
        "Shortcut 's' + [\"command\"]",
        "success",
        Some(serde_json::json!({"proof": "textedit_save"})),
    );
    let history = vec![
        opened_app(AppRole::TextEditor),
        "Typed 'status: in-progress' (textedit body)".to_string(),
        "Shortcut 's' + [\"command\"]".to_string(),
    ];

    let summary = Planner::summarize_execution(
        goal,
        &session,
        &history,
        true,
        &super::PlannerTimingStats::default(),
    );
    assert!(summary.textedit_write_required);
    assert!(summary.textedit_write_confirmed);
    assert!(summary.textedit_save_required);
    assert!(summary.textedit_save_confirmed);
    assert!(summary.business_complete);
}

#[test]
fn goal_requires_textedit_save_detects_cmd_s_only() {
    let goal = "TextEdit를 열고 문서를 편집한 다음 Cmd+S 로 저장하세요.";
    assert!(Planner::goal_requires_textedit_save(goal));
}

#[test]
fn rewrite_redundant_cmd_n_in_mail_to_send_progress() {
    let goal = "Mail을 열고 이메일을 보내세요.";
    let history = vec![
        "Opened app: Mail".to_string(),
        "Shortcut 'n' + [\"command\"] (Created new item)".to_string(),
        "Typed 'Digest 제목' (mail subject)".to_string(),
        "Pasted clipboard contents (mail body)".to_string(),
    ];
    let mut plan = serde_json::json!({
        "action": "shortcut",
        "key": "n",
        "modifiers": ["command"]
    });

    Planner::maybe_rewrite_redundant_new_item_shortcut(goal, &history, &mut plan);

    assert_eq!(plan["action"].as_str(), Some("mail_send"));
}

#[test]
fn goal_requires_textedit_save_does_not_match_cmd_shift_d() {
    let goal = "TextEdit를 열고 내용을 복사한 뒤 Mail에서 보내기(Cmd+Shift+D)로 발송하세요.";
    assert!(!Planner::goal_requires_textedit_save(goal));
}

#[test]
fn summarize_execution_accepts_textedit_save_proof_without_shortcut_text() {
    let goal = "TextEdit에서 문서를 작성하고 저장하세요.";
    let mut session = base_session(goal);
    session.add_step(
        "open_app",
        &opened_app(AppRole::TextEditor),
        "success",
        None,
    );
    session.add_step(
        "type",
        "Typed 'status: in-progress' (textedit body)",
        "success",
        Some(serde_json::json!({"proof": "textedit_append_text"})),
    );
    session.add_step(
        "shortcut",
        "Saved file in TextEdit",
        "success",
        Some(serde_json::json!({"proof": "textedit_save"})),
    );
    let history = vec![
        opened_app(AppRole::TextEditor),
        "Typed 'status: in-progress' (textedit body)".to_string(),
        "Saved file in TextEdit".to_string(),
    ];

    let summary = Planner::summarize_execution(
        goal,
        &session,
        &history,
        true,
        &super::PlannerTimingStats::default(),
    );
    assert!(summary.textedit_save_required);
    assert!(summary.textedit_save_confirmed);
    assert!(summary.business_complete);
}

#[test]
fn fallback_plan_reads_calendar_before_telegram_send() {
    let goal = "캘린더를 열고 오늘 일정 핵심만 텔레그램으로 보내줘";
    let history = vec!["Opened app: Calendar".to_string()];
    let plan = Planner::fallback_plan_from_goal(goal, &history).unwrap();
    assert_eq!(plan["action"].as_str(), Some("read"));
}

#[test]
fn fallback_plan_sends_telegram_after_read_result() {
    let goal = "캘린더를 열고 오늘 일정 핵심만 텔레그램으로 보내줘";
    let history = vec![
        "Opened app: Calendar".to_string(),
        "READ_RESULT: 오늘 일정은 3건입니다.".to_string(),
    ];
    let plan = Planner::fallback_plan_from_goal(goal, &history).unwrap();
    assert_eq!(plan["action"].as_str(), Some("telegram_send"));
}

#[test]
fn ordered_apps_in_goal_maps_korean_aliases() {
    let goal = "메모장 열어서 박대엽이라고 써줘";
    let apps = Planner::ordered_apps_in_goal(goal);
    assert_eq!(apps, vec![app_name(AppRole::NotesApp)]);
}

#[test]
fn deterministic_autoplan_enabled_for_simple_korean_notes_write() {
    let goal = "메모장 열어서 박대엽이라고 써줘";
    assert!(Planner::should_use_deterministic_goal_autoplan(goal));
}

#[test]
fn deterministic_autoplan_enabled_for_simple_korean_notes_open() {
    let goal = "노트 열어줘봐";
    assert!(Planner::should_use_deterministic_goal_autoplan(goal));
}

#[test]
fn deterministic_autoplan_enabled_for_nfd_korean_notes_open() {
    let goal_nfd: String = "노트 열어줘봐".nfd().collect();
    assert!(Planner::should_use_deterministic_goal_autoplan(&goal_nfd));
}

#[test]
fn soft_planner_failure_detects_timeout_and_rate_limit() {
    assert!(Planner::is_soft_planner_failure(
        "planner plan_vision_step timeout after 20s"
    ));
    assert!(Planner::is_soft_planner_failure(
        "HTTP 429 rate limit exceeded"
    ));
}

#[test]
fn soft_planner_failure_ignores_schema_error() {
    assert!(!Planner::is_soft_planner_failure(
        "SCHEMA_ERROR: missing required key"
    ));
}

#[test]
fn type_permission_error_is_recoverable_for_abort_policy() {
    let err = anyhow::anyhow!(
            "Critical type action failed: Type Failed: AppleScript Error: osascript keystroke not allowed (1002) | Native fallback timed out"
        );
    assert!(!Planner::should_abort_on_execution_error(&err));
}

#[test]
fn goal_has_open_signal_supports_multilingual_variants() {
    assert!(Planner::goal_has_open_signal("abre notes por favor"));
    assert!(Planner::goal_has_open_signal("ouvre notes"));
    assert!(Planner::goal_has_open_signal("メモを開いて"));
    assert!(Planner::goal_has_open_signal("打开 notes"));
}

#[test]
fn deterministic_autoplan_enabled_for_ai_news_to_notion_goal() {
    let goal = "구글에서 현재가장 트렌디한 ai 관련 기사 찾아서 llm 으로 요약한후 노션에 정리";
    assert!(Planner::should_use_deterministic_goal_autoplan(goal));
}

#[test]
fn deterministic_autoplan_enabled_for_sports_news_to_notion_goal() {
    let goal = "스포츠 뉴스 5개 선정해서 노션에 정리해줘";
    assert!(Planner::should_use_deterministic_goal_autoplan(goal));
}

#[test]
fn deterministic_autoplan_enabled_for_todo_summary_goal() {
    let goal = "노트에 오늘 할 일 5개 정리해줘";
    assert!(Planner::should_use_deterministic_goal_autoplan(goal));
}

#[test]
fn deterministic_autoplan_enabled_for_general_n8n_workflow_goal() {
    let goal = "n8n으로 테스트 워크플로우를 만들고 실행해줘";
    assert!(Planner::should_use_deterministic_goal_autoplan(goal));
}

#[test]
fn deterministic_autoplan_enabled_for_product_comparison_research_goal() {
    let goal = "OpenClaw와 유사한 macOS 자동화/로컬 에이전트 제품 5개를 찾아 기능 비교표를 만들어. 비교 항목은 보안, 워크플로우 오케스트레이션, 감사로그, UI 제어 방식, 가격/플랜이야.";
    assert!(Planner::goal_targets_product_comparison_research(goal));
    assert!(Planner::should_use_deterministic_goal_autoplan(goal));
}

#[test]
fn fallback_plan_todo_summary_notes_flow_order() {
    let goal = "노트에 오늘 할 일 5개 정리해줘";

    let step1 = Planner::fallback_plan_from_goal(goal, &[]).unwrap();
    assert_eq!(step1["action"].as_str(), Some("open_app"));
    assert_eq!(step1["name"].as_str(), Some(app_name(AppRole::NotesApp)));

    let history_after_open = vec![opened_app(AppRole::NotesApp)];
    let step2 = Planner::fallback_plan_from_goal(goal, &history_after_open).unwrap();
    assert_eq!(step2["action"].as_str(), Some("shortcut"));
    assert_eq!(step2["key"].as_str(), Some("n"));

    let history_after_shortcut = vec![
        opened_app(AppRole::NotesApp),
        "Shortcut 'n' + [\"command\"] (Created new item)".to_string(),
    ];
    let step3 = Planner::fallback_plan_from_goal(goal, &history_after_shortcut).unwrap();
    assert_eq!(step3["action"].as_str(), Some("type"));

    let todo_header = format!("오늘 할 일 체크리스트 ({})", Utc::now().format("%Y-%m-%d"));
    let history_after_type = vec![
        opened_app(AppRole::NotesApp),
        "Shortcut 'n' + [\"command\"] (Created new item)".to_string(),
        format!("Typed '{}'", todo_header),
    ];
    let step4 = Planner::fallback_plan_from_goal(goal, &history_after_type).unwrap();
    assert_eq!(step4["action"].as_str(), Some("done"));
}

#[test]
fn fallback_plan_ai_news_to_notion_flow_order() {
    let goal = "구글에서 현재가장 트렌디한 ai 관련 기사 찾아서 llm 으로 요약한후 노션에 정리";

    let step1 = Planner::fallback_plan_from_goal(goal, &[]).unwrap();
    assert_eq!(step1["action"].as_str(), Some("open_url"));
    assert!(step1["url"]
        .as_str()
        .unwrap_or("")
        .contains("google.com/search"));

    let history_after_google =
        vec!["Opened URL 'https://www.google.com/search?q=trending+AI+news'".to_string()];
    let step2 = Planner::fallback_plan_from_goal(goal, &history_after_google).unwrap();
    if super::util::notion_api_ready() {
        assert_eq!(step2["action"].as_str(), Some("notion_write"));
        assert!(step2["content"]
            .as_str()
            .unwrap_or("")
            .contains("AI 뉴스 기사 요약"));
        let history_after_notion_write = vec![
            "Opened URL 'https://www.google.com/search?q=trending+AI+news'".to_string(),
            "Notion page created: https://www.notion.so/abcd1234".to_string(),
        ];
        let step3 = Planner::fallback_plan_from_goal(goal, &history_after_notion_write).unwrap();
        assert_eq!(step3["action"].as_str(), Some("done"));
    } else {
        assert_eq!(step2["action"].as_str(), Some("open_app"));
        assert_eq!(step2["name"].as_str(), Some("Notion"));

        let history_after_notion = vec![
            "Opened URL 'https://www.google.com/search?q=trending+AI+news'".to_string(),
            "Opened app: Notion".to_string(),
        ];
        let step3 = Planner::fallback_plan_from_goal(goal, &history_after_notion).unwrap();
        assert_eq!(step3["action"].as_str(), Some("shortcut"));
        assert_eq!(step3["key"].as_str(), Some("n"));

        let history_after_new_item = vec![
            "Opened URL 'https://www.google.com/search?q=trending+AI+news'".to_string(),
            "Opened app: Notion".to_string(),
            "Shortcut 'n' + [\"command\"] (Created new item)".to_string(),
        ];
        let step4 = Planner::fallback_plan_from_goal(goal, &history_after_new_item).unwrap();
        assert_eq!(step4["action"].as_str(), Some("type"));
        assert!(step4["text"]
            .as_str()
            .unwrap_or("")
            .contains("AI 뉴스 기사 요약"));
    }
}

#[test]
fn fallback_plan_sports_news_to_notion_uses_topic_search() {
    let goal = "스포츠 뉴스 5개 선정해서 노션에 정리해줘";
    let step1 = Planner::fallback_plan_from_goal(goal, &[]).unwrap();
    assert_eq!(step1["action"].as_str(), Some("open_url"));
    let url = step1["url"].as_str().unwrap_or("");
    assert!(url.contains("google.com/search?q="));
    assert!(url.contains("%EC%8A%A4%ED%8F%AC%EC%B8%A0") || url.contains("sports"));
}

#[test]
fn fallback_plan_sports_news_to_notion_progresses_after_encoded_search() {
    let goal = "스포츠 뉴스 5개 선정해서 노션에 정리해줘";
    let history_after_search = vec![
        "Opened URL 'https://www.google.com/search?q=trending+%EC%8A%A4%ED%8F%AC%EC%B8%A0+news'"
            .to_string(),
    ];
    let step2 = Planner::fallback_plan_from_goal(goal, &history_after_search).unwrap();
    assert_ne!(step2["action"].as_str(), Some("open_url"));
    if super::util::notion_api_ready() {
        assert_eq!(step2["action"].as_str(), Some("notion_write"));
    } else {
        assert_eq!(step2["action"].as_str(), Some("open_app"));
        assert_eq!(step2["name"].as_str(), Some("Notion"));
    }
}

#[test]
fn fallback_plan_product_comparison_flow_order() {
    let goal = "OpenClaw와 유사한 macOS 자동화/로컬 에이전트 제품 5개를 찾아 기능 비교표를 만들어. 근거 링크와 스크린샷도 포함해.";

    let step1 = Planner::fallback_plan_from_goal(goal, &[]).unwrap();
    assert_eq!(step1["action"].as_str(), Some("open_url"));
    assert!(step1["url"]
        .as_str()
        .unwrap_or("")
        .contains("google.com/search"));

    let history_after_search =
        vec!["Opened URL 'https://www.google.com/search?q=openclaw+alternatives'".to_string()];
    let step2 = Planner::fallback_plan_from_goal(goal, &history_after_search).unwrap();
    assert_eq!(step2["action"].as_str(), Some("read"));

    let target_app = Planner::text_staging_app();
    let mut history_after_read = vec![
        "Opened URL 'https://www.google.com/search?q=openclaw+alternatives'".to_string(),
        "READ_RESULT: 후보 제품 추출 완료".to_string(),
    ];
    let step3 = Planner::fallback_plan_from_goal(goal, &history_after_read).unwrap();
    assert_eq!(step3["action"].as_str(), Some("open_app"));
    assert_eq!(step3["name"].as_str(), Some(target_app));
    history_after_read.push(format!("Opened app: {}", target_app));

    if Planner::app_is_role(target_app, AppRole::NotesApp) {
        let step4 = Planner::fallback_plan_from_goal(goal, &history_after_read).unwrap();
        assert_eq!(step4["action"].as_str(), Some("shortcut"));
        history_after_read.push("Shortcut 'n' + [\"command\"] (Created new item)".to_string());
    }

    let step_type = Planner::fallback_plan_from_goal(goal, &history_after_read).unwrap();
    assert_eq!(step_type["action"].as_str(), Some("type"));
    let header = format!(
        "macOS 로컬 자동화/에이전트 제품 비교표 ({})",
        Utc::now().format("%Y-%m-%d")
    );
    assert!(step_type["text"].as_str().unwrap_or("").contains(&header));
    history_after_read.push(format!("Typed '{}'", header));

    let step_done = Planner::fallback_plan_from_goal(goal, &history_after_read).unwrap();
    assert_eq!(step_done["action"].as_str(), Some("done"));
}

#[test]
fn fallback_plan_product_comparison_reports_when_type_permission_blocked() {
    let goal = "OpenClaw와 유사한 macOS 자동화/로컬 에이전트 제품 5개를 찾아 기능 비교표를 만들어. 마지막 줄에 \"RUN_SCOPE_20260226_777777\"를 정확히 입력하세요.";
    let history = vec![
            "Opened URL 'https://www.google.com/search?q=openclaw+alternatives'".to_string(),
            "READ_RESULT: 후보 제품 추출 완료".to_string(),
            opened_app(AppRole::TextEditor),
            "EXECUTION_ERROR: Critical type action failed: Type Failed: AppleScript Error: keystroke not allowed (1002) | Native fallback timed out".to_string(),
        ];
    let step = Planner::fallback_plan_from_goal(goal, &history).unwrap();
    assert_eq!(step["action"].as_str(), Some("report"));
    assert!(step["message"]
        .as_str()
        .unwrap_or("")
        .contains("PRODUCT_COMPARISON_REPORT_EMITTED"));
    assert!(step["message"]
        .as_str()
        .unwrap_or("")
        .contains("RUN_SCOPE_20260226_777777"));
}

#[test]
fn fallback_plan_n8n_workflow_create_then_execute_then_done() {
    let goal = "n8n으로 테스트 워크플로우를 만들고 실행해줘";

    let step1 = Planner::fallback_plan_from_goal(goal, &[]).unwrap();
    assert_eq!(step1["action"].as_str(), Some("n8n_create_workflow"));

    let history_after_create =
        vec!["n8n workflow created: wf_123 (http://localhost:5678/workflow/wf_123)".to_string()];
    let step2 = Planner::fallback_plan_from_goal(goal, &history_after_create).unwrap();
    assert_eq!(step2["action"].as_str(), Some("n8n_execute_workflow"));

    let history_after_execute = vec![
        "n8n workflow created: wf_123 (http://localhost:5678/workflow/wf_123)".to_string(),
        "n8n execution completed: workflow_id=wf_123 execution_id=ex_1 status=success".to_string(),
    ];
    let step3 = Planner::fallback_plan_from_goal(goal, &history_after_execute).unwrap();
    assert_eq!(step3["action"].as_str(), Some("done"));
}

#[test]
fn fallback_plan_types_unquoted_korean_payload_in_notes() {
    let goal = "메모장 열어서 박대엽이라고 써줘";
    let history = vec![opened_app(AppRole::NotesApp)];
    let plan = Planner::fallback_plan_from_goal(goal, &history).unwrap();
    assert_eq!(plan["action"].as_str(), Some("type"));
    assert_eq!(plan["app"].as_str(), Some(app_name(AppRole::NotesApp)));
    assert_eq!(plan["text"].as_str(), Some("박대엽"));
}

#[test]
fn run_scope_marker_is_not_treated_as_text_payload_fragment() {
    let goal = "메모장 열어서 마지막 줄에 \"RUN_SCOPE_20260226_123456\"를 정확히 입력하세요.";
    let fragments = Planner::extract_goal_text_fragments(goal);
    assert!(!fragments.iter().any(|f| f.starts_with("RUN_SCOPE_")));
    assert_eq!(
        Planner::goal_run_scope_marker(goal).as_deref(),
        Some("RUN_SCOPE_20260226_123456")
    );
}
