use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sysinfo::System;

use crate::{
    collector_pipeline, consistency_check, db, feedback_collector, judgment,
    performance_verification, project_scanner, quality_scorer, release_gate, runtime_verification,
    semantic_verification, tool_result_guard, visual_verification,
};

use super::{log_verification_run, AppState, API_SERVER_STARTED_AT};

#[derive(Deserialize)]
pub struct ProjectScanQuery {
    pub max_files: Option<usize>,
    pub workdir: Option<String>,
}

#[derive(Serialize)]
pub struct ProjectScanResponse {
    pub project_type: String,
    pub files: Vec<String>,
    pub key_files: std::collections::HashMap<String, String>,
}

#[derive(Serialize)]
pub struct RuntimeDbPathsResponse {
    pub core_db_path: Option<String>,
    pub collector_db_path: String,
    pub mismatch: bool,
    pub allow_mismatch: bool,
}

#[derive(Serialize)]
pub struct RuntimeInfoResponse {
    pub service: String,
    pub version: String,
    pub profile: String,
    pub pid: u32,
    pub api_port: u16,
    pub allow_no_key: bool,
    pub started_at: String,
    pub binary_path: Option<String>,
    pub current_dir: Option<String>,
}

#[derive(Serialize)]
pub struct LockMetricsResponse {
    pub acquired: u64,
    pub bypassed: u64,
    pub blocked: u64,
    pub stale_recovered: u64,
    pub rejected: u64,
}

#[derive(Deserialize)]
pub struct RuntimeVerifyRequest {
    pub workdir: Option<String>,
    pub run_backend: Option<bool>,
    pub run_frontend: Option<bool>,
    pub run_e2e: Option<bool>,
    pub run_build_checks: Option<bool>,
    pub backend_port: Option<u16>,
    pub frontend_port: Option<u16>,
    pub backend_health_path: Option<String>,
}

#[derive(Deserialize)]
pub struct QualityScoreRequest {
    pub runtime: Option<runtime_verification::RuntimeVerifyResult>,
    pub runtime_options: Option<RuntimeVerifyRequest>,
    pub code_review: Option<quality_scorer::CodeReviewInput>,
    pub goal: Option<String>,
    pub use_llm: Option<bool>,
}

#[derive(Serialize)]
pub struct QualityScoreResponse {
    pub created_at: String,
    pub score: quality_scorer::QualityScore,
}

#[derive(Deserialize)]
pub struct SemanticVerifyRequest {
    pub workdir: Option<String>,
    pub max_files: Option<usize>,
}

#[derive(Deserialize)]
pub struct PerformanceVerifyRequest {
    pub workdir: Option<String>,
    pub max_files: Option<usize>,
}

#[derive(Serialize)]
pub struct SystemStatus {
    pub cpu_usage: f32,
    pub memory_used: u64,
    pub memory_total: u64,
}

#[derive(Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct QualityMetrics {
    pub total: u32,
    pub success: u32,
    pub rate: f64,
}

pub(crate) async fn root_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "online",
        "service": "AllvIa Core API",
        "version": "v0.1.0",
        "ui_url": "http://localhost:5174",
        "docs": "/api/health"
    }))
}

pub(crate) async fn health_check() -> &'static str {
    "ok"
}

pub(crate) async fn get_system_status() -> Json<SystemStatus> {
    let mut sys = System::new_all();
    sys.refresh_cpu(); // First refresh just gathers data
    tokio::time::sleep(std::time::Duration::from_millis(200)).await; // Non-blocking wait for CPU delta
    sys.refresh_cpu(); // Second refresh calculates usage
    sys.refresh_memory();

    let cpu_usage = sys.global_cpu_info().cpu_usage();
    let memory_used = sys.used_memory() as f32 / 1024.0 / 1024.0; // MB
    let memory_total = sys.total_memory() as f32 / 1024.0 / 1024.0; // MB

    Json(SystemStatus {
        cpu_usage,
        memory_used: memory_used as u64,
        memory_total: memory_total as u64,
    })
}

pub(crate) fn truncate_log_message(raw: &str, max_chars: usize) -> String {
    if raw.chars().count() <= max_chars {
        return raw.to_string();
    }
    let mut out = String::new();
    for ch in raw.chars().take(max_chars.saturating_sub(1)) {
        out.push(ch);
    }
    out.push('…');
    out
}

pub(crate) async fn get_recent_logs() -> Json<Vec<LogEntry>> {
    let mut logs: Vec<LogEntry> = Vec::new();

    if let Ok(task_runs) = crate::db::list_task_runs(40, None) {
        for run in task_runs {
            let level = match run.status.as_str() {
                "failed" | "blocked" | "error" => "ERROR",
                "manual_required" | "approval_required" => "WARN",
                _ => "INFO",
            };
            let summary = run.summary.unwrap_or_else(|| "-".to_string());
            let message = format!(
                "task_run status={} intent={} run_id={} summary={}",
                run.status,
                run.intent,
                run.run_id,
                truncate_log_message(&summary, 180)
            );
            logs.push(LogEntry {
                timestamp: run.created_at,
                level: level.to_string(),
                message,
            });
        }
    }

    if let Ok(verification_runs) = crate::db::list_verification_runs(20) {
        for verification in verification_runs {
            logs.push(LogEntry {
                timestamp: verification.created_at,
                level: if verification.ok { "INFO" } else { "WARN" }.to_string(),
                message: truncate_log_message(
                    &format!(
                        "verification kind={} ok={} summary={}",
                        verification.kind, verification.ok, verification.summary
                    ),
                    200,
                ),
            });
        }
    }

    logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    logs.truncate(80);
    Json(logs)
}

pub(crate) async fn get_system_health() -> Json<crate::dependency_check::SystemHealth> {
    let health = crate::dependency_check::SystemHealth::check_all();
    Json(health)
}

pub(crate) async fn scan_project_handler(
    Query(query): Query<ProjectScanQuery>,
) -> Json<ProjectScanResponse> {
    let workdir = query.workdir.unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    });
    let scanner = project_scanner::ProjectScanner::new(&workdir);
    let result = scanner.scan(query.max_files);
    let project_type = scanner.get_project_type();

    Json(ProjectScanResponse {
        project_type: project_type.as_str().to_string(),
        files: result.files,
        key_files: result.key_files,
    })
}

pub(crate) async fn run_runtime_verification_handler(
    Json(payload): Json<RuntimeVerifyRequest>,
) -> Json<runtime_verification::RuntimeVerifyResult> {
    let options = runtime_verification::RuntimeVerifyOptions {
        workdir: payload.workdir,
        run_backend: payload.run_backend,
        run_frontend: payload.run_frontend,
        run_e2e: payload.run_e2e,
        run_build_checks: payload.run_build_checks,
        backend_port: payload.backend_port,
        frontend_port: payload.frontend_port,
        backend_health_path: payload.backend_health_path,
    };
    let result = runtime_verification::run_runtime_verification(options).await;
    let summary = if result.issues.is_empty() {
        "Runtime verification passed".to_string()
    } else {
        format!("Runtime verification issues: {}", result.issues.len())
    };
    log_verification_run(
        "runtime",
        result.issues.is_empty(),
        &summary,
        Some(
            json!({ "issues": result.issues, "backend_health": result.backend_health, "frontend_health": result.frontend_health }),
        ),
    );
    Json(result)
}

pub(crate) async fn run_visual_verification_handler(
    State(state): State<AppState>,
    Json(payload): Json<visual_verification::VisualVerifyRequest>,
) -> Json<visual_verification::VisualVerifyResult> {
    let Some(llm) = &state.llm_client else {
        return Json(visual_verification::VisualVerifyResult {
            ok: false,
            verdicts: vec![],
        });
    };
    match visual_verification::verify_screen(llm.as_ref(), payload).await {
        Ok(result) => {
            let summary = if result.ok {
                "Visual verification passed"
            } else {
                "Visual verification failed"
            };
            let details = json!({
                "verdicts": result.verdicts.iter().map(|v| json!({ "prompt": v.prompt, "ok": v.ok })).collect::<Vec<_>>()
            });
            log_verification_run("visual", result.ok, summary, Some(details));
            Json(result)
        }
        Err(_) => Json(visual_verification::VisualVerifyResult {
            ok: false,
            verdicts: vec![],
        }),
    }
}

pub(crate) async fn run_semantic_verification_handler(
    Json(payload): Json<SemanticVerifyRequest>,
) -> Json<semantic_verification::SemanticVerificationResult> {
    let workdir = payload.workdir.unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    });
    let max_files = payload.max_files.unwrap_or(200);
    let result =
        semantic_verification::semantic_consistency(std::path::Path::new(&workdir), max_files);
    let details = json!({
        "issues": result.issues.iter().take(10).map(|i| json!({"file": i.file, "severity": i.severity, "reason": i.reason})).collect::<Vec<_>>()
    });
    log_verification_run("semantic", result.ok, &result.reason, Some(details));
    Json(result)
}

pub(crate) async fn run_performance_verification_handler(
    Json(payload): Json<PerformanceVerifyRequest>,
) -> Json<performance_verification::PerformanceVerificationResult> {
    let workdir = payload.workdir.unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    });
    let max_files = payload.max_files.unwrap_or(300);
    let result =
        performance_verification::performance_baseline(std::path::Path::new(&workdir), max_files);
    let details = json!({
        "metrics": result.metrics.iter().map(|m| json!({"name": m.name, "value": m.value, "threshold": m.threshold, "ok": m.ok})).collect::<Vec<_>>()
    });
    log_verification_run("performance", result.ok, &result.reason, Some(details));
    Json(result)
}

pub(crate) async fn run_consistency_verification_handler(
    Json(payload): Json<consistency_check::ConsistencyCheckRequest>,
) -> Json<consistency_check::ConsistencyCheckResult> {
    let result = consistency_check::run_consistency_check(payload);
    let details = json!({
        "summary": result.summary,
        "issues": result.issues.iter().take(10).map(|i| json!({"path": i.path, "source": i.source})).collect::<Vec<_>>()
    });
    log_verification_run("consistency", result.ok, &result.summary, Some(details));
    Json(result)
}

pub(crate) async fn run_judgment_handler(
    Json(payload): Json<judgment::JudgmentRequest>,
) -> Json<judgment::JudgmentResponse> {
    let result = judgment::evaluate_judgment(payload);
    Json(result)
}

pub(crate) async fn run_release_gate_handler(
    Json(payload): Json<release_gate::ReleaseGateRequest>,
) -> Json<release_gate::ReleaseGateResult> {
    let result = release_gate::run_release_gate(payload);
    let summary = if result.ok {
        "Release gate passed"
    } else {
        "Release gate failed"
    };
    let details = json!({
        "regressions": result.regressions.iter().take(10).cloned().collect::<Vec<_>>(),
        "warnings": result.warnings.iter().take(10).cloned().collect::<Vec<_>>()
    });
    log_verification_run("release_gate", result.ok, summary, Some(details));
    Json(result)
}

pub(crate) async fn run_exec_results_guard_handler(
    Json(payload): Json<tool_result_guard::ToolResultGuardRequest>,
) -> Json<tool_result_guard::ToolResultGuardResult> {
    let result = tool_result_guard::guard_exec_results(payload);
    Json(result)
}

pub(crate) async fn score_quality_handler(
    State(state): State<AppState>,
    Json(payload): Json<QualityScoreRequest>,
) -> Json<QualityScoreResponse> {
    let runtime = if let Some(rt) = payload.runtime {
        rt
    } else if let Some(opts) = payload.runtime_options {
        let options = runtime_verification::RuntimeVerifyOptions {
            workdir: opts.workdir,
            run_backend: opts.run_backend,
            run_frontend: opts.run_frontend,
            run_e2e: opts.run_e2e,
            run_build_checks: opts.run_build_checks,
            backend_port: opts.backend_port,
            frontend_port: opts.frontend_port,
            backend_health_path: opts.backend_health_path,
        };
        runtime_verification::run_runtime_verification(options).await
    } else {
        runtime_verification::RuntimeVerifyResult {
            backend_started: false,
            backend_health: false,
            backend_build_ok: None,
            frontend_started: false,
            frontend_health: false,
            frontend_build_ok: None,
            e2e_passed: None,
            issues: vec!["No runtime verification provided".to_string()],
            logs: Vec::new(),
        }
    };

    let use_llm = payload.use_llm.unwrap_or(false);
    let score = if use_llm {
        if let Some(llm) = &state.llm_client {
            match quality_scorer::score_quality_with_llm(
                llm.as_ref(),
                payload.goal.as_deref(),
                Some(&runtime),
                payload.code_review.as_ref(),
            )
            .await
            {
                Ok(score) => score,
                Err(_) => {
                    quality_scorer::score_quality(Some(&runtime), payload.code_review.as_ref())
                }
            }
        } else {
            quality_scorer::score_quality(Some(&runtime), payload.code_review.as_ref())
        }
    } else {
        quality_scorer::score_quality(Some(&runtime), payload.code_review.as_ref())
    };
    let _ = db::insert_quality_score(&score);
    let created_at = chrono::Utc::now().to_rfc3339();
    Json(QualityScoreResponse { created_at, score })
}

pub(crate) async fn latest_quality_handler() -> Json<Option<QualityScoreResponse>> {
    match db::get_latest_quality_score() {
        Ok(Some(record)) => {
            let score = quality_scorer::QualityScore {
                overall: record.overall,
                breakdown: record
                    .breakdown
                    .as_object()
                    .map(|map| {
                        map.iter()
                            .filter_map(|(k, v)| v.as_f64().map(|val| (k.clone(), val)))
                            .collect()
                    })
                    .unwrap_or_default(),
                issues: record.issues,
                strengths: record.strengths,
                recommendation: record.recommendation,
                summary: record.summary,
            };
            Json(Some(QualityScoreResponse {
                created_at: record.created_at,
                score,
            }))
        }
        _ => Json(None),
    }
}

pub(crate) async fn runtime_db_paths_handler() -> Json<RuntimeDbPathsResponse> {
    let cfg_path = std::env::var("STEER_COLLECTOR_CONFIG")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "configs/config.yaml".to_string());
    let collector = collector_pipeline::resolve_db_path(Some(std::path::Path::new(&cfg_path)));
    let collector_norm = collector
        .canonicalize()
        .unwrap_or_else(|_| collector.clone())
        .to_string_lossy()
        .to_string();

    let core_opt = db::current_db_path();
    let core_norm_opt = core_opt.as_ref().map(|p| {
        let path = std::path::Path::new(p);
        path.canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string()
    });

    let mismatch = match core_norm_opt.as_deref() {
        Some(core) => core != collector_norm,
        None => false,
    };
    let allow_mismatch = std::env::var("STEER_ALLOW_COLLECTOR_DB_MISMATCH")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);

    Json(RuntimeDbPathsResponse {
        core_db_path: core_norm_opt,
        collector_db_path: collector_norm,
        mismatch,
        allow_mismatch,
    })
}

pub(crate) async fn runtime_info_handler() -> Json<RuntimeInfoResponse> {
    let api_port = std::env::var("STEER_API_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(5680);
    let started_at = API_SERVER_STARTED_AT
        .get()
        .cloned()
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    Json(RuntimeInfoResponse {
        service: "AllvIa Core API".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        profile: if cfg!(debug_assertions) {
            "debug".to_string()
        } else {
            "release".to_string()
        },
        pid: std::process::id(),
        api_port,
        allow_no_key: crate::env_flag("STEER_API_ALLOW_NO_KEY"),
        started_at,
        binary_path: std::env::current_exe()
            .ok()
            .map(|p| p.to_string_lossy().to_string()),
        current_dir: std::env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().to_string()),
    })
}

pub(crate) async fn lock_metrics_handler() -> Json<LockMetricsResponse> {
    let snapshot = crate::singleton_lock::lock_metrics_snapshot();
    Json(LockMetricsResponse {
        acquired: snapshot.acquired,
        bypassed: snapshot.bypassed,
        blocked: snapshot.blocked,
        stale_recovered: snapshot.stale_recovered,
        rejected: snapshot.rejected,
    })
}

pub(crate) async fn get_quality_metrics() -> Json<QualityMetrics> {
    let collector = feedback_collector::FeedbackCollector::new();
    let metrics = collector.get_quality_metrics();

    Json(QualityMetrics {
        total: metrics.total_executions,
        success: metrics.successful_executions,
        rate: metrics.success_rate,
    })
}
