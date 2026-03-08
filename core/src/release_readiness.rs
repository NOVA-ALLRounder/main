use crate::{http_e2e, launch_eval, release_gate};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

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

struct ArchivePaths {
    release_readiness_dir: std::path::PathBuf,
    release_readiness_json: std::path::PathBuf,
    release_readiness_markdown: std::path::PathBuf,
    launch_eval_dir: std::path::PathBuf,
    launch_eval_json: std::path::PathBuf,
    launch_eval_markdown: std::path::PathBuf,
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

fn build_archive_paths(workdir: &Path, generated_at: &str) -> ArchivePaths {
    let slug = archive_slug(generated_at);
    let release_readiness_dir = workdir
        .join("reports/release_readiness/history")
        .join(&slug);
    let launch_eval_dir = workdir.join("reports/launch_eval/history").join(&slug);
    ArchivePaths {
        release_readiness_json: release_readiness_dir.join("release_readiness.json"),
        release_readiness_markdown: release_readiness_dir.join("release_readiness.md"),
        launch_eval_json: launch_eval_dir.join("launch_eval.json"),
        launch_eval_markdown: launch_eval_dir.join("launch_eval.md"),
        release_readiness_dir,
        launch_eval_dir,
    }
}

pub fn list_release_readiness_history(
    workdir: &Path,
    limit: usize,
) -> Result<Vec<ReleaseReadinessHistoryEntry>> {
    let limit = limit.clamp(1, 100);
    let history_dir = workdir.join("reports/release_readiness/history");
    if !history_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = fs::read_dir(&history_dir)
        .with_context(|| format!("failed to read {}", history_dir.display()))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path().join("release_readiness.json");
            if !path.exists() {
                return None;
            }
            let payload = fs::read_to_string(&path).ok()?;
            let report = serde_json::from_str::<ReleaseReadinessReport>(&payload).ok()?;
            Some(ReleaseReadinessHistoryEntry {
                generated_at: report.generated_at,
                status: report.status,
                ready_for_launch: report.ready_for_launch,
                candidate_snapshot_count: report.candidate_snapshot.scenario_count,
                launch_eval_passed: report.launch_eval.passed,
                launch_eval_total: report.launch_eval.total,
                http_e2e_ok: report.http_e2e.as_ref().map(|value| value.ok),
                http_e2e_passed: report.http_e2e.as_ref().map(|value| value.passed),
                http_e2e_total: report.http_e2e.as_ref().map(|value| value.total),
                blocker_count: report.blockers.len(),
                advisory_count: report.advisories.len(),
                report_json_path: path.display().to_string(),
                report_markdown_path: path
                    .with_file_name("release_readiness.md")
                    .display()
                    .to_string(),
            })
        })
        .collect::<Vec<_>>();

    entries.sort_by(|left, right| right.generated_at.cmp(&left.generated_at));
    entries.truncate(limit);
    Ok(entries)
}

pub fn load_latest_release_readiness_report(
    workdir: &Path,
) -> Result<Option<ReleaseReadinessReport>> {
    let path = workdir.join("reports/release_readiness/latest.json");
    if !path.exists() {
        return Ok(None);
    }
    let payload =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let report = serde_json::from_str::<ReleaseReadinessReport>(&payload)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(report))
}

fn status_rank(status: &str) -> i32 {
    match status {
        "ready" => 0,
        "warning" => 1,
        "needs_data" => 2,
        "blocked" => 3,
        _ => 4,
    }
}

fn build_history_trend_summary(
    current: &ReleaseReadinessReport,
    history: &[ReleaseReadinessHistoryEntry],
) -> ReleaseReadinessTrendSummary {
    if history.is_empty() {
        return ReleaseReadinessTrendSummary {
            compared_runs: 0,
            stable_ready_streak: if current.ready_for_launch { 1 } else { 0 },
            status_regressed: false,
            snapshot_delta: 0,
            launch_eval_pass_rate_delta_pct: 0.0,
            http_e2e_pass_rate_delta_pct: 0.0,
            http_e2e_regressed: false,
            blocker_delta: 0,
            advisory_delta: 0,
            warnings: Vec::new(),
            summary: "No prior readiness history yet.".to_string(),
        };
    }

    let previous = &history[0];
    let current_pass_rate = if current.launch_eval.total > 0 {
        (current.launch_eval.passed as f64 / current.launch_eval.total as f64) * 100.0
    } else {
        0.0
    };
    let previous_pass_rate = if previous.launch_eval_total > 0 {
        (previous.launch_eval_passed as f64 / previous.launch_eval_total as f64) * 100.0
    } else {
        0.0
    };
    let current_http_pass_rate = current
        .http_e2e
        .as_ref()
        .filter(|value| value.total > 0)
        .map(|value| (value.passed as f64 / value.total as f64) * 100.0);
    let previous_http_pass_rate = match (previous.http_e2e_passed, previous.http_e2e_total) {
        (Some(passed), Some(total)) if total > 0 => Some((passed as f64 / total as f64) * 100.0),
        _ => None,
    };
    let snapshot_delta =
        current.candidate_snapshot.scenario_count as i64 - previous.candidate_snapshot_count as i64;
    let blocker_delta = current.blockers.len() as i64 - previous.blocker_count as i64;
    let advisory_delta = current.advisories.len() as i64 - previous.advisory_count as i64;
    let pass_rate_delta = current_pass_rate - previous_pass_rate;
    let http_pass_rate_delta = match (current_http_pass_rate, previous_http_pass_rate) {
        (Some(current_rate), Some(previous_rate)) => current_rate - previous_rate,
        _ => 0.0,
    };
    let status_regressed = status_rank(&current.status) > status_rank(&previous.status);
    let http_e2e_regressed = previous.http_e2e_ok.unwrap_or(false)
        && current
            .http_e2e
            .as_ref()
            .map(|value| value.ok)
            .unwrap_or(false)
            == false;

    let mut warnings = Vec::new();
    if status_regressed {
        warnings.push(format!(
            "status_regressed {} -> {}",
            previous.status, current.status
        ));
    }
    if snapshot_delta < 0 {
        warnings.push(format!("candidate_snapshot_dropped {}", snapshot_delta));
    }
    if pass_rate_delta < 0.0 {
        warnings.push(format!(
            "launch_eval_pass_rate_dropped {:.1}pp",
            pass_rate_delta
        ));
    }
    if http_e2e_regressed {
        warnings.push(format!(
            "http_e2e_regressed {} -> {}",
            if previous.http_e2e_ok.unwrap_or(false) {
                "ok"
            } else {
                "fail"
            },
            current
                .http_e2e
                .as_ref()
                .map(|value| if value.ok { "ok" } else { "fail" })
                .unwrap_or("missing")
        ));
    }
    if http_pass_rate_delta < 0.0 {
        warnings.push(format!(
            "http_e2e_pass_rate_dropped {:.1}pp",
            http_pass_rate_delta
        ));
    }
    if blocker_delta > 0 {
        warnings.push(format!("blockers_increased +{}", blocker_delta));
    }
    if advisory_delta > 0 {
        warnings.push(format!("advisories_increased +{}", advisory_delta));
    }

    let mut stable_ready_streak = if current.ready_for_launch { 1 } else { 0 };
    if current.ready_for_launch {
        for entry in history {
            if entry.ready_for_launch {
                stable_ready_streak += 1;
            } else {
                break;
            }
        }
    }

    let summary = if warnings.is_empty() {
        format!(
            "Stable versus last run. Snapshot delta {:+}, launch eval {:+.1}pp, http E2E {:+.1}pp, ready streak {}.",
            snapshot_delta, pass_rate_delta, http_pass_rate_delta, stable_ready_streak
        )
    } else {
        format!("Drift detected versus last run: {}.", warnings.join(", "))
    };

    ReleaseReadinessTrendSummary {
        compared_runs: history.len(),
        stable_ready_streak,
        status_regressed,
        snapshot_delta,
        launch_eval_pass_rate_delta_pct: pass_rate_delta,
        http_e2e_pass_rate_delta_pct: http_pass_rate_delta,
        http_e2e_regressed,
        blocker_delta,
        advisory_delta,
        warnings,
        summary,
    }
}

fn render_markdown_report(report: &ReleaseReadinessReport) -> String {
    let mut lines = vec![
        "# Allvia Release Readiness".to_string(),
        String::new(),
        format!("- generated_at: {}", report.generated_at),
        format!("- workdir: {}", report.workdir),
        format!("- config_path: {}", report.config_path),
        format!(
            "- candidate_snapshot: {} scenarios",
            report.candidate_snapshot.scenario_count
        ),
        format!(
            "- synced_nl_runs_from_launch_ops: {}",
            report.synced_nl_runs_from_launch_ops
        ),
        format!(
            "- launch_eval: {}/{} passed",
            report.launch_eval.passed, report.launch_eval.total
        ),
        format!(
            "- http_e2e: {}",
            report
                .http_e2e
                .as_ref()
                .map(|value| format!("{}/{} passed", value.passed, value.total))
                .unwrap_or_else(|| "not loaded".to_string())
        ),
        format!("- release_gate_ok: {}", report.release_gate.ok),
        format!("- readiness_status: {}", report.status),
        format!("- ready_for_launch: {}", report.ready_for_launch),
        format!("- baseline_saved: {}", report.baseline_saved),
        String::new(),
        "## Launch Readiness".to_string(),
    ];

    if report.blockers.is_empty() {
        lines.push("- blockers: none".to_string());
    } else {
        for item in &report.blockers {
            lines.push(format!("- blocker: {}", item));
        }
    }

    if report.advisories.is_empty() {
        lines.push("- advisories: none".to_string());
    } else {
        for item in &report.advisories {
            lines.push(format!("- advisory: {}", item));
        }
    }

    if let Some(trend) = &report.history_trend {
        lines.extend([String::new(), "## Trend".to_string()]);
        lines.push(format!("- summary: {}", trend.summary));
        lines.push(format!("- compared_runs: {}", trend.compared_runs));
        lines.push(format!(
            "- stable_ready_streak: {}",
            trend.stable_ready_streak
        ));
        lines.push(format!("- snapshot_delta: {:+}", trend.snapshot_delta));
        lines.push(format!(
            "- launch_eval_pass_rate_delta_pct: {:+.1}",
            trend.launch_eval_pass_rate_delta_pct
        ));
        lines.push(format!(
            "- http_e2e_pass_rate_delta_pct: {:+.1}",
            trend.http_e2e_pass_rate_delta_pct
        ));
        if trend.warnings.is_empty() {
            lines.push("- warnings: none".to_string());
        } else {
            for item in &trend.warnings {
                lines.push(format!("- warning: {}", item));
            }
        }
    }

    lines.extend([String::new(), "## Release Gate".to_string()]);

    if report.release_gate.regressions.is_empty() {
        lines.push("- regressions: none".to_string());
    } else {
        for item in &report.release_gate.regressions {
            lines.push(format!("- regression: {}", item));
        }
    }

    if report.release_gate.warnings.is_empty() {
        lines.push("- warnings: none".to_string());
    } else {
        for item in &report.release_gate.warnings {
            lines.push(format!("- warning: {}", item));
        }
    }

    lines.push(String::new());
    lines.push("## Launch Eval".to_string());
    for result in &report.launch_eval.results {
        lines.push(format!(
            "- {}: {}",
            result.id,
            if result.passed { "pass" } else { "fail" }
        ));
    }

    lines.push(String::new());
    lines.push("## HTTP E2E".to_string());
    match &report.http_e2e {
        Some(http_report) => {
            lines.push(format!(
                "- status: {}",
                if http_report.ok { "ok" } else { "failed" }
            ));
            lines.push(format!(
                "- steps: {}/{}",
                http_report.passed, http_report.total
            ));
            lines.push(format!("- report: {}", http_report.report_markdown_path));
            for step in http_report.steps.iter().filter(|step| !step.ok).take(5) {
                lines.push(format!("- failed_step: {} ({})", step.name, step.detail));
            }
        }
        None => {
            if let Some(error) = &report.http_e2e_load_error {
                lines.push(format!("- load_error: {}", error));
            } else {
                lines.push("- status: not_loaded".to_string());
            }
        }
    }

    lines.join("\n")
}

fn finalize_release_readiness(mut report: ReleaseReadinessReport) -> ReleaseReadinessReport {
    let mut blockers = Vec::new();
    let mut advisories = report.release_gate.warnings.clone();

    let min_candidates = std::env::var("ALLVIA_RELEASE_READINESS_MIN_CANDIDATE_SCENARIOS")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(5);

    if report.candidate_snapshot.scenario_count < min_candidates {
        blockers.push(format!(
            "Need more real launch-eval candidates ({} < {})",
            report.candidate_snapshot.scenario_count, min_candidates
        ));
    }

    if report.launch_eval.failed > 0 {
        blockers.push(format!(
            "Launch eval has failing scenarios ({}/{})",
            report.launch_eval.failed, report.launch_eval.total
        ));
    }

    if let Some(http_report) = &report.http_e2e {
        if !http_report.ok {
            blockers.push(format!(
                "HTTP E2E has failing steps ({}/{})",
                http_report.total.saturating_sub(http_report.passed),
                http_report.total
            ));
        } else {
            let stale_hours = std::env::var("ALLVIA_RELEASE_READINESS_HTTP_E2E_STALE_HOURS")
                .ok()
                .and_then(|raw| raw.parse::<i64>().ok())
                .unwrap_or(24);
            if let Ok(generated_at) =
                chrono::DateTime::parse_from_rfc3339(&http_report.generated_at)
            {
                let age = chrono::Utc::now()
                    .signed_duration_since(generated_at.with_timezone(&chrono::Utc));
                if age.num_hours() >= stale_hours {
                    advisories.push(format!(
                        "HTTP E2E report is stale ({}h >= {}h)",
                        age.num_hours(),
                        stale_hours
                    ));
                }
            }
        }
    } else if let Some(error) = &report.http_e2e_load_error {
        blockers.push(format!("HTTP E2E report could not be loaded: {}", error));
    } else {
        advisories.push("No live HTTP E2E report found before readiness run".to_string());
    }

    blockers.extend(report.release_gate.regressions.clone());

    if let Some(error) = report
        .release_gate
        .current
        .launch_eval_candidate_snapshot_refresh_error
        .clone()
    {
        blockers.push(format!("Candidate snapshot refresh failed: {}", error));
    }

    if report.release_gate.baseline.is_none() {
        advisories.push("No historical release baseline existed before this run".to_string());
    }

    let status = if blockers.is_empty() {
        if advisories.is_empty() {
            "ready"
        } else {
            "warning"
        }
    } else if blockers
        .iter()
        .all(|item| item.contains("Need more real launch-eval candidates"))
    {
        "needs_data"
    } else {
        "blocked"
    };

    report.ready_for_launch = blockers.is_empty();
    report.status = status.to_string();
    report.blockers = blockers;
    report.advisories = advisories;
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_report(
        generated_at: &str,
        status: &str,
        ready_for_launch: bool,
    ) -> ReleaseReadinessReport {
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

        let markdown = render_markdown_report(&report);
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
        let summary = build_history_trend_summary(&report, &history);
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

        let finalized = finalize_release_readiness(report);
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

        let finalized = finalize_release_readiness(report);
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
        let older = build_archive_paths(&workdir, "2026-03-08T03:16:14Z");
        let newer = build_archive_paths(&workdir, "2026-03-08T06:10:00Z");
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

        let history = list_release_readiness_history(&workdir, 10).expect("history");
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

        let loaded = load_latest_release_readiness_report(&workdir)
            .expect("load latest report")
            .expect("latest report exists");
        assert_eq!(loaded.generated_at, "2026-03-08T07:30:00Z");
        assert_eq!(loaded.status, "ready");

        let _ = fs::remove_dir_all(workdir);
    }
}
