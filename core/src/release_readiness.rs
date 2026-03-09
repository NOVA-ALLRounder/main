mod support;

use crate::{http_e2e, launch_eval, release_gate};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use support::{
    build_archive_paths, build_history_trend_summary, finalize_release_readiness,
    render_markdown_report,
};
pub use support::{list_release_readiness_history, load_latest_release_readiness_report};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseReadinessReport {
    pub generated_at: String,
    pub workdir: String,
    pub config_path: String,
    pub report_json_path: String,
    pub report_markdown_path: String,
    #[serde(default)]
    pub archived_history_json_path: Option<String>,
    #[serde(default)]
    pub archived_history_markdown_path: Option<String>,
    #[serde(default)]
    pub archived_launch_eval_json_path: Option<String>,
    #[serde(default)]
    pub archived_launch_eval_markdown_path: Option<String>,
    #[serde(default)]
    pub history_trend: Option<ReleaseReadinessTrendSummary>,
    pub baseline_saved: bool,
    pub synced_nl_runs_from_launch_ops: i64,
    #[serde(default)]
    pub http_e2e: Option<http_e2e::HttpE2EReport>,
    #[serde(default)]
    pub http_e2e_load_error: Option<String>,
    pub candidate_snapshot: launch_eval::LaunchEvalCandidateSnapshot,
    pub launch_eval: launch_eval::LaunchEvalReport,
    pub release_gate: release_gate::ReleaseGateResult,
    pub status: String,
    pub ready_for_launch: bool,
    pub blockers: Vec<String>,
    pub advisories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReleaseReadinessTrendSummary {
    pub compared_runs: usize,
    pub stable_ready_streak: usize,
    pub status_regressed: bool,
    pub snapshot_delta: i64,
    pub launch_eval_pass_rate_delta_pct: f64,
    pub http_e2e_pass_rate_delta_pct: f64,
    pub http_e2e_regressed: bool,
    pub blocker_delta: i64,
    pub advisory_delta: i64,
    pub warnings: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseReadinessHistoryEntry {
    pub generated_at: String,
    pub status: String,
    pub ready_for_launch: bool,
    pub candidate_snapshot_count: usize,
    pub launch_eval_passed: usize,
    pub launch_eval_total: usize,
    pub http_e2e_ok: Option<bool>,
    pub http_e2e_passed: Option<usize>,
    pub http_e2e_total: Option<usize>,
    pub blocker_count: usize,
    pub advisory_count: usize,
    pub report_json_path: String,
    pub report_markdown_path: String,
}

pub async fn run_release_readiness(
    workdir: &Path,
    config_path: &Path,
    candidate_limit: usize,
    snapshot_output: Option<&str>,
    save_baseline: bool,
) -> Result<ReleaseReadinessReport> {
    crate::load_env_with_fallback();

    let candidate_snapshot = launch_eval::write_launch_eval_candidate_snapshot(
        workdir,
        snapshot_output,
        candidate_limit,
    )?;
    let launch_eval_report = launch_eval::run_launch_eval_from_path(config_path).await?;
    crate::db::reset_connection();
    crate::db::init().context("failed to re-open primary DB after launch eval")?;
    let backfill_limit = std::env::var("ALLVIA_RELEASE_READINESS_NL_BACKFILL_LIMIT")
        .ok()
        .and_then(|raw| raw.trim().parse::<i64>().ok())
        .map(|value| value.clamp(1, 5000))
        .unwrap_or(500);
    let synced_nl_runs_from_launch_ops =
        crate::db::sync_release_nl_runs_from_launch_ops(backfill_limit)
            .context("failed to sync release nl runs from launch ops")?;
    let (http_e2e, http_e2e_load_error) = match http_e2e::load_latest_http_e2e_report(workdir) {
        Ok(report) => (report, None),
        Err(error) => (None, Some(error.to_string())),
    };
    let release_gate = release_gate::run_release_gate(release_gate::ReleaseGateRequest {
        workdir: Some(workdir.display().to_string()),
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
        launch_eval_config_path: Some(config_path.display().to_string()),
        launch_eval_report_path: Some(launch_eval_report.report_json_path.clone()),
        refresh_launch_eval_candidates: Some(false),
        launch_eval_candidate_limit: Some(candidate_limit),
        launch_eval_snapshot_output_path: snapshot_output.map(|value| value.to_string()),
    });
    if let Some(score) = release_gate.current.quality.as_ref() {
        let _ = crate::db::insert_quality_score(score);
    }

    if save_baseline {
        release_gate::save_baseline(&release_gate.current);
    }

    let report_dir = workdir.join("reports/release_readiness");
    fs::create_dir_all(&report_dir)
        .with_context(|| format!("failed to create {}", report_dir.display()))?;
    let report_json_path = report_dir.join("latest.json");
    let report_markdown_path = report_dir.join("latest.md");

    let report = ReleaseReadinessReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        workdir: workdir.display().to_string(),
        config_path: config_path.display().to_string(),
        report_json_path: report_json_path.display().to_string(),
        report_markdown_path: report_markdown_path.display().to_string(),
        baseline_saved: save_baseline,
        synced_nl_runs_from_launch_ops,
        http_e2e,
        http_e2e_load_error,
        candidate_snapshot,
        launch_eval: launch_eval_report,
        release_gate,
        status: "unknown".to_string(),
        ready_for_launch: false,
        blockers: Vec::new(),
        advisories: Vec::new(),
        archived_history_json_path: None,
        archived_history_markdown_path: None,
        archived_launch_eval_json_path: None,
        archived_launch_eval_markdown_path: None,
        history_trend: None,
    };
    let mut report = finalize_release_readiness(report);
    let trend_window = std::env::var("ALLVIA_RELEASE_READINESS_TREND_WINDOW")
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .map(|value| value.clamp(1, 20))
        .unwrap_or(5);
    let prior_history = list_release_readiness_history(workdir, trend_window).unwrap_or_default();
    report.history_trend = Some(build_history_trend_summary(&report, &prior_history));
    let markdown = render_markdown_report(&report);

    let archive_paths = build_archive_paths(workdir, &report.generated_at);
    fs::create_dir_all(&archive_paths.release_readiness_dir).with_context(|| {
        format!(
            "failed to create {}",
            archive_paths.release_readiness_dir.display()
        )
    })?;
    fs::create_dir_all(&archive_paths.launch_eval_dir).with_context(|| {
        format!(
            "failed to create {}",
            archive_paths.launch_eval_dir.display()
        )
    })?;
    report.archived_history_json_path =
        Some(archive_paths.release_readiness_json.display().to_string());
    report.archived_history_markdown_path = Some(
        archive_paths
            .release_readiness_markdown
            .display()
            .to_string(),
    );
    report.archived_launch_eval_json_path =
        Some(archive_paths.launch_eval_json.display().to_string());
    report.archived_launch_eval_markdown_path =
        Some(archive_paths.launch_eval_markdown.display().to_string());

    let report_json = serde_json::to_string_pretty(&report)
        .context("failed to serialize release readiness json")?;

    fs::write(&report_json_path, &report_json)
        .with_context(|| format!("failed to write {}", report_json_path.display()))?;
    fs::write(&report_markdown_path, &markdown)
        .with_context(|| format!("failed to write {}", report_markdown_path.display()))?;
    fs::write(&archive_paths.release_readiness_json, &report_json).with_context(|| {
        format!(
            "failed to write {}",
            archive_paths.release_readiness_json.display()
        )
    })?;
    fs::write(&archive_paths.release_readiness_markdown, &markdown).with_context(|| {
        format!(
            "failed to write {}",
            archive_paths.release_readiness_markdown.display()
        )
    })?;
    fs::copy(
        &report.launch_eval.report_json_path,
        &archive_paths.launch_eval_json,
    )
    .with_context(|| {
        format!(
            "failed to archive {} to {}",
            report.launch_eval.report_json_path,
            archive_paths.launch_eval_json.display()
        )
    })?;
    fs::copy(
        &report.launch_eval.report_markdown_path,
        &archive_paths.launch_eval_markdown,
    )
    .with_context(|| {
        format!(
            "failed to archive {} to {}",
            report.launch_eval.report_markdown_path,
            archive_paths.launch_eval_markdown.display()
        )
    })?;

    Ok(report)
}

#[cfg(test)]
mod tests;
