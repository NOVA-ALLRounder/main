use super::support::{
    derive_release_quality_score, normalize_release_exec_approval_metrics,
    visible_auto_work_pending_count,
};
use super::*;
use serial_test::serial;

fn release_baseline_with_ops(
    error_routes: i64,
    low_confidence_routes: i64,
    cached_response_hit_rate: f64,
    approved: i64,
    rejected: i64,
    pending: i64,
) -> ReleaseBaseline {
    ReleaseBaseline {
        created_at: "2026-03-07T00:00:00Z".to_string(),
        consistency: None,
        semantic: None,
        performance: None,
        quality: None,
        launch_ops: Some(db::LaunchOpsMetrics {
            window_size: 200,
            total_requests: 100,
            blocked_requests: 3,
            intent_memory_hits: 12,
            request_memory_hits: 20,
            execution_memory_hits: 22,
            cached_response_hit_rate,
            deterministic_routes: 18,
            llm_routes: 28,
            ai_digest_routes: 6,
            ai_digest_auto_routes: 4,
            local_routes: 12,
            freshness_bypasses: 5,
            low_confidence_routes,
            unknown_routes: 2,
            error_routes,
            last_event_at: Some("2026-03-07T00:00:00Z".to_string()),
            route_breakdown: vec![],
        }),
        nl_run_metrics: Some(db::NLRunMetrics {
            total: 40,
            completed: 24,
            manual_required: 6,
            approval_required: 5,
            blocked: 2,
            error: 3,
            success_rate: 60.0,
        }),
        exec_approval_metrics: Some(db::ExecApprovalMetrics {
            window_size: 100,
            total: 12,
            pending: 2,
            approved: 6,
            rejected: 2,
            expired_pending: 0,
            allow_once: 4,
            allow_always: 2,
            deny: 2,
            approval_rate: 75.0,
            oldest_pending_created_at: Some("2026-03-07T00:00:00Z".to_string()),
            last_created_at: Some("2026-03-07T00:00:00Z".to_string()),
            last_resolved_at: Some("2026-03-07T00:00:00Z".to_string()),
        }),
        recommendation_metrics: Some(db::RecommendationMetrics {
            total: approved + rejected + pending,
            approved,
            rejected,
            failed: 0,
            pending,
            later: 0,
            legacy_other: 0,
            last_created_at: Some("2026-03-07T00:00:00Z".to_string()),
        }),
        recommendation_review_metrics: Some(db::RecommendationReviewMetrics {
            window_size: 100,
            total_events: 12,
            approve_actions: approved,
            reject_actions: rejected,
            later_actions: 1,
            restore_actions: 1,
            feedback_positive: 4,
            feedback_refine: 1,
            feedback_negative: 1,
            failed_actions: 0,
            action_failure_rate: 0.0,
            non_positive_feedback_rate: 33.3,
            last_event_at: Some("2026-03-07T00:00:00Z".to_string()),
        }),
        launch_eval: Some(LaunchEvalSummary {
            generated_at: "2026-03-07T00:00:00Z".to_string(),
            config_path: Some("configs/launch_eval.yaml".to_string()),
            total: 8,
            passed: 8,
            failed: 0,
            failed_case_ids: vec![],
        }),
        launch_eval_candidate_snapshot: Some(launch_eval::LaunchEvalCandidateSnapshotInfo {
            output_path: "configs/launch_eval.generated.yaml".to_string(),
            exists: true,
            provenance_filter: "real".to_string(),
            scenario_count: 12,
            updated_at: Some("2026-03-07T00:00:00Z".to_string()),
            scenario_ids: vec![
                "request-memory-a".to_string(),
                "execution-memory-a".to_string(),
            ],
        }),
        launch_eval_candidate_snapshot_refresh_error: None,
    }
}

#[test]
fn release_gate_fails_on_launch_ops_regression() {
    let baseline = release_baseline_with_ops(2, 4, 48.0, 8, 2, 1);
    let current = release_baseline_with_ops(14, 18, 28.0, 8, 2, 1);

    let result = evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

    assert!(!result.ok);
    assert!(result
        .regressions
        .iter()
        .any(|item| item.contains("Launch ops error rate too high")));
    assert!(result
        .regressions
        .iter()
        .any(|item| item.contains("Launch ops low-confidence rate too high")));
    assert!(result
        .regressions
        .iter()
        .any(|item| item.contains("Launch ops cache hit rate dropped")));
}

#[test]
fn release_gate_warns_on_small_ops_sample() {
    let mut baseline = release_baseline_with_ops(1, 2, 40.0, 3, 2, 1);
    let mut current = release_baseline_with_ops(1, 2, 39.0, 3, 2, 1);
    baseline.launch_ops.as_mut().unwrap().total_requests = 10;
    current.launch_ops.as_mut().unwrap().total_requests = 8;

    let result = evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

    assert!(result.ok);
    assert!(result
        .warnings
        .iter()
        .any(|item| item.contains("Launch ops sample is small")));
}

#[test]
fn release_gate_catches_nl_run_regression_and_exec_backlog() {
    let baseline = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
    let mut current = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
    current.nl_run_metrics = Some(db::NLRunMetrics {
        total: 40,
        completed: 10,
        manual_required: 16,
        approval_required: 8,
        blocked: 4,
        error: 18,
        success_rate: 25.0,
    });
    current.exec_approval_metrics = Some(db::ExecApprovalMetrics {
        window_size: 100,
        total: 14,
        pending: 9,
        approved: 2,
        rejected: 5,
        expired_pending: 2,
        allow_once: 1,
        allow_always: 1,
        deny: 5,
        approval_rate: 28.5,
        oldest_pending_created_at: Some("2026-03-07T00:00:00Z".to_string()),
        last_created_at: Some("2026-03-07T00:00:00Z".to_string()),
        last_resolved_at: Some("2026-03-07T00:00:00Z".to_string()),
    });

    let result = evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

    assert!(!result.ok);
    assert!(result
        .regressions
        .iter()
        .any(|item| item.contains("NL run success rate too low")));
    assert!(result
        .regressions
        .iter()
        .any(|item| item.contains("NL run error rate too high")));
    assert!(result
        .warnings
        .iter()
        .any(|item| item.contains("Exec approval backlog is high")));
    assert!(result
        .warnings
        .iter()
        .any(|item| item.contains("Expired exec approvals detected")));
}

#[test]
fn release_gate_fails_on_recommendation_approval_drop() {
    let baseline = release_baseline_with_ops(1, 2, 45.0, 9, 1, 1);
    let current = release_baseline_with_ops(1, 2, 44.0, 1, 9, 10);

    let result = evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

    assert!(!result.ok);
    assert!(result
        .regressions
        .iter()
        .any(|item| item.contains("Recommendation approval rate too low")));
    assert!(result
        .regressions
        .iter()
        .any(|item| item.contains("Recommendation approval rate regressed")));
    assert!(result
        .warnings
        .iter()
        .any(|item| item.contains("Visible auto recommendation queue is high")));
}

#[test]
fn release_gate_warns_on_recommendation_review_friction_regression() {
    let baseline = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
    let mut current = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
    current.recommendation_review_metrics = Some(db::RecommendationReviewMetrics {
        window_size: 100,
        total_events: 16,
        approve_actions: 4,
        reject_actions: 3,
        later_actions: 4,
        restore_actions: 1,
        feedback_positive: 1,
        feedback_refine: 2,
        feedback_negative: 1,
        failed_actions: 4,
        action_failure_rate: 33.3,
        non_positive_feedback_rate: 75.0,
        last_event_at: Some("2026-03-07T00:00:00Z".to_string()),
    });

    let result = evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

    assert!(result.ok);
    assert!(result
        .warnings
        .iter()
        .any(|item| { item.contains("Recommendation review action-failure rate is elevated") }));
    assert!(result.warnings.iter().any(|item| {
        item.contains("Recommendation review non-positive feedback rate is elevated")
    }));
    assert!(result
        .warnings
        .iter()
        .any(|item| { item.contains("Recommendation review action-failure rate regressed") }));
    assert!(result.warnings.iter().any(|item| {
        item.contains("Recommendation review non-positive feedback rate regressed")
    }));
}

#[test]
fn stale_exec_approval_metrics_are_ignored_for_release_gate() {
    let baseline = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
    let mut current = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
    current.exec_approval_metrics = Some(normalize_release_exec_approval_metrics(
        db::ExecApprovalMetrics {
            window_size: 100,
            total: 14,
            pending: 9,
            approved: 0,
            rejected: 0,
            expired_pending: 4,
            allow_once: 0,
            allow_always: 0,
            deny: 0,
            approval_rate: 0.0,
            oldest_pending_created_at: Some("2026-02-01T00:00:00Z".to_string()),
            last_created_at: Some("2026-02-01T00:00:00Z".to_string()),
            last_resolved_at: None,
        },
    ));

    let result = evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

    assert!(result.ok);
    assert!(!result
        .warnings
        .iter()
        .any(|item| item.contains("Exec approval backlog is high")));
    assert!(!result
        .warnings
        .iter()
        .any(|item| item.contains("Expired exec approvals detected")));
}

#[test]
#[serial]
fn visible_work_pending_count_caps_release_queue_to_display_limit() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let db_path = temp_dir.path().join("release-gate-visible.db");
    let prev_db_path = std::env::var("STEER_DB_PATH").ok();
    std::env::set_var("STEER_DB_PATH", db_path.to_string_lossy().to_string());
    crate::db::reset_connection();
    crate::db::init().expect("init db");
    crate::db::clear_recommendations_for_tests();

    for idx in 0..10 {
        let proposal = crate::recommendation::AutomationProposal {
            title: format!("Ready Workflow {}", idx),
            summary: "ready".to_string(),
            trigger: format!("pattern-{}", idx),
            actions: vec!["notify".to_string()],
            confidence: 0.9,
            n8n_prompt: "Create a workflow that posts work updates to Notion.".to_string(),
            evidence: vec![
                "Frequency: Found 6 occurrences".to_string(),
                "Span: 4 distinct day(s)".to_string(),
                "policy.reason=strong_work_signals=notion,calendar".to_string(),
                "policy.reason=pattern.context_score=0.84".to_string(),
                "policy.reason=pattern.work_context=strong".to_string(),
            ],
            pattern_id: Some(format!("pattern-{}", idx)),
            category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
            business_score: 0.82,
        };
        crate::db::insert_recommendation(&proposal).expect("insert ready proposal");
    }

    let count = visible_auto_work_pending_count().expect("visible pending count");
    assert_eq!(
        count,
        crate::recommendation_policy::pending_recommendation_display_limit() as i64
    );

    crate::db::reset_connection();
    match prev_db_path {
        Some(value) => std::env::set_var("STEER_DB_PATH", value),
        None => std::env::remove_var("STEER_DB_PATH"),
    }
    crate::db::reset_connection();
}

#[test]
fn release_gate_fails_on_launch_eval_regression() {
    let baseline = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
    let mut current = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
    current.launch_eval = Some(LaunchEvalSummary {
        generated_at: "2026-03-07T01:00:00Z".to_string(),
        config_path: Some("configs/launch_eval.yaml".to_string()),
        total: 8,
        passed: 6,
        failed: 2,
        failed_case_ids: vec!["ai-digest-web-fallback".to_string()],
    });

    let result = evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

    assert!(!result.ok);
    assert!(result
        .regressions
        .iter()
        .any(|item| item.contains("Launch eval has failing scenarios")));
    assert!(result
        .regressions
        .iter()
        .any(|item| item.contains("Launch eval failures increased")));
}

#[test]
fn release_gate_warns_on_stale_launch_eval_report() {
    let baseline = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
    let mut current = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
    current.launch_eval = Some(LaunchEvalSummary {
        generated_at: "2024-03-01T00:00:00Z".to_string(),
        config_path: Some("configs/launch_eval.yaml".to_string()),
        total: 8,
        passed: 8,
        failed: 0,
        failed_case_ids: vec![],
    });

    let result = evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

    assert!(result.ok);
    assert!(result
        .warnings
        .iter()
        .any(|item| item.contains("Launch eval report is stale")));
}

#[test]
fn release_gate_warns_on_thin_launch_eval_candidate_snapshot() {
    let baseline = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
    let mut current = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
    current.launch_eval_candidate_snapshot = Some(launch_eval::LaunchEvalCandidateSnapshotInfo {
        output_path: "configs/launch_eval.generated.yaml".to_string(),
        exists: true,
        provenance_filter: "real".to_string(),
        scenario_count: 0,
        updated_at: Some(chrono::Utc::now().to_rfc3339()),
        scenario_ids: vec![],
    });

    let result = evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

    assert!(result
        .warnings
        .iter()
        .any(|item| item.contains("candidate snapshot is thin")));
    assert!(result
        .regressions
        .iter()
        .any(|item| item.contains("candidate coverage regressed")));
}

#[test]
fn release_gate_warns_on_candidate_snapshot_refresh_error() {
    let baseline = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
    let mut current = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
    current.launch_eval_candidate_snapshot_refresh_error = Some("permission denied".to_string());

    let result = evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

    assert!(result
        .warnings
        .iter()
        .any(|item| item.contains("snapshot refresh failed")));
}

#[test]
#[serial]
fn build_baseline_refreshes_launch_eval_candidate_snapshot_by_default() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let db_path = temp_dir.path().join("release-gate.db");
    let prev_db_path = std::env::var("STEER_DB_PATH").ok();
    std::env::set_var("STEER_DB_PATH", db_path.to_string_lossy().to_string());
    crate::db::init().expect("init db");
    crate::db::clear_request_memory_for_tests();
    crate::db::clear_execution_memory_for_tests();
    crate::db::clear_launch_ops_events_for_tests();

    let request_intent = serde_json::json!({
        "command": "calendar_today",
        "params": {},
        "confidence": 0.94
    });
    crate::db::upsert_request_memory_scoped(
        Some("channel_web__type_direct__sender_release"),
        "오늘 일정 보여줘",
        Some(&request_intent),
        Some("RELEASE_GATE_CACHE"),
        "ttl_response_signature",
        "api.chat.deterministic",
        0.94,
    )
    .expect("seed request memory");

    let baseline = build_baseline(ReleaseBaselineRequest {
        workdir: Some(temp_dir.path().display().to_string()),
        max_files: Some(10),
        consistency: None,
        semantic: None,
        performance: None,
        quality: None,
        perf_regression_pct: None,
        quality_drop: None,
        launch_error_rate_pct: None,
        launch_low_confidence_rate_pct: None,
        launch_cache_hit_rate_drop_pct: None,
        recommendation_approval_rate_min: None,
        launch_eval_config_path: None,
        launch_eval_report_path: None,
        refresh_launch_eval_candidates: Some(true),
        launch_eval_candidate_limit: Some(10),
        launch_eval_snapshot_output_path: Some("generated.yaml".to_string()),
    });

    assert_eq!(baseline.launch_eval_candidate_snapshot_refresh_error, None);
    let snapshot = baseline
        .launch_eval_candidate_snapshot
        .expect("snapshot info should exist");
    assert!(snapshot.exists);
    assert!(snapshot.scenario_count >= 1);
    assert!(snapshot.output_path.ends_with("generated.yaml"));
    assert!(baseline.quality.is_some());

    match prev_db_path {
        Some(value) => std::env::set_var("STEER_DB_PATH", value),
        None => std::env::remove_var("STEER_DB_PATH"),
    }
    crate::db::reset_connection();
}

#[test]
fn derive_release_quality_score_produces_structured_score() {
    let baseline = release_baseline_with_ops(0, 0, 40.0, 8, 2, 1);
    let quality = derive_release_quality_score(
        baseline.consistency.as_ref(),
        baseline.semantic.as_ref(),
        baseline.performance.as_ref(),
        baseline.launch_ops.as_ref(),
        baseline.launch_eval.as_ref(),
    );

    assert!(quality.overall >= 0.0);
    assert!(quality.breakdown.contains_key("functionality"));
    assert!(quality.breakdown.contains_key("ui_ux"));
    assert!(quality.breakdown.contains_key("code_quality"));
    assert!(quality.breakdown.contains_key("api_compatibility"));
    assert!(!quality.recommendation.is_empty());
}
