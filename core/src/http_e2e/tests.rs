use super::*;
use serial_test::serial;
use std::fs;
use std::io::ErrorKind;
use std::time::{SystemTime, UNIX_EPOCH};

async fn loopback_bind_supported() -> bool {
    match tokio::net::TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => {
            drop(listener);
            true
        }
        Err(error) if error.kind() == ErrorKind::PermissionDenied => {
            eprintln!(
                "skipping http_e2e test: loopback listener bind is not permitted in this environment"
            );
            false
        }
        Err(error) => panic!("bind test listener for http_e2e test: {error}"),
    }
}

#[tokio::test]
#[serial]
async fn http_e2e_smoke_passes_with_temp_workdir() {
    if !loopback_bind_supported().await {
        return;
    }

    let workdir = tempfile::tempdir().expect("temp workdir");
    let report = run_http_e2e(workdir.path()).await.expect("http e2e report");
    assert!(report.ok, "steps failed: {:?}", report.steps);
    assert_eq!(report.passed, report.total);
    assert!(report
        .steps
        .iter()
        .any(|step| step.name == "ai_digest_auto_route"));
    assert!(report
        .steps
        .iter()
        .any(|step| step.name == "http_e2e_latest_endpoint"));
    assert!(report
        .steps
        .iter()
        .any(|step| step.name == "release_readiness_latest_endpoint"));
    assert!(std::path::PathBuf::from(&report.report_json_path).exists());
    assert!(std::path::PathBuf::from(&report.report_markdown_path).exists());
    assert!(report.archived_history_json_path.is_some());
    assert!(report.archived_history_markdown_path.is_some());
}

#[test]
fn list_http_e2e_history_sorts_newest_first() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("duration")
        .as_nanos();
    let workdir = std::env::temp_dir().join(format!("allvia-http-e2e-history-{}", unique));
    let older = support::build_archive_paths(&workdir, "2026-03-08T01:00:00Z");
    let newer = support::build_archive_paths(&workdir, "2026-03-08T02:00:00Z");
    fs::create_dir_all(&older.history_dir).expect("older dir");
    fs::create_dir_all(&newer.history_dir).expect("newer dir");
    let older_report = HttpE2EReport {
        generated_at: "2026-03-08T01:00:00Z".to_string(),
        workdir: workdir.display().to_string(),
        report_json_path: latest_http_e2e_report_path(&workdir).display().to_string(),
        report_markdown_path: workdir
            .join("reports/http_e2e/latest.md")
            .display()
            .to_string(),
        archived_history_json_path: Some(older.report_json.display().to_string()),
        archived_history_markdown_path: Some(older.report_markdown.display().to_string()),
        api_base_url: "http://127.0.0.1:5680".to_string(),
        runtime_db_path: "/tmp/http-e2e-old.db".to_string(),
        digest_stub_url: "http://127.0.0.1:9999/".to_string(),
        ok: true,
        passed: 8,
        total: 8,
        steps: vec![],
    };
    let newer_report = HttpE2EReport {
        generated_at: "2026-03-08T02:00:00Z".to_string(),
        workdir: workdir.display().to_string(),
        report_json_path: latest_http_e2e_report_path(&workdir).display().to_string(),
        report_markdown_path: workdir
            .join("reports/http_e2e/latest.md")
            .display()
            .to_string(),
        archived_history_json_path: Some(newer.report_json.display().to_string()),
        archived_history_markdown_path: Some(newer.report_markdown.display().to_string()),
        api_base_url: "http://127.0.0.1:5680".to_string(),
        runtime_db_path: "/tmp/http-e2e-new.db".to_string(),
        digest_stub_url: "http://127.0.0.1:9999/".to_string(),
        ok: false,
        passed: 7,
        total: 8,
        steps: vec![],
    };
    fs::write(
        &older.report_json,
        serde_json::to_string_pretty(&older_report).expect("older json"),
    )
    .expect("write older");
    fs::write(
        &newer.report_json,
        serde_json::to_string_pretty(&newer_report).expect("newer json"),
    )
    .expect("write newer");

    let history = list_http_e2e_history(&workdir, 10).expect("history");
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].generated_at, "2026-03-08T02:00:00Z");
    assert!(!history[0].ok);
    assert_eq!(history[1].generated_at, "2026-03-08T01:00:00Z");

    let _ = fs::remove_dir_all(workdir);
}
