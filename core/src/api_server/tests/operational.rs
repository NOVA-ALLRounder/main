use super::*;

#[tokio::test]
#[serial]
async fn get_launch_ops_handler_reports_recent_metrics() {
    crate::db::init().ok();
    reset_memory_tables();

    let state = AppState {
        llm_client: None,
        current_goal: Arc::new(Mutex::new(None)),
    };
    let help_request = ChatRequest {
        message: "도움말".to_string(),
        channel: Some("web".to_string()),
        chat_type: Some("direct".to_string()),
        sender: Some("launch-ops-metrics".to_string()),
        mentioned: None,
    };
    let digest_request = ChatRequest {
        message: "AI 뉴스 5개 요약해서 노션에 정리해줘".to_string(),
        channel: Some("web".to_string()),
        chat_type: Some("direct".to_string()),
        sender: Some("launch-ops-metrics".to_string()),
        mentioned: None,
    };

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let addr = listener.local_addr().expect("listener addr");
    let app = axum::Router::new().route(
        "/",
        axum::routing::post(|| async {
            axum::Json(json!({
                "status": "ok",
                "notion_url": "https://www.notion.so/test-launch-ops",
                "top_headlines_text": "1. 헤드라인 A"
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

    let _ = handle_chat(State(state.clone()), Json(help_request.clone())).await;
    let _ = handle_chat(State(state.clone()), Json(help_request)).await;
    let _ = handle_chat(State(state), Json(digest_request)).await;

    let Json(payload) = get_launch_ops_handler(Query(LaunchOpsQuery { limit: Some(50) })).await;

    assert_eq!(payload.chat_metrics.total_requests, 3);
    assert_eq!(payload.chat_metrics.request_memory_hits, 1);
    assert_eq!(payload.chat_metrics.ai_digest_auto_routes, 1);
    assert_eq!(payload.recommendation_metrics.pending >= 0, true);
    assert_eq!(
        payload.recommendation_review_metrics.total_events >= 0,
        true
    );
    assert_eq!(payload.recent_events[0].route_kind, "ai_digest_auto");

    std::env::remove_var("STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL");
}

#[tokio::test]
#[serial]
async fn http_e2e_handlers_run_and_load_latest_report() {
    let workdir = tempfile::tempdir().expect("temp workdir");
    let workdir_str = workdir.path().to_string_lossy().to_string();
    std::env::set_var("ALLVIA_API_ALLOW_WORKDIR_OVERRIDE", "1");

    let Json(run_report) = run_http_e2e_handler(Json(HttpE2ERequest {
        workdir: Some(workdir_str.clone()),
    }))
    .await
    .expect("run http e2e");
    assert!(run_report.ok);
    assert_eq!(run_report.passed, run_report.total);

    let Json(latest) = get_latest_http_e2e_handler(Query(HttpE2ERequest {
        workdir: Some(workdir_str),
    }))
    .await
    .expect("latest http e2e");
    let latest = latest.expect("http e2e report");
    assert_eq!(latest.passed, run_report.passed);
    assert_eq!(latest.total, run_report.total);

    std::env::remove_var("ALLVIA_API_ALLOW_WORKDIR_OVERRIDE");
}

#[tokio::test]
#[serial]
async fn run_release_readiness_handler_rejects_client_baseline_override_by_default() {
    std::env::remove_var("ALLVIA_API_ALLOW_PATH_OVERRIDE");
    let result = run_release_readiness_handler(Json(ReleaseReadinessRequest {
        workdir: None,
        config_path: None,
        candidate_limit: None,
        snapshot_output_path: None,
        save_baseline: Some(true),
    }))
    .await;
    assert!(matches!(result, Err(StatusCode::BAD_REQUEST)));
}

#[tokio::test]
#[serial]
async fn set_release_baseline_handler_rejects_override_payload_by_default() {
    std::env::remove_var("ALLVIA_API_ALLOW_PATH_OVERRIDE");
    let result = set_release_baseline_handler(Json(release_gate::ReleaseBaselineRequest {
        workdir: Some("/tmp/override".to_string()),
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
        refresh_launch_eval_candidates: Some(false),
        launch_eval_candidate_limit: Some(1),
        launch_eval_snapshot_output_path: Some("/tmp/override.json".to_string()),
    }))
    .await;
    assert!(matches!(result, Err(StatusCode::BAD_REQUEST)));
}

#[tokio::test]
#[serial]
async fn run_release_readiness_handler_rejects_client_path_overrides_by_default() {
    std::env::remove_var("ALLVIA_API_ALLOW_PATH_OVERRIDE");
    let result = run_release_readiness_handler(Json(ReleaseReadinessRequest {
        workdir: None,
        config_path: Some("/tmp/custom-launch-eval.yaml".to_string()),
        candidate_limit: None,
        snapshot_output_path: Some("/tmp/custom-snapshot.yaml".to_string()),
        save_baseline: Some(false),
    }))
    .await;
    assert!(matches!(result, Err(StatusCode::BAD_REQUEST)));
}

#[tokio::test]
#[serial]
async fn launch_eval_snapshot_handlers_reject_output_path_override_by_default() {
    std::env::remove_var("ALLVIA_API_ALLOW_PATH_OVERRIDE");

    let info_result =
        get_launch_eval_candidate_snapshot_info_handler(Query(LaunchEvalSnapshotInfoQuery {
            output_path: Some("/tmp/custom-launch-eval.generated.yaml".to_string()),
            workdir: None,
            provenance: Some("real".to_string()),
        }))
        .await;
    assert!(matches!(info_result, Err(StatusCode::BAD_REQUEST)));

    let write_result =
        write_launch_eval_candidate_snapshot_handler(Json(LaunchEvalSnapshotRequest {
            limit: Some(5),
            output_path: Some("/tmp/custom-launch-eval.generated.yaml".to_string()),
            workdir: None,
            provenance: Some("real".to_string()),
        }))
        .await;
    assert!(matches!(write_result, Err(StatusCode::BAD_REQUEST)));
}

#[test]
#[serial]
fn resolve_operational_workdir_ignores_client_override_by_default() {
    std::env::remove_var("ALLVIA_API_ALLOW_WORKDIR_OVERRIDE");
    let resolved = resolve_operational_workdir(Some("/tmp/should-not-be-used"));
    assert_eq!(resolved, default_server_workdir());
}

#[test]
#[serial]
fn resolve_operational_workdir_allows_override_with_env_flag() {
    std::env::set_var("ALLVIA_API_ALLOW_WORKDIR_OVERRIDE", "1");
    let resolved = resolve_operational_workdir(Some("/tmp/allvia-override"));
    assert_eq!(resolved, PathBuf::from("/tmp/allvia-override"));
    std::env::remove_var("ALLVIA_API_ALLOW_WORKDIR_OVERRIDE");
}

#[test]
#[serial]
fn resolve_operational_file_override_requires_explicit_env_flag() {
    std::env::remove_var("ALLVIA_API_ALLOW_PATH_OVERRIDE");
    std::env::remove_var("ALLVIA_API_ALLOW_WORKDIR_OVERRIDE");
    assert!(resolve_operational_file_override(Some("/tmp/blocked")).is_none());

    std::env::set_var("ALLVIA_API_ALLOW_PATH_OVERRIDE", "1");
    assert_eq!(
        resolve_operational_file_override(Some("/tmp/allowed")),
        Some(PathBuf::from("/tmp/allowed"))
    );
    std::env::remove_var("ALLVIA_API_ALLOW_PATH_OVERRIDE");
}

#[test]
fn default_release_baseline_request_uses_fixed_server_defaults() {
    let request = default_release_baseline_request();
    assert_eq!(request.refresh_launch_eval_candidates, Some(true));
    assert_eq!(request.launch_eval_candidate_limit, Some(20));
    assert!(request.workdir.is_none());
    assert!(request.launch_eval_config_path.is_none());
    assert!(request.launch_eval_snapshot_output_path.is_none());
}
