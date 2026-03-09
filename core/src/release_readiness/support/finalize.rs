use super::super::ReleaseReadinessReport;

pub(crate) fn finalize_release_readiness(
    mut report: ReleaseReadinessReport,
) -> ReleaseReadinessReport {
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
