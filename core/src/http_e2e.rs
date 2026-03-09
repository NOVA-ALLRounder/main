mod steps;
mod support;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

use crate::api_server::{self, AppState};
use steps::{
    check_ai_digest_auto_route, check_chat_local_help, check_chat_request_memory_reuse,
    check_db_scope, check_health, check_http_e2e_history_endpoint, check_http_e2e_latest_endpoint,
    check_launch_ops_metrics, check_recommendation_later_action, check_recommendation_review_audit,
    check_release_readiness_history_endpoint, check_release_readiness_latest_endpoint,
};
use support::{
    build_archive_paths, canonical_string, seed_live_e2e_recommendation,
    seed_release_readiness_fixture, spawn_digest_stub, write_http_e2e_report,
    DbRuntimeIsolationGuard, ServerHandle,
};
pub use support::{
    latest_http_e2e_report_path, list_http_e2e_history, load_latest_http_e2e_report,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpE2EStepResult {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpE2EReport {
    pub generated_at: String,
    pub workdir: String,
    pub report_json_path: String,
    pub report_markdown_path: String,
    #[serde(default)]
    pub archived_history_json_path: Option<String>,
    #[serde(default)]
    pub archived_history_markdown_path: Option<String>,
    pub api_base_url: String,
    pub runtime_db_path: String,
    pub digest_stub_url: String,
    pub ok: bool,
    pub passed: usize,
    pub total: usize,
    pub steps: Vec<HttpE2EStepResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpE2EHistoryEntry {
    pub generated_at: String,
    pub ok: bool,
    pub passed: usize,
    pub total: usize,
    pub report_json_path: String,
    pub report_markdown_path: String,
}

async fn run_http_e2e_inner(workdir: &Path) -> Result<HttpE2EReport> {
    let temp_dir = TempDir::new().context("failed to create http e2e temp dir")?;
    let temp_db_path = temp_dir.path().join("http_e2e.db");
    let temp_db_string = temp_db_path.to_string_lossy().to_string();

    let _runtime_guard = DbRuntimeIsolationGuard::capture(&[
        "STEER_DB_PATH",
        "STEER_API_KEY",
        "STEER_API_ALLOW_NO_KEY",
        "STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL",
        "ALLVIA_AI_DIGEST_AUTO_ROUTE_CHANNELS",
        "ALLVIA_API_ALLOW_WORKDIR_OVERRIDE",
        "ALLVIA_API_ALLOW_PATH_OVERRIDE",
    ]);

    std::env::set_var("STEER_DB_PATH", &temp_db_string);
    std::env::set_var("STEER_API_ALLOW_NO_KEY", "1");
    std::env::remove_var("STEER_API_KEY");
    std::env::remove_var("ALLVIA_AI_DIGEST_AUTO_ROUTE_CHANNELS");
    std::env::set_var("ALLVIA_API_ALLOW_WORKDIR_OVERRIDE", "1");
    std::env::set_var("ALLVIA_API_ALLOW_PATH_OVERRIDE", "1");

    crate::db::reset_connection();
    crate::db::init().context("failed to init temp http e2e db")?;

    let seeded_recommendation_id = seed_live_e2e_recommendation()?;

    let (digest_stub_url, digest_stub_handle) = spawn_digest_stub().await?;
    std::env::set_var("STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL", &digest_stub_url);

    let state = AppState {
        llm_client: None,
        current_goal: Arc::new(Mutex::new(None)),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .context("failed to bind live http e2e listener")?;
    let addr = listener
        .local_addr()
        .context("failed to read live http e2e listener addr")?;
    let api_base_url = format!("http://{addr}");
    let app = api_server::build_api_router(state);
    let server_handle = ServerHandle::new(tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    }));

    let client = reqwest::Client::builder()
        .build()
        .context("failed to build http e2e client")?;
    let mut steps = Vec::new();

    let expected_db_path = canonical_string(&temp_db_path);
    check_health(&client, &api_base_url, &mut steps).await;
    check_db_scope(&client, &api_base_url, &expected_db_path, &mut steps).await;
    check_chat_local_help(&client, &api_base_url, &mut steps).await;
    check_chat_request_memory_reuse(&client, &api_base_url, &mut steps).await;
    check_ai_digest_auto_route(&client, &api_base_url, &mut steps).await;
    check_recommendation_later_action(&client, &api_base_url, seeded_recommendation_id, &mut steps)
        .await;
    check_recommendation_review_audit(&client, &api_base_url, &mut steps).await;
    check_launch_ops_metrics(&client, &api_base_url, &mut steps).await;

    let runtime_db_path = canonical_string(&temp_db_path);
    let passed = steps.iter().filter(|step| step.ok).count();
    let total = steps.len();
    let report_dir = workdir.join("reports/http_e2e");
    let report_json_path = report_dir.join("latest.json");
    let report_markdown_path = report_dir.join("latest.md");

    let mut report = HttpE2EReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        workdir: workdir.display().to_string(),
        report_json_path: report_json_path.display().to_string(),
        report_markdown_path: report_markdown_path.display().to_string(),
        archived_history_json_path: None,
        archived_history_markdown_path: None,
        api_base_url: api_base_url.clone(),
        runtime_db_path,
        digest_stub_url: digest_stub_url.clone(),
        ok: passed == total,
        passed,
        total,
        steps: steps.clone(),
    };

    let archive_paths = build_archive_paths(workdir, &report.generated_at);
    report.archived_history_json_path = Some(archive_paths.report_json.display().to_string());
    report.archived_history_markdown_path =
        Some(archive_paths.report_markdown.display().to_string());
    write_http_e2e_report(
        &report,
        &report_json_path,
        &report_markdown_path,
        &archive_paths,
    )?;

    check_http_e2e_latest_endpoint(&client, &api_base_url, workdir, &report, &mut steps).await;
    check_http_e2e_history_endpoint(&client, &api_base_url, workdir, &report, &mut steps).await;

    let readiness_fixture_root = temp_dir.path().join("release-readiness-fixture");
    let readiness_fixture = seed_release_readiness_fixture(&readiness_fixture_root)?;
    check_release_readiness_latest_endpoint(
        &client,
        &api_base_url,
        &readiness_fixture_root,
        &readiness_fixture,
        &mut steps,
    )
    .await;
    check_release_readiness_history_endpoint(
        &client,
        &api_base_url,
        &readiness_fixture_root,
        &readiness_fixture,
        &mut steps,
    )
    .await;

    report.total = steps.len();
    report.passed = steps.iter().filter(|step| step.ok).count();
    report.ok = report.passed == report.total;
    report.steps = steps.clone();
    write_http_e2e_report(
        &report,
        &report_json_path,
        &report_markdown_path,
        &archive_paths,
    )?;

    server_handle.shutdown().await;
    digest_stub_handle.shutdown().await;

    drop(temp_dir);
    Ok(report)
}

pub async fn run_http_e2e(workdir: &Path) -> Result<HttpE2EReport> {
    crate::load_env_with_fallback();
    run_http_e2e_inner(workdir).await
}

#[cfg(test)]
mod tests;
