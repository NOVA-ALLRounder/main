use axum::{extract::Query, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;

use crate::{launch_eval, release_gate, release_readiness};

pub(super) fn default_server_workdir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn is_truthy_env_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn allow_operational_workdir_override() -> bool {
    std::env::var("ALLVIA_API_ALLOW_WORKDIR_OVERRIDE")
        .ok()
        .map(|value| is_truthy_env_value(&value))
        .unwrap_or(false)
}

fn allow_operational_path_override() -> bool {
    allow_operational_workdir_override()
        || std::env::var("ALLVIA_API_ALLOW_PATH_OVERRIDE")
            .ok()
            .map(|value| is_truthy_env_value(&value))
            .unwrap_or(false)
}

pub(super) fn resolve_operational_workdir(requested: Option<&str>) -> PathBuf {
    if allow_operational_workdir_override() {
        if let Some(raw) = requested.map(str::trim).filter(|value| !value.is_empty()) {
            return PathBuf::from(raw);
        }
    }
    default_server_workdir()
}

pub(super) fn resolve_operational_file_override(requested: Option<&str>) -> Option<PathBuf> {
    if allow_operational_path_override() {
        if let Some(raw) = requested.map(str::trim).filter(|value| !value.is_empty()) {
            return Some(PathBuf::from(raw));
        }
    }
    None
}

fn requested_operational_override(requested: Option<&str>) -> bool {
    requested
        .map(str::trim)
        .map(|value| !value.is_empty())
        .unwrap_or(false)
}

fn default_launch_eval_config_path() -> PathBuf {
    default_server_workdir().join("configs/launch_eval.yaml")
}

fn has_release_baseline_override(payload: &release_gate::ReleaseBaselineRequest) -> bool {
    payload.workdir.is_some()
        || payload.max_files.is_some()
        || payload.consistency.is_some()
        || payload.semantic.is_some()
        || payload.performance.is_some()
        || payload.quality.is_some()
        || payload.perf_regression_pct.is_some()
        || payload.quality_drop.is_some()
        || payload.launch_error_rate_pct.is_some()
        || payload.launch_low_confidence_rate_pct.is_some()
        || payload.launch_cache_hit_rate_drop_pct.is_some()
        || payload.recommendation_approval_rate_min.is_some()
        || payload.launch_eval_config_path.is_some()
        || payload.launch_eval_report_path.is_some()
        || payload.launch_eval_snapshot_output_path.is_some()
        || payload.refresh_launch_eval_candidates.is_some()
        || payload.launch_eval_candidate_limit.is_some()
}

pub(super) fn default_release_baseline_request() -> release_gate::ReleaseBaselineRequest {
    release_gate::ReleaseBaselineRequest {
        workdir: None,
        max_files: None,
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
        launch_eval_candidate_limit: Some(20),
        launch_eval_snapshot_output_path: None,
    }
}

fn launch_eval_candidate_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(12).clamp(1, 30)
}

#[derive(Deserialize)]
pub(super) struct LaunchEvalCandidatesQuery {
    pub limit: Option<usize>,
    pub provenance: Option<String>,
    pub workdir: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct LaunchEvalSnapshotRequest {
    pub limit: Option<usize>,
    pub output_path: Option<String>,
    pub workdir: Option<String>,
    pub provenance: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct LaunchEvalSnapshotInfoQuery {
    pub output_path: Option<String>,
    pub workdir: Option<String>,
    pub provenance: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReleaseReadinessRequest {
    pub workdir: Option<String>,
    pub config_path: Option<String>,
    pub candidate_limit: Option<usize>,
    pub snapshot_output_path: Option<String>,
    pub save_baseline: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReleaseReadinessHistoryQuery {
    pub workdir: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(super) struct HttpE2ERequest {
    pub workdir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct HttpE2EHistoryQuery {
    pub workdir: Option<String>,
    pub limit: Option<usize>,
}

pub(super) async fn set_release_baseline_handler(
    Json(payload): Json<release_gate::ReleaseBaselineRequest>,
) -> Result<Json<release_gate::ReleaseBaseline>, StatusCode> {
    if has_release_baseline_override(&payload) && !allow_operational_path_override() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let request = if allow_operational_path_override() && has_release_baseline_override(&payload) {
        payload
    } else {
        default_release_baseline_request()
    };
    let baseline = release_gate::build_baseline(request);
    release_gate::save_baseline(&baseline);
    Ok(Json(baseline))
}

pub(super) async fn run_release_readiness_handler(
    Json(payload): Json<ReleaseReadinessRequest>,
) -> Result<Json<release_readiness::ReleaseReadinessReport>, StatusCode> {
    if payload.save_baseline.unwrap_or(false) && !allow_operational_path_override() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if requested_operational_override(payload.config_path.as_deref())
        && !allow_operational_path_override()
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if requested_operational_override(payload.snapshot_output_path.as_deref())
        && !allow_operational_path_override()
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let workdir = resolve_operational_workdir(payload.workdir.as_deref());
    let config_path = resolve_operational_file_override(payload.config_path.as_deref())
        .unwrap_or_else(default_launch_eval_config_path);
    let snapshot_output_path =
        resolve_operational_file_override(payload.snapshot_output_path.as_deref())
            .map(|value| value.display().to_string());
    let report = release_readiness::run_release_readiness(
        &workdir,
        &config_path,
        payload.candidate_limit.unwrap_or(20).clamp(1, 50),
        snapshot_output_path.as_deref(),
        payload.save_baseline.unwrap_or(false) && allow_operational_path_override(),
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ok = report.ready_for_launch;
    let summary = if ok {
        "Release readiness passed"
    } else {
        "Release readiness found issues"
    };
    let details = json!({
        "candidate_snapshot_scenarios": report.candidate_snapshot.scenario_count,
        "launch_eval_passed": report.launch_eval.passed,
        "launch_eval_total": report.launch_eval.total,
        "http_e2e_passed": report.http_e2e.as_ref().map(|value| value.passed),
        "http_e2e_total": report.http_e2e.as_ref().map(|value| value.total),
        "http_e2e_ok": report.http_e2e.as_ref().map(|value| value.ok),
        "release_gate_ok": report.release_gate.ok,
        "status": report.status,
        "ready_for_launch": report.ready_for_launch,
        "baseline_saved": report.baseline_saved,
        "report_json_path": report.report_json_path,
        "report_markdown_path": report.report_markdown_path,
        "blockers": report.blockers.iter().take(10).cloned().collect::<Vec<_>>(),
        "regressions": report.release_gate.regressions.iter().take(10).cloned().collect::<Vec<_>>(),
        "warnings": report.advisories.iter().take(10).cloned().collect::<Vec<_>>()
    });
    super::log_verification_run("release_readiness", ok, summary, Some(details));

    Ok(Json(report))
}

pub(super) async fn get_latest_release_readiness_handler(
    Query(query): Query<ReleaseReadinessHistoryQuery>,
) -> Result<Json<Option<release_readiness::ReleaseReadinessReport>>, StatusCode> {
    let workdir = resolve_operational_workdir(query.workdir.as_deref());
    let report = release_readiness::load_latest_release_readiness_report(&workdir)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(report))
}

pub(super) async fn get_release_readiness_history_handler(
    Query(query): Query<ReleaseReadinessHistoryQuery>,
) -> Result<Json<Vec<release_readiness::ReleaseReadinessHistoryEntry>>, StatusCode> {
    let workdir = resolve_operational_workdir(query.workdir.as_deref());
    let limit = query.limit.unwrap_or(10).clamp(1, 50);
    let history = release_readiness::list_release_readiness_history(&workdir, limit)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(history))
}

pub(super) async fn run_http_e2e_handler(
    Json(payload): Json<HttpE2ERequest>,
) -> Result<Json<crate::http_e2e::HttpE2EReport>, StatusCode> {
    let workdir = resolve_operational_workdir(payload.workdir.as_deref());
    let report = crate::http_e2e::run_http_e2e(&workdir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let summary = if report.ok {
        "HTTP E2E passed"
    } else {
        "HTTP E2E found failures"
    };
    let details = json!({
        "passed": report.passed,
        "total": report.total,
        "report_json_path": report.report_json_path,
        "report_markdown_path": report.report_markdown_path,
        "failed_steps": report
            .steps
            .iter()
            .filter(|step| !step.ok)
            .map(|step| step.name.clone())
            .take(10)
            .collect::<Vec<_>>()
    });
    super::log_verification_run("http_e2e", report.ok, summary, Some(details));

    Ok(Json(report))
}

pub(super) async fn get_latest_http_e2e_handler(
    Query(query): Query<HttpE2ERequest>,
) -> Result<Json<Option<crate::http_e2e::HttpE2EReport>>, StatusCode> {
    let workdir = resolve_operational_workdir(query.workdir.as_deref());
    let report = crate::http_e2e::load_latest_http_e2e_report(&workdir)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(report))
}

pub(super) async fn get_http_e2e_history_handler(
    Query(query): Query<HttpE2EHistoryQuery>,
) -> Result<Json<Vec<crate::http_e2e::HttpE2EHistoryEntry>>, StatusCode> {
    let workdir = resolve_operational_workdir(query.workdir.as_deref());
    let limit = query.limit.unwrap_or(10).clamp(1, 50);
    let history = crate::http_e2e::list_http_e2e_history(&workdir, limit)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(history))
}

pub(super) async fn get_launch_eval_candidates_handler(
    Query(query): Query<LaunchEvalCandidatesQuery>,
) -> Json<Vec<launch_eval::LaunchEvalCandidate>> {
    let workdir = resolve_operational_workdir(query.workdir.as_deref());
    let provenance_filter =
        launch_eval::parse_launch_eval_candidate_provenance_filter(query.provenance.as_deref());
    Json(launch_eval::generate_launch_eval_candidates_with_filter(
        &workdir,
        launch_eval_candidate_limit(query.limit),
        provenance_filter,
    ))
}

pub(super) async fn get_launch_eval_candidate_snapshot_info_handler(
    Query(query): Query<LaunchEvalSnapshotInfoQuery>,
) -> Result<Json<launch_eval::LaunchEvalCandidateSnapshotInfo>, StatusCode> {
    if requested_operational_override(query.output_path.as_deref())
        && !allow_operational_path_override()
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let workdir = resolve_operational_workdir(query.workdir.as_deref());
    let provenance_filter =
        launch_eval::parse_launch_eval_candidate_provenance_filter(query.provenance.as_deref());
    let output_path = resolve_operational_file_override(query.output_path.as_deref())
        .map(|value| value.display().to_string());
    Ok(Json(
        launch_eval::read_launch_eval_candidate_snapshot_info_with_filter(
            &workdir,
            output_path.as_deref(),
            provenance_filter,
        ),
    ))
}

pub(super) async fn write_launch_eval_candidate_snapshot_handler(
    Json(payload): Json<LaunchEvalSnapshotRequest>,
) -> Result<Json<launch_eval::LaunchEvalCandidateSnapshot>, StatusCode> {
    if requested_operational_override(payload.output_path.as_deref())
        && !allow_operational_path_override()
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let limit = launch_eval_candidate_limit(payload.limit);
    let workdir = resolve_operational_workdir(payload.workdir.as_deref());
    let provenance_filter =
        launch_eval::parse_launch_eval_candidate_provenance_filter(payload.provenance.as_deref());
    let output_path = resolve_operational_file_override(payload.output_path.as_deref())
        .map(|value| value.display().to_string());
    let snapshot = launch_eval::write_launch_eval_candidate_snapshot_with_filter(
        &workdir,
        output_path.as_deref(),
        limit,
        provenance_filter,
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(snapshot))
}
