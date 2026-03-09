use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn test_report(generated_at: &str, status: &str, ready_for_launch: bool) -> ReleaseReadinessReport {
    ReleaseReadinessReport {
        generated_at: generated_at.to_string(),
        workdir: "/tmp/allvia".to_string(),
        config_path: "configs/launch_eval.yaml".to_string(),
        report_json_path: "reports/release_readiness/latest.json".to_string(),
        report_markdown_path: "reports/release_readiness/latest.md".to_string(),
        archived_history_json_path: None,
        archived_history_markdown_path: None,
        archived_launch_eval_json_path: None,
        archived_launch_eval_markdown_path: None,
        history_trend: None,
        baseline_saved: true,
        synced_nl_runs_from_launch_ops: 7,
        http_e2e: Some(http_e2e::HttpE2EReport {
            generated_at: generated_at.to_string(),
            workdir: "/tmp/allvia".to_string(),
            report_json_path: "reports/http_e2e/latest.json".to_string(),
            report_markdown_path: "reports/http_e2e/latest.md".to_string(),
            archived_history_json_path: Some(
                "reports/http_e2e/history/20260308-000000/http_e2e.json".to_string(),
            ),
            archived_history_markdown_path: Some(
                "reports/http_e2e/history/20260308-000000/http_e2e.md".to_string(),
            ),
            api_base_url: "http://127.0.0.1:5680".to_string(),
            runtime_db_path: "/tmp/http-e2e.db".to_string(),
            digest_stub_url: "http://127.0.0.1:9999/".to_string(),
            ok: true,
            passed: 8,
            total: 8,
            steps: vec![],
        }),
        http_e2e_load_error: None,
        candidate_snapshot: launch_eval::LaunchEvalCandidateSnapshot {
            generated_at: generated_at.to_string(),
            output_path: "configs/launch_eval.generated.yaml".to_string(),
            provenance_filter: "real".to_string(),
            scenario_count: 3,
            candidate_ids: vec!["one".to_string()],
        },
        launch_eval: launch_eval::LaunchEvalReport {
            generated_at: generated_at.to_string(),
            config_path: Some("configs/launch_eval.yaml".to_string()),
            db_path: "/tmp/eval.db".to_string(),
            report_json_path: "reports/launch_eval/latest.json".to_string(),
            report_markdown_path: "reports/launch_eval/latest.md".to_string(),
            total: 2,
            passed: 2,
            failed: 0,
            results: vec![launch_eval::LaunchEvalCaseResult {
                id: "help-local".to_string(),
                kind: "chat".to_string(),
                description: None,
                passed: true,
                errors: vec![],
                notes: vec![],
                command: Some("help_local".to_string()),
                response_preview: None,
                readiness: None,
                admission: None,
            }],
        },
        release_gate: release_gate::ReleaseGateResult {
            ok: true,
            regressions: vec![],
            warnings: vec!["thin candidate snapshot".to_string()],
            baseline: None,
            current: release_gate::ReleaseBaseline {
                created_at: generated_at.to_string(),
                consistency: None,
                semantic: None,
                performance: None,
                quality: None,
                launch_ops: None,
                nl_run_metrics: None,
                exec_approval_metrics: None,
                recommendation_metrics: None,
                recommendation_review_metrics: None,
                launch_eval: None,
                launch_eval_candidate_snapshot: None,
                launch_eval_candidate_snapshot_refresh_error: None,
            },
            template: "release_gate".to_string(),
        },
        status: status.to_string(),
        ready_for_launch,
        blockers: if ready_for_launch {
            vec![]
        } else {
            vec!["Need more real launch-eval candidates (3 < 5)".to_string()]
        },
        advisories: vec!["thin candidate snapshot".to_string()],
    }
}

#[test]
fn render_markdown_report_includes_gate_and_eval_summary() {
    let report = test_report("2026-03-08T00:00:00Z", "needs_data", false);

    let markdown = super::support::render_markdown_report(&report);
    assert!(markdown.contains("Allvia Release Readiness"));
    assert!(markdown.contains("candidate_snapshot: 3 scenarios"));
    assert!(markdown.contains("synced_nl_runs_from_launch_ops: 7"));
    assert!(markdown.contains("http_e2e: 8/8 passed"));
    assert!(markdown.contains("readiness_status: needs_data"));
    assert!(markdown.contains("help-local: pass"));
}

#[test]
fn build_history_trend_summary_detects_regression() {
    let mut report = test_report("2026-03-08T06:10:00Z", "warning", false);
    report.candidate_snapshot.scenario_count = 4;
    report.launch_eval.total = 10;
    report.launch_eval.passed = 8;
    report.blockers = vec!["blocked".to_string()];
    report.advisories = vec!["warn-a".to_string(), "warn-b".to_string()];

    let history = vec![ReleaseReadinessHistoryEntry {
        generated_at: "2026-03-08T05:00:00Z".to_string(),
        status: "ready".to_string(),
        ready_for_launch: true,
        candidate_snapshot_count: 6,
        launch_eval_passed: 10,
        launch_eval_total: 10,
        http_e2e_ok: Some(true),
        http_e2e_passed: Some(8),
        http_e2e_total: Some(8),
        blocker_count: 0,
        advisory_count: 0,
        report_json_path: "older.json".to_string(),
        report_markdown_path: "older.md".to_string(),
    }];

    report.http_e2e.as_mut().expect("http e2e").ok = false;
    report.http_e2e.as_mut().expect("http e2e").passed = 6;
    report.http_e2e.as_mut().expect("http e2e").total = 8;
    let summary = super::support::build_history_trend_summary(&report, &history);
    assert!(summary.status_regressed);
    assert_eq!(summary.snapshot_delta, -2);
    assert!(summary.launch_eval_pass_rate_delta_pct < 0.0);
    assert!(summary.http_e2e_regressed);
    assert!(summary.http_e2e_pass_rate_delta_pct < 0.0);
    assert!(summary
        .warnings
        .iter()
        .any(|item| item.contains("status_regressed")));
    assert!(summary
        .warnings
        .iter()
        .any(|item| item.contains("http_e2e_regressed")));
    assert!(summary.summary.contains("Drift detected"));
}

#[test]
fn finalize_release_readiness_marks_needs_data_for_thin_snapshot() {
    let mut report = test_report("2026-03-08T00:00:00Z", "unknown", false);
    report.baseline_saved = false;
    report.synced_nl_runs_from_launch_ops = 0;
    report.candidate_snapshot.scenario_count = 0;
    report.candidate_snapshot.candidate_ids.clear();
    report.launch_eval.total = 8;
    report.launch_eval.passed = 8;
    report.launch_eval.results.clear();
    report.release_gate.warnings.clear();
    report.status = "unknown".to_string();
    report.blockers.clear();
    report.advisories.clear();

    let finalized = super::support::finalize_release_readiness(report);
    assert_eq!(finalized.status, "needs_data");
    assert!(!finalized.ready_for_launch);
    assert!(finalized
        .blockers
        .iter()
        .any(|item| item.contains("Need more real launch-eval candidates")));
}

#[test]
fn finalize_release_readiness_blocks_http_e2e_failures() {
    let mut report = test_report("2026-03-08T00:00:00Z", "unknown", false);
    report.candidate_snapshot.scenario_count = 20;
    report.release_gate.warnings.clear();
    report.release_gate.regressions.clear();
    report.release_gate.baseline = Some(report.release_gate.current.clone());
    report.http_e2e.as_mut().expect("http e2e").ok = false;
    report.http_e2e.as_mut().expect("http e2e").passed = 6;
    report.http_e2e.as_mut().expect("http e2e").total = 8;
    report.launch_eval.total = 44;
    report.launch_eval.passed = 44;
    report.launch_eval.failed = 0;
    report.blockers.clear();
    report.advisories.clear();

    let finalized = super::support::finalize_release_readiness(report);
    assert_eq!(finalized.status, "blocked");
    assert!(!finalized.ready_for_launch);
    assert!(finalized
        .blockers
        .iter()
        .any(|item| item.contains("HTTP E2E has failing steps")));
}

#[test]
fn list_release_readiness_history_sorts_newest_first() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("duration")
        .as_nanos();
    let workdir = std::env::temp_dir().join(format!("allvia-readiness-history-{}", unique));
    let older = super::support::build_archive_paths(&workdir, "2026-03-08T03:16:14Z");
    let newer = super::support::build_archive_paths(&workdir, "2026-03-08T06:10:00Z");
    fs::create_dir_all(&older.release_readiness_dir).expect("older dir");
    fs::create_dir_all(&newer.release_readiness_dir).expect("newer dir");
    fs::write(
        &older.release_readiness_json,
        serde_json::to_string_pretty(&test_report("2026-03-08T03:16:14Z", "warning", true))
            .expect("older json"),
    )
    .expect("write older");
    fs::write(
        &newer.release_readiness_json,
        serde_json::to_string_pretty(&test_report("2026-03-08T06:10:00Z", "ready", true))
            .expect("newer json"),
    )
    .expect("write newer");

    let history = super::support::list_release_readiness_history(&workdir, 10).expect("history");
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].status, "ready");
    assert_eq!(history[0].generated_at, "2026-03-08T06:10:00Z");
    assert_eq!(history[1].status, "warning");

    let _ = fs::remove_dir_all(workdir);
}

#[test]
fn load_latest_release_readiness_report_reads_latest_json() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("duration")
        .as_nanos();
    let workdir = std::env::temp_dir().join(format!("allvia-readiness-latest-{}", unique));
    let report_dir = workdir.join("reports/release_readiness");
    fs::create_dir_all(&report_dir).expect("report dir");
    let report = test_report("2026-03-08T07:30:00Z", "ready", true);
    fs::write(
        report_dir.join("latest.json"),
        serde_json::to_string_pretty(&report).expect("report json"),
    )
    .expect("write latest report");

    let loaded = super::support::load_latest_release_readiness_report(&workdir)
        .expect("load latest report")
        .expect("latest report exists");
    assert_eq!(loaded.generated_at, "2026-03-08T07:30:00Z");
    assert_eq!(loaded.status, "ready");

    let _ = fs::remove_dir_all(workdir);
}
