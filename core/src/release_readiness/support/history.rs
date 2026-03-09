use super::super::{
    ReleaseReadinessHistoryEntry, ReleaseReadinessReport, ReleaseReadinessTrendSummary,
};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

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

pub(crate) fn build_history_trend_summary(
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
        && !current
            .http_e2e
            .as_ref()
            .map(|value| value.ok)
            .unwrap_or(false);

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
