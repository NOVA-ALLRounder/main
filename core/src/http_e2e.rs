use anyhow::{Context, Result};
use axum::{routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

use crate::api_server::{self, AppState};
use crate::recommendation::AutomationProposal;

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

struct EnvVarGuard {
    entries: Option<Vec<(String, Option<String>)>>,
}

impl EnvVarGuard {
    fn capture(keys: &[&str]) -> Self {
        let entries = keys
            .iter()
            .map(|key| (key.to_string(), std::env::var(key).ok()))
            .collect();
        Self {
            entries: Some(entries),
        }
    }

    fn restore_in_place(&mut self) {
        if let Some(entries) = self.entries.take() {
            for (key, value) in entries {
                match value {
                    Some(value) => std::env::set_var(&key, value),
                    None => std::env::remove_var(&key),
                }
            }
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        self.restore_in_place();
    }
}

struct DbRuntimeIsolationGuard {
    env_guard: EnvVarGuard,
}

impl DbRuntimeIsolationGuard {
    fn capture(keys: &[&str]) -> Self {
        Self {
            env_guard: EnvVarGuard::capture(keys),
        }
    }
}

impl Drop for DbRuntimeIsolationGuard {
    fn drop(&mut self) {
        crate::db::reset_connection();
        self.env_guard.restore_in_place();
        crate::db::reset_connection();
        let _ = crate::db::init();
    }
}

struct ServerHandle {
    join: Option<tokio::task::JoinHandle<()>>,
}

impl ServerHandle {
    fn new(join: tokio::task::JoinHandle<()>) -> Self {
        Self { join: Some(join) }
    }

    async fn shutdown(mut self) {
        if let Some(join) = self.join.take() {
            join.abort();
            let _ = join.await;
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        if let Some(join) = self.join.take() {
            join.abort();
        }
    }
}

fn push_step(steps: &mut Vec<HttpE2EStepResult>, name: &str, ok: bool, detail: impl Into<String>) {
    steps.push(HttpE2EStepResult {
        name: name.to_string(),
        ok,
        detail: detail.into(),
    });
}

fn seed_live_e2e_recommendation() -> Result<i64> {
    let proposal = AutomationProposal {
        title: "HTTP E2E Workflow Proposal".to_string(),
        summary: "Detected 5 repeats across 4 distinct day(s).".to_string(),
        trigger: "http-e2e".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.91,
        n8n_prompt: "Create a workflow that stores weekly meeting notes to Notion.".to_string(),
        evidence: vec![
            "Frequency: Found 5 occurrences".to_string(),
            "Span: 4 distinct day(s)".to_string(),
            "policy.reason=pattern.context_score=0.87".to_string(),
            "policy.reason=pattern.work_context=strong".to_string(),
            "policy.reason=strong_work_signals=calendar,notion".to_string(),
        ],
        pattern_id: Some("http-e2e-pattern".to_string()),
        category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
        business_score: 0.86,
    };
    crate::db::insert_recommendation(&proposal).context("failed to seed recommendation")?;
    let recommendations = crate::db::get_recommendations_with_filter(Some("pending"))
        .context("failed to list seeded recommendations")?;
    recommendations
        .into_iter()
        .find(|rec| rec.title == "HTTP E2E Workflow Proposal")
        .map(|rec| rec.id)
        .context("seeded recommendation id not found")
}

fn seed_release_readiness_fixture(
    workdir: &Path,
) -> Result<crate::release_readiness::ReleaseReadinessReport> {
    let generated_at = chrono::Utc::now().to_rfc3339();
    let launch_eval_dir = workdir.join("reports/launch_eval");
    let release_readiness_dir = workdir.join("reports/release_readiness");
    let release_readiness_history_dir = release_readiness_dir
        .join("history")
        .join(archive_slug(&generated_at));
    let launch_eval_history_dir = launch_eval_dir
        .join("history")
        .join(archive_slug(&generated_at));
    let http_e2e_dir = workdir.join("reports/http_e2e");
    let http_e2e_history_dir = http_e2e_dir
        .join("history")
        .join(archive_slug(&generated_at));

    for dir in [
        &launch_eval_dir,
        &release_readiness_dir,
        &release_readiness_history_dir,
        &launch_eval_history_dir,
        &http_e2e_dir,
        &http_e2e_history_dir,
    ] {
        fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
    }

    let fixture_http_report = HttpE2EReport {
        generated_at: generated_at.clone(),
        workdir: workdir.display().to_string(),
        report_json_path: http_e2e_dir.join("latest.json").display().to_string(),
        report_markdown_path: http_e2e_dir.join("latest.md").display().to_string(),
        archived_history_json_path: Some(
            http_e2e_history_dir
                .join("http_e2e.json")
                .display()
                .to_string(),
        ),
        archived_history_markdown_path: Some(
            http_e2e_history_dir
                .join("http_e2e.md")
                .display()
                .to_string(),
        ),
        api_base_url: "http://127.0.0.1:0".to_string(),
        runtime_db_path: workdir.join("fixture-http-e2e.db").display().to_string(),
        digest_stub_url: "http://127.0.0.1:0/".to_string(),
        ok: true,
        passed: 2,
        total: 2,
        steps: vec![
            HttpE2EStepResult {
                name: "fixture-health".to_string(),
                ok: true,
                detail: "ok".to_string(),
            },
            HttpE2EStepResult {
                name: "fixture-ops".to_string(),
                ok: true,
                detail: "ok".to_string(),
            },
        ],
    };
    let http_report_json = serde_json::to_string_pretty(&fixture_http_report)
        .context("failed to serialize fixture http e2e report")?;
    let http_report_markdown = render_markdown_report(&fixture_http_report);
    fs::write(http_e2e_dir.join("latest.json"), &http_report_json).with_context(|| {
        format!(
            "failed to write {}",
            http_e2e_dir.join("latest.json").display()
        )
    })?;
    fs::write(http_e2e_dir.join("latest.md"), &http_report_markdown).with_context(|| {
        format!(
            "failed to write {}",
            http_e2e_dir.join("latest.md").display()
        )
    })?;
    fs::write(
        http_e2e_history_dir.join("http_e2e.json"),
        &http_report_json,
    )
    .with_context(|| {
        format!(
            "failed to write {}",
            http_e2e_history_dir.join("http_e2e.json").display()
        )
    })?;
    fs::write(
        http_e2e_history_dir.join("http_e2e.md"),
        &http_report_markdown,
    )
    .with_context(|| {
        format!(
            "failed to write {}",
            http_e2e_history_dir.join("http_e2e.md").display()
        )
    })?;

    let launch_eval_report = crate::launch_eval::LaunchEvalReport {
        generated_at: generated_at.clone(),
        config_path: Some("configs/launch_eval.yaml".to_string()),
        db_path: workdir.join("fixture-launch-eval.db").display().to_string(),
        report_json_path: launch_eval_dir.join("latest.json").display().to_string(),
        report_markdown_path: launch_eval_dir.join("latest.md").display().to_string(),
        total: 3,
        passed: 3,
        failed: 0,
        results: vec![],
    };
    let launch_eval_json = serde_json::to_string_pretty(&launch_eval_report)
        .context("failed to serialize fixture launch eval report")?;
    let launch_eval_markdown = "# Fixture Launch Eval\n\n- status: ok\n- passed: 3/3\n".to_string();
    fs::write(launch_eval_dir.join("latest.json"), &launch_eval_json).with_context(|| {
        format!(
            "failed to write {}",
            launch_eval_dir.join("latest.json").display()
        )
    })?;
    fs::write(launch_eval_dir.join("latest.md"), &launch_eval_markdown).with_context(|| {
        format!(
            "failed to write {}",
            launch_eval_dir.join("latest.md").display()
        )
    })?;
    fs::write(
        launch_eval_history_dir.join("launch_eval.json"),
        &launch_eval_json,
    )
    .with_context(|| {
        format!(
            "failed to write {}",
            launch_eval_history_dir.join("launch_eval.json").display()
        )
    })?;
    fs::write(
        launch_eval_history_dir.join("launch_eval.md"),
        &launch_eval_markdown,
    )
    .with_context(|| {
        format!(
            "failed to write {}",
            launch_eval_history_dir.join("launch_eval.md").display()
        )
    })?;

    let report = crate::release_readiness::ReleaseReadinessReport {
        generated_at: generated_at.clone(),
        workdir: workdir.display().to_string(),
        config_path: "configs/launch_eval.yaml".to_string(),
        report_json_path: release_readiness_dir
            .join("latest.json")
            .display()
            .to_string(),
        report_markdown_path: release_readiness_dir
            .join("latest.md")
            .display()
            .to_string(),
        archived_history_json_path: Some(
            release_readiness_history_dir
                .join("release_readiness.json")
                .display()
                .to_string(),
        ),
        archived_history_markdown_path: Some(
            release_readiness_history_dir
                .join("release_readiness.md")
                .display()
                .to_string(),
        ),
        archived_launch_eval_json_path: Some(
            launch_eval_history_dir
                .join("launch_eval.json")
                .display()
                .to_string(),
        ),
        archived_launch_eval_markdown_path: Some(
            launch_eval_history_dir
                .join("launch_eval.md")
                .display()
                .to_string(),
        ),
        history_trend: Some(crate::release_readiness::ReleaseReadinessTrendSummary {
            compared_runs: 1,
            stable_ready_streak: 1,
            status_regressed: false,
            snapshot_delta: 0,
            launch_eval_pass_rate_delta_pct: 0.0,
            http_e2e_pass_rate_delta_pct: 0.0,
            http_e2e_regressed: false,
            blocker_delta: 0,
            advisory_delta: 0,
            warnings: vec![],
            summary: "Fixture readiness history is stable.".to_string(),
        }),
        baseline_saved: false,
        synced_nl_runs_from_launch_ops: 0,
        http_e2e: Some(fixture_http_report),
        http_e2e_load_error: None,
        candidate_snapshot: crate::launch_eval::LaunchEvalCandidateSnapshot {
            generated_at: generated_at.clone(),
            output_path: workdir
                .join("configs/launch_eval.generated.yaml")
                .display()
                .to_string(),
            provenance_filter: "real".to_string(),
            scenario_count: 7,
            candidate_ids: vec!["fixture-a".to_string(), "fixture-b".to_string()],
        },
        launch_eval: launch_eval_report,
        release_gate: crate::release_gate::ReleaseGateResult {
            ok: true,
            regressions: vec![],
            warnings: vec![],
            current: crate::release_gate::ReleaseBaseline {
                created_at: generated_at.clone(),
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
            baseline: None,
            template: "release_gate".to_string(),
        },
        status: "ready".to_string(),
        ready_for_launch: true,
        blockers: vec![],
        advisories: vec![],
    };

    let report_json = serde_json::to_string_pretty(&report)
        .context("failed to serialize fixture release readiness report")?;
    let report_markdown =
        "# Fixture Release Readiness\n\n- status: ready\n- launch_eval: 3/3\n- http_e2e: 2/2\n"
            .to_string();
    fs::write(release_readiness_dir.join("latest.json"), &report_json).with_context(|| {
        format!(
            "failed to write {}",
            release_readiness_dir.join("latest.json").display()
        )
    })?;
    fs::write(release_readiness_dir.join("latest.md"), &report_markdown).with_context(|| {
        format!(
            "failed to write {}",
            release_readiness_dir.join("latest.md").display()
        )
    })?;
    fs::write(
        release_readiness_history_dir.join("release_readiness.json"),
        &report_json,
    )
    .with_context(|| {
        format!(
            "failed to write {}",
            release_readiness_history_dir
                .join("release_readiness.json")
                .display()
        )
    })?;
    fs::write(
        release_readiness_history_dir.join("release_readiness.md"),
        &report_markdown,
    )
    .with_context(|| {
        format!(
            "failed to write {}",
            release_readiness_history_dir
                .join("release_readiness.md")
                .display()
        )
    })?;

    Ok(report)
}

async fn spawn_digest_stub() -> Result<(String, ServerHandle)> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .context("failed to bind digest stub listener")?;
    let addr = listener
        .local_addr()
        .context("failed to read digest stub addr")?;
    let app = Router::new().route(
        "/",
        post(|| async {
            Json(json!({
                "status": "ok",
                "notion_url": "https://www.notion.so/http-e2e-digest",
                "top_headlines_text": "1. 헤드라인 A\n2. 헤드라인 B"
            }))
        }),
    );
    let join = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok((format!("http://{addr}/"), ServerHandle::new(join)))
}

fn canonical_string(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn render_markdown_report(report: &HttpE2EReport) -> String {
    let mut lines = vec![
        "# Allvia HTTP E2E".to_string(),
        "".to_string(),
        format!("- generated_at: {}", report.generated_at),
        format!("- status: {}", if report.ok { "ok" } else { "failed" }),
        format!("- steps: {}/{}", report.passed, report.total),
        format!("- api_base_url: {}", report.api_base_url),
        format!("- runtime_db_path: {}", report.runtime_db_path),
        format!("- digest_stub_url: {}", report.digest_stub_url),
        "".to_string(),
        "## Steps".to_string(),
        "".to_string(),
    ];

    for step in &report.steps {
        lines.push(format!(
            "- [{}] {}: {}",
            if step.ok { "pass" } else { "fail" },
            step.name,
            step.detail
        ));
    }

    lines.join("\n")
}

fn archive_slug(generated_at: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(generated_at)
        .map(|value| value.format("%Y%m%d-%H%M%S").to_string())
        .unwrap_or_else(|_| {
            generated_at
                .chars()
                .filter(|value| value.is_ascii_digit())
                .collect::<String>()
                .chars()
                .take(14)
                .collect::<String>()
        })
}

struct HttpE2EArchivePaths {
    history_dir: PathBuf,
    report_json: PathBuf,
    report_markdown: PathBuf,
}

fn build_archive_paths(workdir: &Path, generated_at: &str) -> HttpE2EArchivePaths {
    let slug = archive_slug(generated_at);
    let history_dir = workdir.join("reports/http_e2e/history").join(slug);
    HttpE2EArchivePaths {
        report_json: history_dir.join("http_e2e.json"),
        report_markdown: history_dir.join("http_e2e.md"),
        history_dir,
    }
}

pub fn latest_http_e2e_report_path(workdir: &Path) -> PathBuf {
    workdir.join("reports/http_e2e/latest.json")
}

pub fn load_latest_http_e2e_report(workdir: &Path) -> Result<Option<HttpE2EReport>> {
    let path = latest_http_e2e_report_path(workdir);
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let report = serde_json::from_str::<HttpE2EReport>(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(report))
}

pub fn list_http_e2e_history(workdir: &Path, limit: usize) -> Result<Vec<HttpE2EHistoryEntry>> {
    let limit = limit.clamp(1, 100);
    let history_dir = workdir.join("reports/http_e2e/history");
    if !history_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = fs::read_dir(&history_dir)
        .with_context(|| format!("failed to read {}", history_dir.display()))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path().join("http_e2e.json");
            if !path.exists() {
                return None;
            }
            let payload = fs::read_to_string(&path).ok()?;
            let report = serde_json::from_str::<HttpE2EReport>(&payload).ok()?;
            Some(HttpE2EHistoryEntry {
                generated_at: report.generated_at,
                ok: report.ok,
                passed: report.passed,
                total: report.total,
                report_json_path: path.display().to_string(),
                report_markdown_path: path.with_file_name("http_e2e.md").display().to_string(),
            })
        })
        .collect::<Vec<_>>();

    entries.sort_by(|left, right| right.generated_at.cmp(&left.generated_at));
    entries.truncate(limit);
    Ok(entries)
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

    match client
        .get(format!("{api_base_url}/api/health"))
        .send()
        .await
        .context("health request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            push_step(
                &mut steps,
                "health",
                status.is_success() && body.trim() == "ok",
                format!("status={} body={}", status, body.trim()),
            );
        }
        Err(error) => push_step(&mut steps, "health", false, error.to_string()),
    }

    match client
        .get(format!("{api_base_url}/api/system/db-paths"))
        .send()
        .await
        .context("db-path request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!({}));
            let runtime_db_path = body["core_db_path"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let expected_db_path = canonical_string(&temp_db_path);
            push_step(
                &mut steps,
                "db_scope",
                status.is_success() && runtime_db_path == expected_db_path,
                format!("status={} core_db_path={}", status, runtime_db_path),
            );
        }
        Err(error) => push_step(&mut steps, "db_scope", false, error.to_string()),
    }

    let help_payload = json!({
        "message": "도움말",
        "channel": "web",
        "chat_type": "direct",
        "sender": "http-e2e"
    });

    match client
        .post(format!("{api_base_url}/api/chat"))
        .json(&help_payload)
        .send()
        .await
        .context("first help request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!({}));
            let route_kind = body["route_meta"]["route_kind"]
                .as_str()
                .unwrap_or_default();
            let command = body["command"].as_str().unwrap_or_default();
            let local_only = body["route_meta"]["local_only"].as_bool().unwrap_or(false);
            push_step(
                &mut steps,
                "chat_local_help",
                status.is_success()
                    && command == "help_local"
                    && route_kind == "local_command"
                    && local_only,
                format!(
                    "status={} command={} route_kind={} local_only={}",
                    status, command, route_kind, local_only
                ),
            );
        }
        Err(error) => push_step(&mut steps, "chat_local_help", false, error.to_string()),
    }

    match client
        .post(format!("{api_base_url}/api/chat"))
        .json(&help_payload)
        .send()
        .await
        .context("repeat help request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!({}));
            let route_kind = body["route_meta"]["route_kind"]
                .as_str()
                .unwrap_or_default();
            let request_memory_hit = body["route_meta"]["request_memory_hit"]
                .as_bool()
                .unwrap_or(false);
            push_step(
                &mut steps,
                "chat_request_memory_reuse",
                status.is_success() && route_kind == "request_memory" && request_memory_hit,
                format!(
                    "status={} route_kind={} request_memory_hit={}",
                    status, route_kind, request_memory_hit
                ),
            );
        }
        Err(error) => push_step(
            &mut steps,
            "chat_request_memory_reuse",
            false,
            error.to_string(),
        ),
    }

    let digest_payload = json!({
        "message": "AI 뉴스 5개 요약해서 노션에 정리해줘",
        "channel": "web",
        "chat_type": "direct",
        "sender": "http-e2e"
    });

    match client
        .post(format!("{api_base_url}/api/chat"))
        .json(&digest_payload)
        .send()
        .await
        .context("ai digest request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!({}));
            let route_kind = body["route_meta"]["route_kind"]
                .as_str()
                .unwrap_or_default();
            let ai_digest_used = body["route_meta"]["ai_digest_used"]
                .as_bool()
                .unwrap_or(false);
            let response_text = body["response"].as_str().unwrap_or_default();
            push_step(
                &mut steps,
                "ai_digest_auto_route",
                status.is_success()
                    && route_kind == "ai_digest_auto"
                    && ai_digest_used
                    && response_text.contains("https://www.notion.so/http-e2e-digest"),
                format!(
                    "status={} route_kind={} ai_digest_used={} response_has_notion={}",
                    status,
                    route_kind,
                    ai_digest_used,
                    response_text.contains("https://www.notion.so/http-e2e-digest")
                ),
            );
        }
        Err(error) => push_step(&mut steps, "ai_digest_auto_route", false, error.to_string()),
    }

    match client
        .post(format!(
            "{api_base_url}/api/recommendations/{seeded_recommendation_id}/later"
        ))
        .json(&json!({
            "actor": "http_e2e",
            "note": "live http smoke snooze"
        }))
        .send()
        .await
        .context("later recommendation request failed")
    {
        Ok(response) => {
            let status = response.status();
            push_step(
                &mut steps,
                "recommendation_later_action",
                status.is_success(),
                format!("status={}", status),
            );
        }
        Err(error) => push_step(
            &mut steps,
            "recommendation_later_action",
            false,
            error.to_string(),
        ),
    }

    match client
        .get(format!(
            "{api_base_url}/api/recommendations/review-events?limit=10"
        ))
        .send()
        .await
        .context("review events request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!([]));
            let first = body
                .as_array()
                .and_then(|rows| rows.first())
                .cloned()
                .unwrap_or_else(|| json!({}));
            let action = first["action"].as_str().unwrap_or_default();
            let actor = first["actor"].as_str().unwrap_or_default();
            let ok = first["ok"].as_bool().unwrap_or(false);
            push_step(
                &mut steps,
                "recommendation_review_audit",
                status.is_success() && action == "later" && actor == "http_e2e" && ok,
                format!(
                    "status={} action={} actor={} ok={}",
                    status, action, actor, ok
                ),
            );
        }
        Err(error) => push_step(
            &mut steps,
            "recommendation_review_audit",
            false,
            error.to_string(),
        ),
    }

    match client
        .get(format!("{api_base_url}/api/launch/ops?limit=20"))
        .send()
        .await
        .context("launch ops request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!({}));
            let total_requests = body["chat_metrics"]["total_requests"]
                .as_i64()
                .unwrap_or_default();
            let request_memory_hits = body["chat_metrics"]["request_memory_hits"]
                .as_i64()
                .unwrap_or_default();
            let ai_digest_auto_routes = body["chat_metrics"]["ai_digest_auto_routes"]
                .as_i64()
                .unwrap_or_default();
            let later_actions = body["recommendation_review_metrics"]["later_actions"]
                .as_i64()
                .unwrap_or_default();
            push_step(
                &mut steps,
                "launch_ops_metrics",
                status.is_success()
                    && total_requests >= 3
                    && request_memory_hits >= 1
                    && ai_digest_auto_routes >= 1
                    && later_actions >= 1,
                format!(
                    "status={} total_requests={} request_memory_hits={} ai_digest_auto_routes={} later_actions={}",
                    status, total_requests, request_memory_hits, ai_digest_auto_routes, later_actions
                ),
            );
        }
        Err(error) => push_step(&mut steps, "launch_ops_metrics", false, error.to_string()),
    }

    let runtime_db_path = canonical_string(&temp_db_path);
    let passed = steps.iter().filter(|step| step.ok).count();
    let total = steps.len();
    let report_dir = workdir.join("reports/http_e2e");
    fs::create_dir_all(&report_dir)
        .with_context(|| format!("failed to create {}", report_dir.display()))?;
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
    fs::create_dir_all(&archive_paths.history_dir)
        .with_context(|| format!("failed to create {}", archive_paths.history_dir.display()))?;
    report.archived_history_json_path = Some(archive_paths.report_json.display().to_string());
    report.archived_history_markdown_path =
        Some(archive_paths.report_markdown.display().to_string());

    let report_json =
        serde_json::to_string_pretty(&report).context("failed to serialize http e2e report")?;
    let report_markdown = render_markdown_report(&report);
    fs::write(&report_json_path, &report_json)
        .with_context(|| format!("failed to write {}", report_json_path.display()))?;
    fs::write(&report_markdown_path, &report_markdown)
        .with_context(|| format!("failed to write {}", report_markdown_path.display()))?;
    fs::write(&archive_paths.report_json, &report_json)
        .with_context(|| format!("failed to write {}", archive_paths.report_json.display()))?;
    fs::write(&archive_paths.report_markdown, &report_markdown).with_context(|| {
        format!(
            "failed to write {}",
            archive_paths.report_markdown.display()
        )
    })?;

    match client
        .get(format!("{api_base_url}/api/http-e2e/latest"))
        .query(&[("workdir", workdir.display().to_string())])
        .send()
        .await
        .context("http e2e latest request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!(null));
            let passed = body["passed"].as_u64().unwrap_or_default() as usize;
            let total = body["total"].as_u64().unwrap_or_default() as usize;
            let ok = body["ok"].as_bool().unwrap_or(false);
            push_step(
                &mut steps,
                "http_e2e_latest_endpoint",
                status.is_success() && ok && passed == report.passed && total == report.total,
                format!(
                    "status={} ok={} passed={} total={}",
                    status, ok, passed, total
                ),
            );
        }
        Err(error) => push_step(
            &mut steps,
            "http_e2e_latest_endpoint",
            false,
            error.to_string(),
        ),
    }

    match client
        .get(format!("{api_base_url}/api/http-e2e/history"))
        .query(&[
            ("workdir", workdir.display().to_string()),
            ("limit", "5".to_string()),
        ])
        .send()
        .await
        .context("http e2e history request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!([]));
            let first = body
                .as_array()
                .and_then(|rows| rows.first())
                .cloned()
                .unwrap_or_else(|| json!({}));
            let passed = first["passed"].as_u64().unwrap_or_default() as usize;
            let total = first["total"].as_u64().unwrap_or_default() as usize;
            let ok = first["ok"].as_bool().unwrap_or(false);
            push_step(
                &mut steps,
                "http_e2e_history_endpoint",
                status.is_success() && ok && passed == report.passed && total == report.total,
                format!(
                    "status={} ok={} passed={} total={}",
                    status, ok, passed, total
                ),
            );
        }
        Err(error) => push_step(
            &mut steps,
            "http_e2e_history_endpoint",
            false,
            error.to_string(),
        ),
    }

    let readiness_fixture_root = temp_dir.path().join("release-readiness-fixture");
    let readiness_fixture = seed_release_readiness_fixture(&readiness_fixture_root)?;

    match client
        .get(format!("{api_base_url}/api/release/readiness"))
        .query(&[("workdir", readiness_fixture_root.display().to_string())])
        .send()
        .await
        .context("release readiness latest request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!(null));
            let ready = body["ready_for_launch"].as_bool().unwrap_or(false);
            let report_status = body["status"].as_str().unwrap_or_default();
            let passed = body["http_e2e"]["passed"].as_u64().unwrap_or_default() as usize;
            push_step(
                &mut steps,
                "release_readiness_latest_endpoint",
                status.is_success()
                    && ready
                    && report_status == "ready"
                    && passed
                        == readiness_fixture
                            .http_e2e
                            .as_ref()
                            .map(|value| value.passed)
                            .unwrap_or_default(),
                format!(
                    "status={} ready_for_launch={} report_status={} http_e2e_passed={}",
                    status, ready, report_status, passed
                ),
            );
        }
        Err(error) => push_step(
            &mut steps,
            "release_readiness_latest_endpoint",
            false,
            error.to_string(),
        ),
    }

    match client
        .get(format!("{api_base_url}/api/release/readiness/history"))
        .query(&[
            ("workdir", readiness_fixture_root.display().to_string()),
            ("limit", "5".to_string()),
        ])
        .send()
        .await
        .context("release readiness history request failed")
    {
        Ok(response) => {
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!([]));
            let first = body
                .as_array()
                .and_then(|rows| rows.first())
                .cloned()
                .unwrap_or_else(|| json!({}));
            let launch_eval_passed =
                first["launch_eval_passed"].as_u64().unwrap_or_default() as usize;
            let http_e2e_passed = first["http_e2e_passed"].as_u64().unwrap_or_default() as usize;
            let ready = first["ready_for_launch"].as_bool().unwrap_or(false);
            push_step(
                &mut steps,
                "release_readiness_history_endpoint",
                status.is_success()
                    && ready
                    && launch_eval_passed == readiness_fixture.launch_eval.passed
                    && http_e2e_passed
                        == readiness_fixture
                            .http_e2e
                            .as_ref()
                            .map(|value| value.passed)
                            .unwrap_or_default(),
                format!(
                    "status={} ready_for_launch={} launch_eval_passed={} http_e2e_passed={}",
                    status, ready, launch_eval_passed, http_e2e_passed
                ),
            );
        }
        Err(error) => push_step(
            &mut steps,
            "release_readiness_history_endpoint",
            false,
            error.to_string(),
        ),
    }

    report.total = steps.len();
    report.passed = steps.iter().filter(|step| step.ok).count();
    report.ok = report.passed == report.total;
    report.steps = steps.clone();
    let report_json =
        serde_json::to_string_pretty(&report).context("failed to re-serialize http e2e report")?;
    let report_markdown = render_markdown_report(&report);
    fs::write(&report_json_path, &report_json)
        .with_context(|| format!("failed to refresh {}", report_json_path.display()))?;
    fs::write(&report_markdown_path, &report_markdown)
        .with_context(|| format!("failed to refresh {}", report_markdown_path.display()))?;
    fs::write(&archive_paths.report_json, &report_json)
        .with_context(|| format!("failed to refresh {}", archive_paths.report_json.display()))?;
    fs::write(&archive_paths.report_markdown, &report_markdown).with_context(|| {
        format!(
            "failed to refresh {}",
            archive_paths.report_markdown.display()
        )
    })?;

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
mod tests {
    use super::*;
    use serial_test::serial;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    #[serial]
    async fn http_e2e_smoke_passes_with_temp_workdir() {
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
        let older = build_archive_paths(&workdir, "2026-03-08T01:00:00Z");
        let newer = build_archive_paths(&workdir, "2026-03-08T02:00:00Z");
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
}
