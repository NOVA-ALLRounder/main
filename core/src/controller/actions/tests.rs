use super::ActionRunner;
use serde_json::json;
use serial_test::serial;

#[test]
fn normalize_email_candidate_strips_korean_particle_suffix() {
    let got = ActionRunner::normalize_email_candidate("\"qed4950@gmail.com\"를");
    assert_eq!(got.as_deref(), Some("qed4950@gmail.com"));
}

#[test]
fn extract_mail_recipient_from_goal_uses_plain_email() {
    let goal = "받는 사람에 \"qed4950@gmail.com\"를 입력하고 보내기(Cmd+Shift+D)로 발송하세요.";
    let got = ActionRunner::extract_mail_recipient_from_goal(goal);
    assert_eq!(got.as_deref(), Some("qed4950@gmail.com"));
}

#[test]
fn action_idempotency_key_scopes_cmd_n_to_app_context() {
    let plan_with_app = json!({
        "action": "shortcut",
        "key": "n",
        "modifiers": ["command"],
        "app": "Mail"
    });
    let key =
        ActionRunner::action_idempotency_key("shortcut", &plan_with_app, "Mail 초안 작성", &[]);
    assert_eq!(key.as_deref(), Some("shortcut:mail:command+n:new_item"));

    let history = vec!["Opened app: Notes".to_string()];
    let plan_without_app = json!({
        "action": "shortcut",
        "key": "n",
        "modifiers": ["command"]
    });
    let key_from_history =
        ActionRunner::action_idempotency_key("shortcut", &plan_without_app, "메모 작성", &history);
    assert_eq!(
        key_from_history.as_deref(),
        Some("shortcut:notes:command+n:new_item")
    );

    let key_from_goal = ActionRunner::action_idempotency_key(
        "shortcut",
        &plan_without_app,
        "메일 새 창 만들어줘",
        &[],
    );
    assert_eq!(
        key_from_goal.as_deref(),
        Some("shortcut:mail:command+n:new_item")
    );
}

#[test]
fn redundant_cmd_n_skip_is_scoped_to_same_app_context() {
    let history = vec![
        "Opened app: Mail".to_string(),
        "Shortcut 'n' + [\"command\"] (Created new item)".to_string(),
        "Typed 'Subject' (mail subject)".to_string(),
    ];
    assert!(ActionRunner::should_skip_redundant_cmd_n(&history, "Mail"));
    assert!(!ActionRunner::should_skip_redundant_cmd_n(
        &history, "Notes"
    ));
}

#[test]
fn history_recent_cmd_n_created_count_tracks_window_flood() {
    let history = vec![
        "Opened app: Mail".to_string(),
        "Shortcut 'n' + [\"command\"] (Created new item)".to_string(),
        "Typed 'Subject A'".to_string(),
        "Shortcut 'n' + [\"command\"] (Created new item)".to_string(),
        "Typed 'Subject B'".to_string(),
        "Shortcut 'n' + [\"command\"] (Created new item)".to_string(),
    ];
    let count = ActionRunner::history_recent_cmd_n_created_count(&history, "Mail", 16);
    assert_eq!(count, 3);
}

#[test]
fn history_recent_cmd_n_created_count_is_app_scoped() {
    let history = vec![
        "Opened app: Mail".to_string(),
        "Shortcut 'n' + [\"command\"] (Created new item)".to_string(),
        "Opened app: Notes".to_string(),
        "Shortcut 'n' + [\"command\"] (Created new item)".to_string(),
        "Shortcut 'n' + [\"command\"] (Created new item)".to_string(),
        "Opened app: Mail".to_string(),
        "Shortcut 'n' + [\"command\"] (Created new item)".to_string(),
    ];

    let mail_count = ActionRunner::history_recent_cmd_n_created_count(&history, "Mail", 32);
    let notes_count = ActionRunner::history_recent_cmd_n_created_count(&history, "Notes", 32);

    assert_eq!(mail_count, 2);
    assert_eq!(notes_count, 2);
}

#[test]
#[serial]
fn cmd_n_window_flood_limit_for_mail_defaults_to_one() {
    std::env::remove_var("STEER_CMD_N_WINDOW_FLOOD_LIMIT");
    std::env::remove_var("STEER_CMD_N_WINDOW_FLOOD_LIMIT_MAIL");
    let got = ActionRunner::cmd_n_window_flood_limit_for_app("Mail");
    assert_eq!(got, 1);
}

#[test]
#[serial]
fn cmd_n_window_flood_limit_for_app_specific_env_overrides_default() {
    std::env::set_var("STEER_CMD_N_WINDOW_FLOOD_LIMIT", "4");
    std::env::set_var("STEER_CMD_N_WINDOW_FLOOD_LIMIT_MAIL", "2");
    let mail = ActionRunner::cmd_n_window_flood_limit_for_app("Mail");
    let notes = ActionRunner::cmd_n_window_flood_limit_for_app("Notes");
    std::env::remove_var("STEER_CMD_N_WINDOW_FLOOD_LIMIT_MAIL");
    std::env::remove_var("STEER_CMD_N_WINDOW_FLOOD_LIMIT");
    assert_eq!(mail, 2);
    assert_eq!(notes, 4);
}

#[test]
#[serial]
fn mail_max_outgoing_for_auto_draft_defaults_and_clamps() {
    std::env::remove_var("STEER_MAIL_MAX_OUTGOING_FOR_AUTO_DRAFT");
    assert_eq!(ActionRunner::mail_max_outgoing_for_auto_draft(), 8);

    std::env::set_var("STEER_MAIL_MAX_OUTGOING_FOR_AUTO_DRAFT", "0");
    assert_eq!(ActionRunner::mail_max_outgoing_for_auto_draft(), 1);

    std::env::set_var("STEER_MAIL_MAX_OUTGOING_FOR_AUTO_DRAFT", "999");
    assert_eq!(ActionRunner::mail_max_outgoing_for_auto_draft(), 64);

    std::env::remove_var("STEER_MAIL_MAX_OUTGOING_FOR_AUTO_DRAFT");
}

#[test]
fn tracked_mail_draft_is_detected_even_without_recent_cmd_n_phrase() {
    let history = vec![
        "Opened app: Mail".to_string(),
        "MAIL_DRAFT_ID:123456".to_string(),
        "Typed '안건 공유' (mail subject)".to_string(),
    ];
    assert!(ActionRunner::has_tracked_mail_draft(&history));
    assert!(!ActionRunner::should_skip_redundant_cmd_n(
        &history, "Notes"
    ));
}

#[test]
fn history_recent_open_for_app_uses_latest_open_entry() {
    let history = vec![
        "Opened app: Notes".to_string(),
        "Typed 'hello' (notes body)".to_string(),
        "Opened app: Mail".to_string(),
    ];
    assert!(ActionRunner::history_has_recent_open_for_app(
        &history, "Mail"
    ));
    assert!(!ActionRunner::history_has_recent_open_for_app(
        &history, "Notes"
    ));
}

#[test]
fn single_fire_cmd_n_idempotency_survives_long_step_history() {
    let mut session = crate::session_store::Session::new("mail 작성", Some("test"));
    session.add_step(
        "shortcut",
        "Shortcut 'n' + [\"command\"] (Created new item)",
        "success",
        Some(json!({
            "proof": "created_new_item",
            "idempotency_key": "shortcut:mail:command+n:new_item"
        })),
    );
    for i in 0..30usize {
        session.add_step(
            "type",
            &format!("noise step {}", i),
            "success",
            Some(json!({"idempotency_key": format!("noise:{}", i)})),
        );
    }
    assert!(ActionRunner::idempotency_success_hit(
        &session,
        "shortcut:mail:command+n:new_item",
        None
    ));
    assert!(!ActionRunner::idempotency_recent_hit(
        &session,
        "shortcut:mail:command+n:new_item",
        8
    ));
}

#[test]
fn session_cmd_n_stats_counts_attempts_and_successes_per_app() {
    let mut session = crate::session_store::Session::new("mail 작성", Some("test"));
    session.add_step(
        "shortcut",
        "Shortcut 'n' + [\"command\"] failed",
        "failed",
        Some(json!({"idempotency_key": "shortcut:mail:command+n:new_item"})),
    );
    session.add_step(
        "shortcut",
        "Shortcut 'n' + [\"command\"] (Created new item)",
        "success",
        Some(json!({"idempotency_key": "shortcut:mail:command+n:new_item"})),
    );
    session.add_step(
        "shortcut",
        "Shortcut 'n' + [\"command\"] (Created new item)",
        "success",
        Some(json!({"idempotency_key": "shortcut:notes:command+n:new_item"})),
    );

    let (mail_attempts, mail_successes) = ActionRunner::session_cmd_n_stats(&session, "Mail");
    let (notes_attempts, notes_successes) = ActionRunner::session_cmd_n_stats(&session, "Notes");
    assert_eq!((mail_attempts, mail_successes), (2, 1));
    assert_eq!((notes_attempts, notes_successes), (1, 1));
}

#[test]
fn strip_markup_for_mail_body_removes_basic_html_tags() {
    let raw = "<div>Title</div><br/>line 2&nbsp;&amp;&lt;ok&gt;";
    let got = ActionRunner::strip_markup_for_mail_body(raw);
    assert!(got.contains("Title"));
    assert!(got.contains("line 2"));
    assert!(got.contains("&<ok>"));
    assert!(!got.contains("<div>"));
}

#[test]
fn goal_mentions_mail_matches_korean_and_english() {
    assert!(ActionRunner::goal_mentions_mail("Mail 앱에서 보내줘"));
    assert!(ActionRunner::goal_mentions_mail("gmail digest"));
    assert!(!ActionRunner::goal_mentions_mail("notes only"));
}

#[test]
fn goal_targets_news_to_notion_matches_non_ai_topic() {
    assert!(ActionRunner::goal_targets_ai_news_to_notion(
        "스포츠 뉴스 5개 선정해서 노션에 정리해줘"
    ));
    assert!(!ActionRunner::goal_targets_ai_news_to_notion(
        "스포츠 뉴스 5개만 보여줘"
    ));
}

#[test]
fn infer_news_topic_from_goal_prefers_domain_topic() {
    assert_eq!(
        ActionRunner::infer_news_topic_from_goal(
            "구글에서 현재가장 트렌디한 ai 관련 기사 찾아서 요약 후 노션 정리"
        ),
        "AI"
    );
    assert_eq!(
        ActionRunner::infer_news_topic_from_goal("스포츠 뉴스 5개 선정해서 노션에 정리해줘"),
        "스포츠"
    );
}

#[test]
fn infer_news_item_count_respects_goal_count_and_default() {
    assert_eq!(
        ActionRunner::infer_news_item_count(
            "최근 ai 트렌드 중요한거 5개 llm으로 요약해서 노션에 정리해줘"
        ),
        5
    );
    assert_eq!(
        ActionRunner::infer_news_item_count("AI 뉴스 요약해서 노션에 정리해줘"),
        5
    );
}

#[test]
fn notion_blocks_embed_clickable_url_links() {
    let blocks = ActionRunner::notion_paragraph_blocks("링크: https://example.com/a?b=1");
    assert!(!blocks.is_empty());
    let rich = blocks[0]["paragraph"]["rich_text"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let has_link = rich.iter().any(|seg| {
        seg["text"]["link"]["url"]
            .as_str()
            .map(|u| u == "https://example.com/a?b=1")
            .unwrap_or(false)
    });
    assert!(has_link, "expected a clickable rich_text link segment");
}

#[test]
fn notion_blocks_strip_trailing_punctuation_from_link_target() {
    let blocks = ActionRunner::notion_paragraph_blocks("원문: https://example.com/path.");
    let rich = blocks[0]["paragraph"]["rich_text"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let has_clean_link = rich.iter().any(|seg| {
        seg["text"]["link"]["url"]
            .as_str()
            .map(|u| u == "https://example.com/path")
            .unwrap_or(false)
    });
    assert!(
        has_clean_link,
        "expected trailing punctuation to be excluded from url"
    );
}
