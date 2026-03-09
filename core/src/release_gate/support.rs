use super::{LaunchEvalSummary, ReleaseBaseline};
use crate::{
    consistency_check, db, launch_eval, performance_verification, quality_scorer,
    semantic_verification,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn maybe_refresh_launch_eval_candidate_snapshot(
    workdir: &Path,
    refresh_flag: Option<bool>,
    limit: Option<usize>,
    output_path: Option<&str>,
) -> anyhow::Result<()> {
    if !refresh_flag.unwrap_or(false) {
        return Ok(());
    }

    launch_eval::write_launch_eval_candidate_snapshot(
        workdir,
        output_path,
        limit.unwrap_or(20).clamp(1, 50),
    )?;
    Ok(())
}

pub(super) fn count_severity(
    issues: &[semantic_verification::SemanticIssue],
    severity: &str,
) -> usize {
    let target = severity.to_lowercase();
    issues
        .iter()
        .filter(|issue| issue.severity.to_lowercase() == target)
        .count()
}

pub(super) fn resolve_workdir(workdir: Option<&str>) -> PathBuf {
    workdir
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

pub(super) fn load_launch_eval_summary(
    workdir: &Path,
    config_path: Option<&str>,
    report_path: Option<&str>,
) -> Option<LaunchEvalSummary> {
    let report_path = resolve_launch_eval_report_path(workdir, config_path, report_path)?;
    let raw = fs::read_to_string(&report_path).ok()?;
    let report: launch_eval::LaunchEvalReport = serde_json::from_str(&raw).ok()?;
    let failed_case_ids = report
        .results
        .iter()
        .filter(|case| !case.passed)
        .map(|case| case.id.clone())
        .collect::<Vec<_>>();

    Some(LaunchEvalSummary {
        generated_at: report.generated_at,
        config_path: report.config_path,
        total: report.total,
        passed: report.passed,
        failed: report.failed,
        failed_case_ids,
    })
}

fn resolve_launch_eval_report_path(
    workdir: &Path,
    config_path: Option<&str>,
    report_path: Option<&str>,
) -> Option<PathBuf> {
    if let Some(path) = report_path {
        let candidate = resolve_relative_to_workdir(workdir, path);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let config_candidate = config_path
        .map(|path| resolve_relative_to_workdir(workdir, path))
        .or_else(|| {
            let default = workdir.join("configs/launch_eval.yaml");
            default.exists().then_some(default)
        });

    if let Some(config_candidate) = config_candidate {
        let raw = fs::read_to_string(&config_candidate).ok()?;
        let config: launch_eval::LaunchEvalConfig = serde_yaml::from_str(&raw).ok()?;
        let report_dir = resolve_relative_to_workdir(workdir, &config.report_dir);
        let latest = report_dir.join("latest.json");
        if latest.exists() {
            return Some(latest);
        }
    }

    let fallback = workdir.join("reports/launch_eval/latest.json");
    fallback.exists().then_some(fallback)
}

fn resolve_relative_to_workdir(workdir: &Path, path: &str) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        candidate
    } else {
        workdir.join(candidate)
    }
}

pub(super) fn launch_eval_age_hours(generated_at: &str) -> Option<i64> {
    let parsed = chrono::DateTime::parse_from_rfc3339(generated_at).ok()?;
    Some(
        chrono::Utc::now()
            .signed_duration_since(parsed.with_timezone(&chrono::Utc))
            .num_hours(),
    )
}

pub(super) fn env_f64(key: &str, default_val: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default_val)
}

pub(super) fn env_i64(key: &str, default_val: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(default_val)
}

pub(super) fn env_usize(key: &str, default_val: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default_val)
}

pub(super) fn ratio(value: i64, total: i64) -> f64 {
    if total <= 0 {
        0.0
    } else {
        value as f64 / total as f64
    }
}

pub(super) fn percentage(value: i64, total: i64) -> f64 {
    ratio(value, total) * 100.0
}

fn parse_rfc3339_utc(value: Option<&str>) -> Option<chrono::DateTime<chrono::Utc>> {
    value.and_then(|raw| {
        chrono::DateTime::parse_from_rfc3339(raw.trim())
            .ok()
            .map(|ts| ts.with_timezone(&chrono::Utc))
    })
}

fn timestamp_is_recent(value: Option<&str>, max_age_days: i64) -> bool {
    let cutoff = chrono::Utc::now() - chrono::Duration::days(max_age_days.max(1));
    parse_rfc3339_utc(value).is_some_and(|ts| ts >= cutoff)
}

pub(super) fn normalize_release_exec_approval_metrics(
    mut metrics: db::ExecApprovalMetrics,
) -> db::ExecApprovalMetrics {
    let recent_days = env_i64("RELEASE_EXEC_APPROVAL_ACTIVITY_MAX_AGE_DAYS", 7);
    let has_recent_activity = timestamp_is_recent(metrics.last_created_at.as_deref(), recent_days)
        || timestamp_is_recent(metrics.last_resolved_at.as_deref(), recent_days);
    if has_recent_activity {
        return metrics;
    }

    metrics.total = 0;
    metrics.pending = 0;
    metrics.expired_pending = 0;
    metrics.approved = 0;
    metrics.rejected = 0;
    metrics.allow_once = 0;
    metrics.allow_always = 0;
    metrics.deny = 0;
    metrics.approval_rate = 0.0;
    metrics.oldest_pending_created_at = None;
    metrics.last_created_at = None;
    metrics.last_resolved_at = None;
    metrics
}

fn recommendation_effective_category(rec: &db::Recommendation) -> String {
    let derived = crate::recommendation_policy::classify_recommendation_record(rec);
    if rec.category.trim().is_empty()
        || rec
            .category
            .eq_ignore_ascii_case(crate::recommendation_policy::CATEGORY_UNKNOWN)
    {
        derived.category
    } else {
        rec.category.clone()
    }
}

fn recommendation_is_snoozed(rec: &db::Recommendation) -> bool {
    parse_rfc3339_utc(
        rec.snoozed_until
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    )
    .is_some_and(|ts| ts > chrono::Utc::now())
}

fn recommendation_effective_status(rec: &db::Recommendation) -> String {
    if rec.status.eq_ignore_ascii_case("pending")
        && rec
            .last_error
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
    {
        "failed".to_string()
    } else if rec.status.eq_ignore_ascii_case("pending") && recommendation_is_snoozed(rec) {
        "later".to_string()
    } else {
        rec.status.clone()
    }
}

fn limit_visible_work_recommendations(
    recs: Vec<db::Recommendation>,
    preference_history: &[db::Recommendation],
) -> Vec<db::Recommendation> {
    use std::cmp::Ordering;

    let limit = crate::recommendation_policy::pending_recommendation_display_limit();
    let work_filter = crate::recommendation_policy::normalize_list_category_filter(None);
    let preference_profile =
        crate::recommendation_policy::build_recommendation_preference_profile(preference_history);

    if limit == 0 {
        return recs
            .into_iter()
            .filter(|rec| {
                crate::recommendation_policy::matches_category_filter(
                    &recommendation_effective_category(rec),
                    work_filter.as_deref(),
                )
            })
            .collect();
    }

    let mut manual_pending = Vec::new();
    let mut auto_pending = Vec::new();
    let mut others = Vec::new();

    for rec in recs {
        if !crate::recommendation_policy::matches_category_filter(
            &recommendation_effective_category(&rec),
            work_filter.as_deref(),
        ) {
            continue;
        }

        if recommendation_effective_status(&rec).eq_ignore_ascii_case("pending") {
            if rec.pattern_id.is_some() {
                let readiness =
                    crate::recommendation_policy::evaluate_recommendation_approval_readiness(&rec);
                if !readiness.ready {
                    continue;
                }
                auto_pending.push(rec);
            } else {
                manual_pending.push(rec);
            }
        } else {
            others.push(rec);
        }
    }

    auto_pending.sort_by(|left, right| {
        crate::recommendation_policy::recommendation_record_priority_score_with_profile(
            right,
            &preference_profile,
        )
        .partial_cmp(
            &crate::recommendation_policy::recommendation_record_priority_score_with_profile(
                left,
                &preference_profile,
            ),
        )
        .unwrap_or(Ordering::Equal)
    });

    let mut visible = manual_pending;
    visible.extend(auto_pending.into_iter().take(limit));
    visible.extend(others);
    visible
}

pub(super) fn visible_auto_work_pending_count() -> Option<i64> {
    let recs = db::get_recommendations_with_filter(Some("all")).ok()?;
    let history = db::get_recent_recommendations(
        crate::recommendation_policy::auto_recommendation_history_limit(),
    )
    .ok()?;
    let visible = limit_visible_work_recommendations(recs, &history);
    Some(
        visible
            .iter()
            .filter(|rec| {
                rec.pattern_id.is_some()
                    && recommendation_effective_status(rec).eq_ignore_ascii_case("pending")
            })
            .count() as i64,
    )
}

pub(super) fn derive_release_quality_score(
    consistency: Option<&consistency_check::ConsistencyCheckResult>,
    semantic: Option<&semantic_verification::SemanticVerificationResult>,
    performance: Option<&performance_verification::PerformanceVerificationResult>,
    launch_ops: Option<&db::LaunchOpsMetrics>,
    launch_eval: Option<&LaunchEvalSummary>,
) -> quality_scorer::QualityScore {
    let mut breakdown = HashMap::new();
    let mut issues = Vec::new();
    let mut strengths = Vec::new();

    let functionality = if let Some(eval) = launch_eval {
        if eval.total > 0 {
            let pass_rate = eval.passed as f64 / eval.total as f64;
            if eval.failed == 0 {
                strengths.push(format!(
                    "Launch eval passed ({}/{})",
                    eval.passed, eval.total
                ));
            } else {
                issues.push(format!(
                    "Launch eval has failures ({}/{})",
                    eval.failed, eval.total
                ));
            }
            (pass_rate * 3.0).clamp(0.0, 3.0)
        } else {
            issues.push("Launch eval did not run".to_string());
            1.0
        }
    } else {
        issues.push("Launch eval summary missing".to_string());
        1.0
    };
    breakdown.insert("functionality".to_string(), round2(functionality));

    let mut ui_ux: f64 = 1.5;
    if let Some(check) = consistency {
        if check.ok {
            ui_ux = 2.6;
            strengths.push("API/frontend route consistency passed".to_string());
        } else {
            ui_ux = 1.0;
            issues.push(format!(
                "Consistency check reports {} mismatches",
                check.issues.len()
            ));
        }
    } else {
        issues.push("Consistency check missing".to_string());
    }
    if let Some(ops) = launch_ops {
        if ops.total_requests > 0 && ops.low_confidence_routes == 0 {
            ui_ux += 0.4;
            strengths.push("No low-confidence launch ops routes".to_string());
        }
    }
    ui_ux = ui_ux.clamp(0.0, 3.0);
    breakdown.insert("ui_ux".to_string(), round2(ui_ux));

    let mut code_quality: f64 = 0.7;
    if let Some(sem) = semantic {
        if sem.ok {
            code_quality += 0.9;
            strengths.push("Semantic verification passed".to_string());
        } else {
            let high_count = sem
                .issues
                .iter()
                .filter(|issue| issue.severity.eq_ignore_ascii_case("high"))
                .count();
            issues.push(format!(
                "Semantic verification found {} issue(s) ({} high)",
                sem.issues.len(),
                high_count
            ));
            code_quality -= (high_count as f64 * 0.2).min(0.6);
        }
    } else {
        issues.push("Semantic verification missing".to_string());
    }
    if let Some(perf) = performance {
        if perf.ok {
            code_quality += 0.4;
            strengths.push("Performance baseline within thresholds".to_string());
        } else {
            issues.push("Performance verification failed".to_string());
        }
    } else {
        issues.push("Performance verification missing".to_string());
    }
    code_quality = code_quality.clamp(0.0, 2.0);
    breakdown.insert("code_quality".to_string(), round2(code_quality));

    let mut api_compatibility: f64 = 0.8;
    if let Some(check) = consistency {
        if check.ok {
            api_compatibility += 0.7;
        } else {
            api_compatibility -= 0.2;
        }
    }
    if let Some(ops) = launch_ops {
        if ops.total_requests > 0 {
            let error_rate = ratio(ops.error_routes, ops.total_requests);
            if error_rate == 0.0 {
                api_compatibility += 0.5;
                strengths.push("Launch ops error rate is 0%".to_string());
            } else {
                issues.push(format!(
                    "Launch ops error rate is {:.1}%",
                    error_rate * 100.0
                ));
                api_compatibility -= (error_rate * 2.0).min(0.6);
            }
        }
    }
    api_compatibility = api_compatibility.clamp(0.0, 2.0);
    breakdown.insert("api_compatibility".to_string(), round2(api_compatibility));

    issues.sort();
    issues.dedup();
    strengths.sort();
    strengths.dedup();

    let overall = round2(functionality + ui_ux + code_quality + api_compatibility);
    let recommendation = if overall < 5.0 {
        "replanning"
    } else if overall < 8.0 {
        "fix"
    } else {
        "done"
    };
    let summary = if issues.is_empty() {
        "Release quality checks passed".to_string()
    } else {
        format!(
            "Release quality issues: {}",
            issues
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join("; ")
        )
    };

    quality_scorer::QualityScore {
        overall,
        breakdown,
        issues,
        strengths,
        recommendation: recommendation.to_string(),
        summary,
    }
}

fn round2(val: f64) -> f64 {
    (val * 100.0).round() / 100.0
}

#[allow(dead_code)]
pub fn load_baseline_from_path(path: &Path) -> Option<ReleaseBaseline> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}
