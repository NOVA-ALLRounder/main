use super::*;

#[test]
fn recommendation_visibility_keeps_manual_pending_and_caps_auto_pending() {
    let manual = crate::db::Recommendation {
        id: 1,
        status: "pending".to_string(),
        title: "Manual Workflow".to_string(),
        summary: "manual".to_string(),
        trigger: "manual".to_string(),
        actions: vec![],
        n8n_prompt: "manual".to_string(),
        confidence: 0.6,
        workflow_id: None,
        workflow_json: None,
        evidence: vec![],
        pattern_id: None,
        last_error: None,
        snoozed_until: None,
        category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
        business_score: 0.1,
        feedback_status: None,
        feedback_note: None,
        feedback_count: 0,
        last_feedback_at: None,
    };
    let auto = (0..6)
        .map(|idx| crate::db::Recommendation {
            id: idx + 10,
            status: "pending".to_string(),
            title: format!("Auto Workflow {}", idx),
            summary: "auto".to_string(),
            trigger: format!("pattern {}", idx),
            actions: vec![],
            n8n_prompt: "auto".to_string(),
            confidence: 0.84 + (idx as f64 * 0.01),
            workflow_id: None,
            workflow_json: None,
            evidence: vec![
                format!("Frequency: Found {} occurrences", 6 + idx),
                "Span: 4 distinct day(s)".to_string(),
                "policy.reason=strong_work_signals=slack,calendar".to_string(),
                "policy.reason=pattern.context_score=0.82".to_string(),
                "policy.reason=pattern.work_context=strong".to_string(),
            ],
            pattern_id: Some(format!("pattern-{}", idx)),
            last_error: None,
            snoozed_until: None,
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.75 + (idx as f64 * 0.02),
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        })
        .collect::<Vec<_>>();

    let mut input = vec![manual];
    input.extend(auto);
    let visible = limit_visible_recommendations(input, Some("work"), &[]);

    assert_eq!(visible.len(), 6);
    assert_eq!(visible[0].title, "Manual Workflow");
    assert!(visible.iter().any(|rec| rec.title == "Auto Workflow 5"));
    assert!(!visible.iter().any(|rec| rec.title == "Auto Workflow 0"));
}

#[test]
fn recommendation_visibility_prefers_aligned_delivery_channel() {
    let history = vec![
        crate::db::Recommendation {
            id: 1,
            status: "approved".to_string(),
            title: "Notion Notes".to_string(),
            summary: "Save notes to Notion".to_string(),
            trigger: "meeting".to_string(),
            actions: vec![],
            n8n_prompt: "Create a workflow that writes meeting notes to Notion.".to_string(),
            confidence: 0.8,
            workflow_id: Some("wf-1".to_string()),
            workflow_json: None,
            evidence: vec![],
            pattern_id: Some("hist-notion".to_string()),
            last_error: None,
            snoozed_until: None,
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.8,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        },
        crate::db::Recommendation {
            id: 2,
            status: "rejected".to_string(),
            title: "Telegram Digest".to_string(),
            summary: "Send digest to Telegram".to_string(),
            trigger: "digest".to_string(),
            actions: vec![],
            n8n_prompt: "Create a workflow that sends updates to Telegram.".to_string(),
            confidence: 0.8,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![],
            pattern_id: Some("hist-telegram".to_string()),
            last_error: None,
            snoozed_until: None,
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.8,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        },
    ];
    let pending = vec![
        crate::db::Recommendation {
            id: 10,
            status: "pending".to_string(),
            title: "Telegram Status Digest".to_string(),
            summary: "Send digest to Telegram".to_string(),
            trigger: "digest".to_string(),
            actions: vec![],
            n8n_prompt: "Create a workflow that sends a work digest to Telegram.".to_string(),
            confidence: 0.91,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![
                "Frequency: Found 6 occurrences".to_string(),
                "Span: 4 distinct day(s)".to_string(),
                "policy.reason=strong_work_signals=slack,gmail".to_string(),
                "policy.reason=pattern.context_score=0.80".to_string(),
                "policy.reason=pattern.work_context=strong".to_string(),
            ],
            pattern_id: Some("pending-telegram".to_string()),
            last_error: None,
            snoozed_until: None,
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.90,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        },
        crate::db::Recommendation {
            id: 11,
            status: "pending".to_string(),
            title: "Notion Meeting Brief".to_string(),
            summary: "Save briefing to Notion".to_string(),
            trigger: "meeting".to_string(),
            actions: vec![],
            n8n_prompt: "Create a workflow that stores meeting briefs in Notion.".to_string(),
            confidence: 0.88,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![
                "Frequency: Found 5 occurrences".to_string(),
                "Span: 4 distinct day(s)".to_string(),
                "policy.reason=strong_work_signals=notion,calendar".to_string(),
                "policy.reason=pattern.context_score=0.81".to_string(),
                "policy.reason=pattern.work_context=strong".to_string(),
            ],
            pattern_id: Some("pending-notion".to_string()),
            last_error: None,
            snoozed_until: None,
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.86,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        },
    ];

    let visible = limit_visible_recommendations(pending, Some("work"), &history);

    assert_eq!(visible[0].title, "Notion Meeting Brief");
    assert_eq!(visible[1].title, "Telegram Status Digest");
}

#[test]
fn recommendation_visibility_prefers_approval_ready_auto_items() {
    let ready = crate::db::Recommendation {
        id: 10,
        status: "pending".to_string(),
        title: "Ready Workflow".to_string(),
        summary: "Detected 6 repeats across 4 distinct day(s).".to_string(),
        trigger: "ready".to_string(),
        actions: vec![],
        n8n_prompt: "Create a workflow that uses Slack and Calendar.".to_string(),
        confidence: 0.90,
        workflow_id: None,
        workflow_json: None,
        evidence: vec![
            "Frequency: Found 6 occurrences".to_string(),
            "Span: 4 distinct day(s)".to_string(),
            "policy.reason=strong_work_signals=slack,calendar".to_string(),
            "policy.reason=pattern.context_score=0.82".to_string(),
            "policy.reason=pattern.work_context=strong".to_string(),
        ],
        pattern_id: Some("ready-pattern".to_string()),
        last_error: None,
        snoozed_until: None,
        category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
        business_score: 0.82,
        feedback_status: None,
        feedback_note: None,
        feedback_count: 0,
        last_feedback_at: None,
    };
    let weak = crate::db::Recommendation {
        id: 11,
        status: "pending".to_string(),
        title: "Weak Workflow".to_string(),
        summary: "Detected 3 repeats across 1 distinct day(s).".to_string(),
        trigger: "weak".to_string(),
        actions: vec![],
        n8n_prompt: "Create a workflow that uses VSCode.".to_string(),
        confidence: 0.91,
        workflow_id: None,
        workflow_json: None,
        evidence: vec![
            "Frequency: Found 3 occurrences".to_string(),
            "Span: 1 distinct day(s)".to_string(),
            "policy.reason=support_work_signals=vscode,terminal".to_string(),
        ],
        pattern_id: Some("weak-pattern".to_string()),
        last_error: None,
        snoozed_until: None,
        category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
        business_score: 0.90,
        feedback_status: None,
        feedback_note: None,
        feedback_count: 0,
        last_feedback_at: None,
    };

    let visible = limit_visible_recommendations(vec![weak, ready], Some("work"), &[]);

    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].title, "Ready Workflow");
}

#[test]
fn recommendation_visibility_hides_non_ready_auto_pending_items() {
    let weak = crate::db::Recommendation {
        id: 11,
        status: "pending".to_string(),
        title: "Weak Workflow".to_string(),
        summary: "Detected 3 repeats across 1 distinct day(s).".to_string(),
        trigger: "weak".to_string(),
        actions: vec![],
        n8n_prompt: "Create a workflow that uses VSCode.".to_string(),
        confidence: 0.91,
        workflow_id: None,
        workflow_json: None,
        evidence: vec![
            "Frequency: Found 3 occurrences".to_string(),
            "Span: 1 distinct day(s)".to_string(),
            "policy.reason=support_work_signals=vscode,terminal".to_string(),
        ],
        pattern_id: Some("weak-pattern".to_string()),
        last_error: None,
        snoozed_until: None,
        category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
        business_score: 0.90,
        feedback_status: None,
        feedback_note: None,
        feedback_count: 0,
        last_feedback_at: None,
    };

    let visible = limit_visible_recommendations(vec![weak], Some("work"), &[]);

    assert!(visible.is_empty());
}

#[test]
fn recommendation_effective_status_maps_snoozed_pending_to_later() {
    let snoozed = crate::db::Recommendation {
        id: 1,
        status: "pending".to_string(),
        title: "Snoozed Workflow".to_string(),
        summary: "manual".to_string(),
        trigger: "manual".to_string(),
        actions: vec![],
        n8n_prompt: "manual".to_string(),
        confidence: 0.6,
        workflow_id: None,
        workflow_json: None,
        evidence: vec![],
        pattern_id: None,
        last_error: None,
        snoozed_until: Some((chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339()),
        category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
        business_score: 0.1,
        feedback_status: None,
        feedback_note: None,
        feedback_count: 0,
        last_feedback_at: None,
    };

    assert_eq!(recommendation_effective_status(&snoozed), "later");
}

#[test]
fn recommendation_feedback_classifier_is_conservative() {
    assert_eq!(
        classify_recommendation_feedback("텔레그램 말고 노션으로 바꿔줘"),
        "refine"
    );
    assert_eq!(
        classify_recommendation_feedback("이건 필요없고 중복이라 싫어"),
        "negative"
    );
    assert_eq!(
        classify_recommendation_feedback("좋아요. 이 방향이 맞아요"),
        "positive"
    );
}

#[test]
fn parse_u32_param_uses_bounds_and_string_values() {
    assert_eq!(parse_u32_param(Some(&json!("12")), 5, 1, 20), 12);
    assert_eq!(parse_u32_param(Some(&json!(99)), 5, 1, 20), 20);
    assert_eq!(parse_u32_param(Some(&json!(0)), 5, 1, 20), 1);
    assert_eq!(parse_u32_param(None, 5, 1, 20), 5);
}

#[test]
fn feedback_tie_suppresses_response_reuse() {
    assert!(feedback_allows_response_reuse(1, 0));
    assert!(feedback_allows_response_reuse(2, 1));
    assert!(!feedback_allows_response_reuse(1, 1));
    assert!(!feedback_allows_response_reuse(0, 1));
}

#[test]
fn request_prefers_fresh_data_is_precise() {
    assert!(request_prefers_fresh_data(
        "지금 최근 이메일 5개 새로고침",
        "gmail_list"
    ));
    assert!(!request_prefers_fresh_data(
        "새로운 이메일 5개 보여줘",
        "gmail_list"
    ));
    assert!(!request_prefers_fresh_data(
        "최근 이메일 5개 보여줘",
        "build_workflow"
    ));
}

#[tokio::test]
#[serial]
async fn approve_recommendation_blocks_auto_items_without_readiness() {
    crate::db::init().ok();
    crate::db::clear_recommendations_for_tests();
    crate::db::clear_recommendation_review_events_for_tests();

    let proposal = crate::recommendation::AutomationProposal {
        title: "Weak Approval Candidate".to_string(),
        summary: "Detected 3 repeats across 1 distinct day(s).".to_string(),
        trigger: "weak-approval".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.74,
        n8n_prompt: "Create a workflow for weak approval candidate.".to_string(),
        evidence: vec![
            "Frequency: Found 3 occurrences".to_string(),
            "Span: 1 distinct day(s)".to_string(),
            "policy.reason=support_work_signals=vscode,terminal".to_string(),
        ],
        pattern_id: Some("weak-approval-pattern".to_string()),
        category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
        business_score: 0.61,
    };
    crate::db::insert_recommendation(&proposal).expect("insert recommendation");
    let rec_id = crate::db::get_recommendations_with_filter(Some("pending"))
        .expect("list recs")
        .into_iter()
        .find(|rec| rec.title == "Weak Approval Candidate")
        .map(|rec| rec.id)
        .expect("recommendation id");

    let state = AppState {
        llm_client: None,
        current_goal: Arc::new(Mutex::new(None)),
    };

    let result = approve_recommendation(
        State(state),
        axum::extract::Path(rec_id),
        Some(Json(RecommendationReviewActionRequest {
            actor: Some("test_dashboard".to_string()),
            note: Some("manual approve attempt".to_string()),
        })),
    )
    .await;

    let (status, Json(body)) = result.expect_err("approval should be blocked");
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["details"]
        .as_str()
        .unwrap_or_default()
        .contains("needs stronger business evidence"));

    let events = crate::db::list_recommendation_review_events(10).expect("review events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].action, "approve");
    assert!(!events[0].ok);
    assert_eq!(events[0].actor.as_deref(), Some("test_dashboard"));
    assert_eq!(events[0].note.as_deref(), Some("manual approve attempt"));
    assert_eq!(events[0].status_after.as_deref(), Some("pending"));
}

#[tokio::test]
#[serial]
async fn later_recommendation_rejects_approved_items_and_logs_event() {
    crate::db::init().ok();
    crate::db::clear_recommendations_for_tests();
    crate::db::clear_recommendation_review_events_for_tests();

    let proposal = crate::recommendation::AutomationProposal {
        title: "Approved Later Candidate".to_string(),
        summary: "Detected 5 repeats across 4 distinct day(s).".to_string(),
        trigger: "approved-later".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.91,
        n8n_prompt: "Create a workflow.".to_string(),
        evidence: vec![
            "Frequency: Found 5 occurrences".to_string(),
            "Span: 4 distinct day(s)".to_string(),
            "policy.reason=strong_work_signals=slack,calendar".to_string(),
            "policy.reason=pattern.context_score=0.86".to_string(),
            "policy.reason=pattern.work_context=strong".to_string(),
        ],
        pattern_id: Some("approved-later".to_string()),
        category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
        business_score: 0.83,
    };
    crate::db::insert_recommendation(&proposal).expect("insert recommendation");
    let rec_id = crate::db::get_recommendations_with_filter(Some("pending"))
        .expect("list recommendations")
        .into_iter()
        .find(|rec| rec.title == "Approved Later Candidate")
        .map(|rec| rec.id)
        .expect("recommendation id");
    crate::db::update_recommendation_review_status(rec_id, "approved")
        .expect("approve recommendation");

    let status = later_recommendation(
        axum::extract::Path(rec_id),
        Some(Json(RecommendationReviewActionRequest {
            actor: Some("test_workflows".to_string()),
            note: Some("should not snooze approved item".to_string()),
        })),
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT);
    let rec = crate::db::get_recommendation(rec_id)
        .expect("get recommendation")
        .expect("recommendation");
    assert_eq!(rec.status, "approved");
    assert!(rec.snoozed_until.is_none());

    let events = crate::db::list_recommendation_review_events(10).expect("review events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].action, "later");
    assert!(!events[0].ok);
    assert_eq!(events[0].status_after.as_deref(), Some("approved"));
    assert_eq!(events[0].actor.as_deref(), Some("test_workflows"));
}

#[tokio::test]
#[serial]
async fn recommendation_metrics_hide_non_ready_auto_pending_items() {
    crate::db::init().ok();
    crate::db::clear_recommendations_for_tests();

    crate::db::insert_recommendation(&crate::recommendation::AutomationProposal {
        title: "Ready Metric Candidate".to_string(),
        summary: "Detected 6 repeats across 4 distinct day(s).".to_string(),
        trigger: "ready-metric".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.88,
        n8n_prompt: "Create a workflow.".to_string(),
        evidence: vec![
            "Frequency: Found 6 occurrences".to_string(),
            "Span: 4 distinct day(s)".to_string(),
            "policy.reason=strong_work_signals=slack,calendar".to_string(),
            "policy.reason=pattern.context_score=0.82".to_string(),
            "policy.reason=pattern.work_context=strong".to_string(),
        ],
        pattern_id: Some("ready-metric".to_string()),
        category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
        business_score: 0.82,
    })
    .expect("insert ready recommendation");
    crate::db::insert_recommendation(&crate::recommendation::AutomationProposal {
        title: "Weak Metric Candidate".to_string(),
        summary: "Detected 3 repeats across 1 distinct day(s).".to_string(),
        trigger: "weak-metric".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.74,
        n8n_prompt: "Create a workflow.".to_string(),
        evidence: vec![
            "Frequency: Found 3 occurrences".to_string(),
            "Span: 1 distinct day(s)".to_string(),
            "policy.reason=support_work_signals=vscode,terminal".to_string(),
        ],
        pattern_id: Some("weak-metric".to_string()),
        category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
        business_score: 0.61,
    })
    .expect("insert weak recommendation");

    let Json(metrics) = get_recommendation_metrics().await;

    assert_eq!(metrics.pending, 1);
}
