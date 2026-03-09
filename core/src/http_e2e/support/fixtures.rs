use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use super::super::{HttpE2EReport, HttpE2EStepResult};
use super::reporting::{archive_slug, render_markdown_report};

use crate::recommendation::AutomationProposal;

pub(in crate::http_e2e) fn seed_live_e2e_recommendation() -> Result<i64> {
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

pub(in crate::http_e2e) fn seed_release_readiness_fixture(
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
