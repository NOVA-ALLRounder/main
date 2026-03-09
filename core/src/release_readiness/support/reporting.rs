use super::super::ReleaseReadinessReport;
use std::path::Path;

pub(crate) struct ArchivePaths {
    pub(crate) release_readiness_dir: std::path::PathBuf,
    pub(crate) release_readiness_json: std::path::PathBuf,
    pub(crate) release_readiness_markdown: std::path::PathBuf,
    pub(crate) launch_eval_dir: std::path::PathBuf,
    pub(crate) launch_eval_json: std::path::PathBuf,
    pub(crate) launch_eval_markdown: std::path::PathBuf,
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

pub(crate) fn build_archive_paths(workdir: &Path, generated_at: &str) -> ArchivePaths {
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

pub(crate) fn render_markdown_report(report: &ReleaseReadinessReport) -> String {
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
