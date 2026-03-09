mod candidates;
mod evaluation;
mod seeding;
mod support;
#[cfg(test)]
mod tests;
mod types;

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use evaluation::run_scenario;
use support::{load_launch_eval_config, render_markdown_report, ScopedEnvGuard};

pub(crate) use candidates::{
    candidate_id, candidate_scope, execution_memory_confidence, execution_memory_score,
    execution_request_message, launch_event_score, normalize_request, parse_json_object,
    render_scenario_yaml, request_memory_params, request_memory_score, scenario_id, slug_id,
    summarize_message,
};
pub use candidates::{
    generate_launch_eval_candidates, generate_launch_eval_candidates_with_filter,
    read_launch_eval_candidate_snapshot_info, read_launch_eval_candidate_snapshot_info_with_filter,
    write_launch_eval_candidate_snapshot, write_launch_eval_candidate_snapshot_with_filter,
};
pub(crate) use types::LaunchEvalScenarioSnippet;
pub use types::*;

pub async fn run_launch_eval_from_path(config_path: &Path) -> Result<LaunchEvalReport> {
    let config = load_launch_eval_config(config_path)?;
    run_launch_eval(config, Some(config_path)).await
}

pub async fn run_launch_eval(
    config: LaunchEvalConfig,
    config_path: Option<&Path>,
) -> Result<LaunchEvalReport> {
    crate::load_env_with_fallback();

    let report_dir = PathBuf::from(config.report_dir.trim());
    fs::create_dir_all(&report_dir)
        .with_context(|| format!("failed to create report dir {}", report_dir.display()))?;

    let db_path =
        std::env::temp_dir().join(format!("allvia-launch-eval-{}.db", uuid::Uuid::new_v4()));
    let json_path = report_dir.join("latest.json");
    let markdown_path = report_dir.join("latest.md");

    let report = {
        let _env_guard = ScopedEnvGuard::set(vec![
            ("STEER_DB_PATH", Some(db_path.to_string_lossy().to_string())),
            ("CHAT_GATE_ENABLED", Some("0".to_string())),
            (
                "ALLVIA_ENABLE_RECOMMENDATION_CATEGORY_BROWSING",
                Some("0".to_string()),
            ),
            (
                "ALLVIA_AUTO_APPROVAL_MIN_BUSINESS_SCORE",
                Some("0.68".to_string()),
            ),
            (
                "ALLVIA_AUTO_APPROVAL_MIN_CONFIDENCE",
                Some("0.80".to_string()),
            ),
            (
                "ALLVIA_AUTO_APPROVAL_MIN_DISTINCT_DAYS",
                Some("3".to_string()),
            ),
            (
                "ALLVIA_AUTO_APPROVAL_MIN_OCCURRENCES",
                Some("4".to_string()),
            ),
            ("ALLVIA_WORK_PATTERN_CONTEXT_MIN", Some("0.6".to_string())),
            ("ALLVIA_AUTO_PENDING_WORK_LIMIT", Some("5".to_string())),
            ("ALLVIA_AUTO_PENDING_OTHER_LIMIT", Some("0".to_string())),
            (
                "ALLVIA_PENDING_RECOMMENDATION_DISPLAY_LIMIT",
                Some("5".to_string()),
            ),
            (
                "ALLVIA_RESPONSE_CACHE_TTL_GMAIL_LIST",
                Some("20".to_string()),
            ),
            (
                "ALLVIA_RESPONSE_CACHE_TTL_CALENDAR_TODAY",
                Some("30".to_string()),
            ),
            (
                "ALLVIA_RESPONSE_CACHE_TTL_CALENDAR_WEEK",
                Some("120".to_string()),
            ),
            (
                "ALLVIA_RESPONSE_CACHE_TTL_SYSTEM_STATUS",
                Some("10".to_string()),
            ),
            (
                "ALLVIA_REPEAT_REQUEST_FRESH_WINDOW_SECONDS",
                Some("15".to_string()),
            ),
        ]);

        crate::db::reset_connection();
        crate::db::init().context("failed to initialize launch eval DB")?;

        let mut results = Vec::new();
        for scenario in config.scenarios {
            let result = run_scenario(&scenario).await;
            results.push(result);
        }

        let total = results.len();
        let passed = results.iter().filter(|result| result.passed).count();
        let failed = total.saturating_sub(passed);

        LaunchEvalReport {
            generated_at: chrono::Utc::now().to_rfc3339(),
            config_path: config_path.map(|path| path.display().to_string()),
            db_path: db_path.display().to_string(),
            report_json_path: json_path.display().to_string(),
            report_markdown_path: markdown_path.display().to_string(),
            total,
            passed,
            failed,
            results,
        }
    };
    crate::db::reset_connection();

    fs::write(
        &json_path,
        serde_json::to_string_pretty(&report).context("failed to serialize launch eval JSON")?,
    )
    .with_context(|| format!("failed to write {}", json_path.display()))?;
    fs::write(&markdown_path, render_markdown_report(&report))
        .with_context(|| format!("failed to write {}", markdown_path.display()))?;

    Ok(report)
}
