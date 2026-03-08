use crate::{
    consistency_check, db, launch_eval, performance_verification, quality_scorer,
    semantic_verification,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalSummary {
    pub generated_at: String,
    pub config_path: Option<String>,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub failed_case_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseBaseline {
    pub created_at: String,
    pub consistency: Option<consistency_check::ConsistencyCheckResult>,
    pub semantic: Option<semantic_verification::SemanticVerificationResult>,
    pub performance: Option<performance_verification::PerformanceVerificationResult>,
    pub quality: Option<quality_scorer::QualityScore>,
    pub launch_ops: Option<db::LaunchOpsMetrics>,
    pub nl_run_metrics: Option<db::NLRunMetrics>,
    pub exec_approval_metrics: Option<db::ExecApprovalMetrics>,
    pub recommendation_metrics: Option<db::RecommendationMetrics>,
    pub recommendation_review_metrics: Option<db::RecommendationReviewMetrics>,
    pub launch_eval: Option<LaunchEvalSummary>,
    pub launch_eval_candidate_snapshot: Option<launch_eval::LaunchEvalCandidateSnapshotInfo>,
    pub launch_eval_candidate_snapshot_refresh_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseGateResult {
    pub ok: bool,
    pub regressions: Vec<String>,
    pub warnings: Vec<String>,
    pub baseline: Option<ReleaseBaseline>,
    pub current: ReleaseBaseline,
    pub template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseGateRequest {
    pub workdir: Option<String>,
    pub max_files: Option<usize>,
    pub consistency: Option<consistency_check::ConsistencyCheckResult>,
    pub semantic: Option<semantic_verification::SemanticVerificationResult>,
    pub performance: Option<performance_verification::PerformanceVerificationResult>,
    pub quality: Option<quality_scorer::QualityScore>,
    pub perf_regression_pct: Option<f64>,
    pub quality_drop: Option<f64>,
    pub launch_error_rate_pct: Option<f64>,
    pub launch_low_confidence_rate_pct: Option<f64>,
    pub launch_cache_hit_rate_drop_pct: Option<f64>,
    pub recommendation_approval_rate_min: Option<f64>,
    pub launch_eval_config_path: Option<String>,
    pub launch_eval_report_path: Option<String>,
    pub refresh_launch_eval_candidates: Option<bool>,
    pub launch_eval_candidate_limit: Option<usize>,
    pub launch_eval_snapshot_output_path: Option<String>,
}

pub type ReleaseBaselineRequest = ReleaseGateRequest;

pub fn build_baseline(req: ReleaseBaselineRequest) -> ReleaseBaseline {
    let workdir = resolve_workdir(req.workdir.as_deref());
    let semantic_max = req.max_files.unwrap_or(200);
    let performance_max = req.max_files.unwrap_or(300);

    let semantic = req.semantic.or_else(|| {
        Some(semantic_verification::semantic_consistency(
            &workdir,
            semantic_max,
        ))
    });
    let performance = req.performance.or_else(|| {
        Some(performance_verification::performance_baseline(
            &workdir,
            performance_max,
        ))
    });
    let consistency = req.consistency.or_else(|| {
        Some(consistency_check::run_consistency_check(
            consistency_check::ConsistencyCheckRequest {
                workdir: Some(workdir.to_string_lossy().to_string()),
            },
        ))
    });
    let launch_ops = db::get_launch_ops_metrics(200).ok();
    let launch_eval_candidate_snapshot_refresh_error =
        maybe_refresh_launch_eval_candidate_snapshot(
            &workdir,
            req.refresh_launch_eval_candidates,
            req.launch_eval_candidate_limit,
            req.launch_eval_snapshot_output_path.as_deref(),
        )
        .err()
        .map(|error| error.to_string());
    let launch_eval = load_launch_eval_summary(
        &workdir,
        req.launch_eval_config_path.as_deref(),
        req.launch_eval_report_path.as_deref(),
    );
    let launch_eval_candidate_snapshot =
        Some(launch_eval::read_launch_eval_candidate_snapshot_info(
            &workdir,
            req.launch_eval_snapshot_output_path.as_deref(),
        ));
    let quality = req.quality.or_else(|| {
        Some(derive_release_quality_score(
            consistency.as_ref(),
            semantic.as_ref(),
            performance.as_ref(),
            launch_ops.as_ref(),
            launch_eval.as_ref(),
        ))
    });
    let nl_run_metrics = db::get_release_nl_run_metrics(200).ok();
    let exec_approval_metrics = db::get_exec_approval_metrics(200)
        .ok()
        .map(normalize_release_exec_approval_metrics);
    let recommendation_metrics = db::get_recommendation_metrics().ok().map(|mut metrics| {
        metrics.pending = visible_auto_work_pending_count().unwrap_or(metrics.pending);
        metrics
    });
    let recommendation_review_metrics = db::get_recommendation_review_metrics(100).ok();

    ReleaseBaseline {
        created_at: chrono::Utc::now().to_rfc3339(),
        consistency,
        semantic,
        performance,
        quality,
        launch_ops,
        nl_run_metrics,
        exec_approval_metrics,
        recommendation_metrics,
        recommendation_review_metrics,
        launch_eval,
        launch_eval_candidate_snapshot,
        launch_eval_candidate_snapshot_refresh_error,
    }
}

pub fn save_baseline(baseline: &ReleaseBaseline) {
    if let Ok(json) = serde_json::to_string(baseline) {
        let _ = db::upsert_release_baseline_json(&baseline.created_at, &json);
    }
}

pub fn run_release_gate(req: ReleaseGateRequest) -> ReleaseGateResult {
    let perf_override = req.perf_regression_pct;
    let quality_override = req.quality_drop;
    let launch_error_override = req.launch_error_rate_pct;
    let launch_low_conf_override = req.launch_low_confidence_rate_pct;
    let launch_cache_drop_override = req.launch_cache_hit_rate_drop_pct;
    let recommendation_approval_min = req.recommendation_approval_rate_min;
    let current = build_baseline(req);
    let baseline = load_baseline();
    evaluate_release_gate(
        current,
        baseline,
        perf_override,
        quality_override,
        launch_error_override,
        launch_low_conf_override,
        launch_cache_drop_override,
        recommendation_approval_min,
    )
}

fn load_baseline() -> Option<ReleaseBaseline> {
    db::get_release_baseline_json()
        .ok()
        .and_then(|record| record)
        .and_then(|record| serde_json::from_str::<ReleaseBaseline>(&record.baseline_json).ok())
}

fn evaluate_release_gate(
    current: ReleaseBaseline,
    baseline: Option<ReleaseBaseline>,
    perf_override: Option<f64>,
    quality_override: Option<f64>,
    launch_error_override: Option<f64>,
    launch_low_conf_override: Option<f64>,
    launch_cache_drop_override: Option<f64>,
    recommendation_approval_min: Option<f64>,
) -> ReleaseGateResult {
    let mut regressions = Vec::new();
    let mut warnings = Vec::new();

    let Some(base) = baseline.clone() else {
        warnings.push("No release baseline stored".to_string());
        return ReleaseGateResult {
            ok: true,
            regressions,
            warnings,
            baseline: None,
            current,
            template: "release_gate".to_string(),
        };
    };

    compare_semantic(&base, &current, &mut regressions, &mut warnings);
    compare_performance(
        &base,
        &current,
        &mut regressions,
        &mut warnings,
        perf_override,
    );
    compare_quality(
        &base,
        &current,
        &mut regressions,
        &mut warnings,
        quality_override,
    );
    compare_consistency(&base, &current, &mut regressions, &mut warnings);
    compare_launch_ops(
        &base,
        &current,
        &mut regressions,
        &mut warnings,
        launch_error_override,
        launch_low_conf_override,
        launch_cache_drop_override,
    );
    compare_execution_safety(&base, &current, &mut regressions, &mut warnings);
    compare_recommendation_health(
        &base,
        &current,
        &mut regressions,
        &mut warnings,
        recommendation_approval_min,
    );
    compare_recommendation_review_health(&base, &current, &mut warnings);
    compare_launch_eval(&base, &current, &mut regressions, &mut warnings);
    compare_launch_eval_candidate_snapshot(&base, &current, &mut regressions, &mut warnings);

    ReleaseGateResult {
        ok: regressions.is_empty(),
        regressions,
        warnings,
        baseline: Some(base),
        current,
        template: "release_gate".to_string(),
    }
}

fn compare_semantic(
    baseline: &ReleaseBaseline,
    current: &ReleaseBaseline,
    regressions: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let Some(base_sem) = &baseline.semantic else {
        warnings.push("Baseline semantic check missing".to_string());
        return;
    };
    let Some(cur_sem) = &current.semantic else {
        warnings.push("Current semantic check missing".to_string());
        return;
    };

    if base_sem.ok && !cur_sem.ok {
        regressions.push("Semantic verification regressed from ok to failing".to_string());
    }
    if cur_sem.issues.len() > base_sem.issues.len() {
        regressions.push(format!(
            "Semantic issues increased ({} -> {})",
            base_sem.issues.len(),
            cur_sem.issues.len()
        ));
    }

    let base_high = count_severity(&base_sem.issues, "high");
    let cur_high = count_severity(&cur_sem.issues, "high");
    if cur_high > base_high {
        regressions.push(format!(
            "High severity semantic issues increased ({} -> {})",
            base_high, cur_high
        ));
    }
}

fn compare_performance(
    baseline: &ReleaseBaseline,
    current: &ReleaseBaseline,
    regressions: &mut Vec<String>,
    warnings: &mut Vec<String>,
    perf_override: Option<f64>,
) {
    let Some(base_perf) = &baseline.performance else {
        warnings.push("Baseline performance check missing".to_string());
        return;
    };
    let Some(cur_perf) = &current.performance else {
        warnings.push("Current performance check missing".to_string());
        return;
    };

    if base_perf.ok && !cur_perf.ok {
        regressions.push("Performance verification regressed from ok to failing".to_string());
    }

    let delta = perf_override.unwrap_or_else(|| env_f64("RELEASE_PERF_REGRESSION_PCT", 0.1));
    for base_metric in &base_perf.metrics {
        let Some(cur_metric) = cur_perf.metrics.iter().find(|m| m.name == base_metric.name) else {
            warnings.push(format!(
                "Current performance metric missing: {}",
                base_metric.name
            ));
            continue;
        };
        if base_metric.value > 0.0 {
            let allowed = base_metric.value * (1.0 + delta);
            if cur_metric.value > allowed {
                regressions.push(format!(
                    "Performance metric {} regressed ({} -> {})",
                    base_metric.name, base_metric.value, cur_metric.value
                ));
            }
        } else if cur_metric.value > 0.0 {
            regressions.push(format!(
                "Performance metric {} increased from baseline 0 to {}",
                base_metric.name, cur_metric.value
            ));
        }
    }
}

fn compare_quality(
    baseline: &ReleaseBaseline,
    current: &ReleaseBaseline,
    regressions: &mut Vec<String>,
    warnings: &mut Vec<String>,
    quality_override: Option<f64>,
) {
    let Some(base_quality) = &baseline.quality else {
        warnings.push("Baseline quality score missing".to_string());
        return;
    };
    let Some(cur_quality) = &current.quality else {
        warnings.push("Current quality score missing".to_string());
        return;
    };

    let drop = quality_override.unwrap_or_else(|| env_f64("RELEASE_QUALITY_DROP", 0.3));
    if cur_quality.overall + drop < base_quality.overall {
        regressions.push(format!(
            "Quality score dropped ({} -> {})",
            base_quality.overall, cur_quality.overall
        ));
    }
}

fn compare_consistency(
    baseline: &ReleaseBaseline,
    current: &ReleaseBaseline,
    regressions: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let Some(base_consistency) = &baseline.consistency else {
        warnings.push("Baseline consistency check missing".to_string());
        return;
    };
    let Some(cur_consistency) = &current.consistency else {
        warnings.push("Current consistency check missing".to_string());
        return;
    };

    if base_consistency.ok && !cur_consistency.ok {
        regressions.push("API consistency regressed from ok to failing".to_string());
    }
    if cur_consistency.issues.len() > base_consistency.issues.len() {
        regressions.push(format!(
            "API consistency issues increased ({} -> {})",
            base_consistency.issues.len(),
            cur_consistency.issues.len()
        ));
    }
}

fn compare_launch_ops(
    baseline: &ReleaseBaseline,
    current: &ReleaseBaseline,
    regressions: &mut Vec<String>,
    warnings: &mut Vec<String>,
    launch_error_override: Option<f64>,
    launch_low_conf_override: Option<f64>,
    launch_cache_drop_override: Option<f64>,
) {
    let Some(cur_ops) = &current.launch_ops else {
        warnings.push("Current launch ops metrics missing".to_string());
        return;
    };
    if cur_ops.total_requests <= 0 {
        warnings.push("Current launch ops sample is empty".to_string());
        return;
    }

    let min_sample = env_i64("RELEASE_LAUNCH_MIN_SAMPLE", 20);
    if cur_ops.total_requests < min_sample {
        warnings.push(format!(
            "Launch ops sample is small ({} < {})",
            cur_ops.total_requests, min_sample
        ));
        return;
    }

    let error_rate = ratio(cur_ops.error_routes, cur_ops.total_requests);
    let low_conf_rate = ratio(cur_ops.low_confidence_routes, cur_ops.total_requests);
    let blocked_rate = ratio(cur_ops.blocked_requests, cur_ops.total_requests);

    let max_error_rate =
        launch_error_override.unwrap_or_else(|| env_f64("RELEASE_LAUNCH_ERROR_RATE_PCT", 0.05));
    let max_low_conf_rate = launch_low_conf_override
        .unwrap_or_else(|| env_f64("RELEASE_LAUNCH_LOW_CONFIDENCE_RATE_PCT", 0.12));
    let max_blocked_rate = env_f64("RELEASE_LAUNCH_BLOCKED_RATE_PCT", 0.15);

    if error_rate > max_error_rate {
        regressions.push(format!(
            "Launch ops error rate too high ({:.1}% > {:.1}%)",
            error_rate * 100.0,
            max_error_rate * 100.0
        ));
    }
    if low_conf_rate > max_low_conf_rate {
        regressions.push(format!(
            "Launch ops low-confidence rate too high ({:.1}% > {:.1}%)",
            low_conf_rate * 100.0,
            max_low_conf_rate * 100.0
        ));
    }
    if blocked_rate > max_blocked_rate {
        warnings.push(format!(
            "Launch ops blocked-rate is elevated ({:.1}% > {:.1}%)",
            blocked_rate * 100.0,
            max_blocked_rate * 100.0
        ));
    }

    let Some(base_ops) = &baseline.launch_ops else {
        warnings.push("Baseline launch ops metrics missing".to_string());
        return;
    };
    if base_ops.total_requests < min_sample {
        warnings.push(format!(
            "Baseline launch ops sample is small ({} < {})",
            base_ops.total_requests, min_sample
        ));
        return;
    }

    let cache_drop_allowed = launch_cache_drop_override
        .unwrap_or_else(|| env_f64("RELEASE_LAUNCH_CACHE_HIT_RATE_DROP_PCT", 10.0));
    let cache_drop = base_ops.cached_response_hit_rate - cur_ops.cached_response_hit_rate;
    if cache_drop > cache_drop_allowed {
        regressions.push(format!(
            "Launch ops cache hit rate dropped ({:.1}% -> {:.1}%)",
            base_ops.cached_response_hit_rate, cur_ops.cached_response_hit_rate
        ));
    }

    let base_error_rate = ratio(base_ops.error_routes, base_ops.total_requests);
    let allowed_error_rate_increase = env_f64("RELEASE_LAUNCH_ERROR_RATE_INCREASE_PCT", 0.05);
    if error_rate > base_error_rate + allowed_error_rate_increase {
        regressions.push(format!(
            "Launch ops error rate regressed ({:.1}% -> {:.1}%)",
            base_error_rate * 100.0,
            error_rate * 100.0
        ));
    }
}

fn compare_execution_safety(
    baseline: &ReleaseBaseline,
    current: &ReleaseBaseline,
    regressions: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    if let Some(cur_runs) = &current.nl_run_metrics {
        if cur_runs.total <= 0 {
            warnings.push("Current NL run sample is empty".to_string());
        } else {
            let min_sample = env_i64("RELEASE_NL_MIN_SAMPLE", 10);
            if cur_runs.total < min_sample {
                warnings.push(format!(
                    "NL run sample is small ({} < {})",
                    cur_runs.total, min_sample
                ));
            } else {
                let error_rate = ratio(cur_runs.error, cur_runs.total);
                let blocked_rate = ratio(cur_runs.blocked, cur_runs.total);
                let manual_required_rate = ratio(cur_runs.manual_required, cur_runs.total);
                let approval_required_rate = ratio(cur_runs.approval_required, cur_runs.total);

                let min_success_rate = env_f64("RELEASE_NL_SUCCESS_RATE_MIN", 55.0);
                let max_error_rate = env_f64("RELEASE_NL_ERROR_RATE_PCT", 0.15);
                let max_blocked_rate = env_f64("RELEASE_NL_BLOCKED_RATE_PCT", 0.25);
                let max_manual_required_rate = env_f64("RELEASE_NL_MANUAL_REQUIRED_RATE_PCT", 0.35);
                let max_approval_required_rate =
                    env_f64("RELEASE_NL_APPROVAL_REQUIRED_RATE_PCT", 0.35);

                if cur_runs.success_rate < min_success_rate {
                    regressions.push(format!(
                        "NL run success rate too low ({:.1}% < {:.1}%)",
                        cur_runs.success_rate, min_success_rate
                    ));
                }
                if error_rate > max_error_rate {
                    regressions.push(format!(
                        "NL run error rate too high ({:.1}% > {:.1}%)",
                        error_rate * 100.0,
                        max_error_rate * 100.0
                    ));
                }
                if blocked_rate > max_blocked_rate {
                    warnings.push(format!(
                        "NL run blocked-rate is elevated ({:.1}% > {:.1}%)",
                        blocked_rate * 100.0,
                        max_blocked_rate * 100.0
                    ));
                }
                if manual_required_rate > max_manual_required_rate {
                    warnings.push(format!(
                        "NL run manual-required rate is elevated ({:.1}% > {:.1}%)",
                        manual_required_rate * 100.0,
                        max_manual_required_rate * 100.0
                    ));
                }
                if approval_required_rate > max_approval_required_rate {
                    warnings.push(format!(
                        "NL run approval-required rate is elevated ({:.1}% > {:.1}%)",
                        approval_required_rate * 100.0,
                        max_approval_required_rate * 100.0
                    ));
                }

                if let Some(base_runs) = &baseline.nl_run_metrics {
                    if base_runs.total >= min_sample {
                        let allowed_success_drop = env_f64("RELEASE_NL_SUCCESS_RATE_DROP", 12.0);
                        if base_runs.success_rate - cur_runs.success_rate > allowed_success_drop {
                            regressions.push(format!(
                                "NL run success rate regressed ({:.1}% -> {:.1}%)",
                                base_runs.success_rate, cur_runs.success_rate
                            ));
                        }
                        let base_error_rate = ratio(base_runs.error, base_runs.total);
                        let allowed_error_rate_increase =
                            env_f64("RELEASE_NL_ERROR_RATE_INCREASE_PCT", 0.10);
                        if error_rate > base_error_rate + allowed_error_rate_increase {
                            regressions.push(format!(
                                "NL run error rate regressed ({:.1}% -> {:.1}%)",
                                base_error_rate * 100.0,
                                error_rate * 100.0
                            ));
                        }
                    } else {
                        warnings.push(format!(
                            "Baseline NL run sample is small ({} < {})",
                            base_runs.total, min_sample
                        ));
                    }
                } else {
                    warnings.push("Baseline NL run metrics missing".to_string());
                }
            }
        }
    } else {
        warnings.push("Current NL run metrics missing".to_string());
    }

    if let Some(cur_approvals) = &current.exec_approval_metrics {
        if cur_approvals.total > 0 {
            let max_pending = env_i64("RELEASE_EXEC_APPROVAL_PENDING_MAX", 8);
            let max_expired_pending = env_i64("RELEASE_EXEC_APPROVAL_EXPIRED_MAX", 0);
            if cur_approvals.pending > max_pending {
                warnings.push(format!(
                    "Exec approval backlog is high ({} > {})",
                    cur_approvals.pending, max_pending
                ));
            }
            if cur_approvals.expired_pending > max_expired_pending {
                warnings.push(format!(
                    "Expired exec approvals detected ({} > {})",
                    cur_approvals.expired_pending, max_expired_pending
                ));
            }

            let resolved = cur_approvals.approved + cur_approvals.rejected;
            let min_resolved_sample = env_i64("RELEASE_EXEC_APPROVAL_MIN_RESOLVED_SAMPLE", 5);
            if resolved < min_resolved_sample {
                warnings.push(format!(
                    "Exec approval review sample is small ({} < {})",
                    resolved, min_resolved_sample
                ));
            } else {
                let min_approval_rate = env_f64("RELEASE_EXEC_APPROVAL_RATE_MIN", 40.0);
                if cur_approvals.approval_rate < min_approval_rate {
                    warnings.push(format!(
                        "Exec approval rate is low ({:.1}% < {:.1}%)",
                        cur_approvals.approval_rate, min_approval_rate
                    ));
                }

                if let Some(base_approvals) = &baseline.exec_approval_metrics {
                    let base_resolved = base_approvals.approved + base_approvals.rejected;
                    if base_resolved >= min_resolved_sample {
                        let allowed_drop = env_f64("RELEASE_EXEC_APPROVAL_RATE_DROP", 20.0);
                        if base_approvals.approval_rate - cur_approvals.approval_rate > allowed_drop
                        {
                            warnings.push(format!(
                                "Exec approval rate regressed ({:.1}% -> {:.1}%)",
                                base_approvals.approval_rate, cur_approvals.approval_rate
                            ));
                        }
                    } else {
                        warnings.push(format!(
                            "Baseline exec approval review sample is small ({} < {})",
                            base_resolved, min_resolved_sample
                        ));
                    }
                } else {
                    warnings.push("Baseline exec approval metrics missing".to_string());
                }
            }
        }
    } else {
        warnings.push("Current exec approval metrics missing".to_string());
    }
}

fn compare_recommendation_health(
    baseline: &ReleaseBaseline,
    current: &ReleaseBaseline,
    regressions: &mut Vec<String>,
    warnings: &mut Vec<String>,
    recommendation_approval_min: Option<f64>,
) {
    let Some(cur_metrics) = &current.recommendation_metrics else {
        warnings.push("Current recommendation metrics missing".to_string());
        return;
    };

    let review_count = cur_metrics.approved + cur_metrics.rejected;
    let min_reviews = env_i64("RELEASE_RECOMMENDATION_MIN_REVIEW_SAMPLE", 5);
    if review_count < min_reviews {
        warnings.push(format!(
            "Recommendation review sample is small ({} < {})",
            review_count, min_reviews
        ));
        return;
    }

    let approval_rate = percentage(cur_metrics.approved, review_count);
    let min_approval_rate = recommendation_approval_min
        .unwrap_or_else(|| env_f64("RELEASE_RECOMMENDATION_APPROVAL_RATE_MIN", 20.0));
    if approval_rate < min_approval_rate {
        regressions.push(format!(
            "Recommendation approval rate too low ({:.1}% < {:.1}%)",
            approval_rate, min_approval_rate
        ));
    }

    let max_pending = env_i64("RELEASE_RECOMMENDATION_PENDING_MAX", 8);
    if cur_metrics.pending > max_pending {
        warnings.push(format!(
            "Visible auto recommendation queue is high ({} > {})",
            cur_metrics.pending, max_pending
        ));
    }

    let Some(base_metrics) = &baseline.recommendation_metrics else {
        warnings.push("Baseline recommendation metrics missing".to_string());
        return;
    };
    let base_review_count = base_metrics.approved + base_metrics.rejected;
    if base_review_count < min_reviews {
        warnings.push(format!(
            "Baseline recommendation review sample is small ({} < {})",
            base_review_count, min_reviews
        ));
        return;
    }

    let base_approval_rate = percentage(base_metrics.approved, base_review_count);
    let allowed_drop = env_f64("RELEASE_RECOMMENDATION_APPROVAL_RATE_DROP", 15.0);
    if base_approval_rate - approval_rate > allowed_drop {
        regressions.push(format!(
            "Recommendation approval rate regressed ({:.1}% -> {:.1}%)",
            base_approval_rate, approval_rate
        ));
    }
}

fn compare_recommendation_review_health(
    baseline: &ReleaseBaseline,
    current: &ReleaseBaseline,
    warnings: &mut Vec<String>,
) {
    let Some(cur_metrics) = &current.recommendation_review_metrics else {
        warnings.push("Current recommendation review metrics missing".to_string());
        return;
    };

    let min_sample = env_i64("RELEASE_RECOMMENDATION_REVIEW_MIN_SAMPLE", 5);
    if cur_metrics.total_events == 0 {
        return;
    }
    if cur_metrics.total_events < min_sample {
        warnings.push(format!(
            "Recommendation review event sample is small ({} < {})",
            cur_metrics.total_events, min_sample
        ));
        return;
    }

    let max_action_failure_rate = env_f64(
        "RELEASE_RECOMMENDATION_REVIEW_ACTION_FAILURE_RATE_MAX",
        20.0,
    );
    if cur_metrics.action_failure_rate > max_action_failure_rate {
        warnings.push(format!(
            "Recommendation review action-failure rate is elevated ({:.1}% > {:.1}%)",
            cur_metrics.action_failure_rate, max_action_failure_rate
        ));
    }

    let max_non_positive_rate =
        env_f64("RELEASE_RECOMMENDATION_REVIEW_NON_POSITIVE_RATE_MAX", 70.0);
    if cur_metrics.non_positive_feedback_rate > max_non_positive_rate {
        warnings.push(format!(
            "Recommendation review non-positive feedback rate is elevated ({:.1}% > {:.1}%)",
            cur_metrics.non_positive_feedback_rate, max_non_positive_rate
        ));
    }

    let Some(base_metrics) = &baseline.recommendation_review_metrics else {
        warnings.push("Baseline recommendation review metrics missing".to_string());
        return;
    };
    if base_metrics.total_events < min_sample {
        warnings.push(format!(
            "Baseline recommendation review event sample is small ({} < {})",
            base_metrics.total_events, min_sample
        ));
        return;
    }

    let allowed_action_failure_increase = env_f64(
        "RELEASE_RECOMMENDATION_REVIEW_ACTION_FAILURE_RATE_INCREASE",
        15.0,
    );
    if cur_metrics.action_failure_rate - base_metrics.action_failure_rate
        > allowed_action_failure_increase
    {
        warnings.push(format!(
            "Recommendation review action-failure rate regressed ({:.1}% -> {:.1}%)",
            base_metrics.action_failure_rate, cur_metrics.action_failure_rate
        ));
    }

    let allowed_non_positive_increase = env_f64(
        "RELEASE_RECOMMENDATION_REVIEW_NON_POSITIVE_RATE_INCREASE",
        25.0,
    );
    if cur_metrics.non_positive_feedback_rate - base_metrics.non_positive_feedback_rate
        > allowed_non_positive_increase
    {
        warnings.push(format!(
            "Recommendation review non-positive feedback rate regressed ({:.1}% -> {:.1}%)",
            base_metrics.non_positive_feedback_rate, cur_metrics.non_positive_feedback_rate
        ));
    }
}

fn compare_launch_eval(
    baseline: &ReleaseBaseline,
    current: &ReleaseBaseline,
    regressions: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let Some(cur_eval) = &current.launch_eval else {
        warnings.push("Current launch eval report missing".to_string());
        return;
    };

    let max_age_hours = env_i64("RELEASE_LAUNCH_EVAL_MAX_AGE_HOURS", 24);
    if launch_eval_age_hours(&cur_eval.generated_at).is_some_and(|age| age > max_age_hours) {
        warnings.push(format!(
            "Launch eval report is stale (older than {}h)",
            max_age_hours
        ));
    }

    if cur_eval.failed > 0 {
        regressions.push(format!(
            "Launch eval has failing scenarios ({}/{})",
            cur_eval.failed, cur_eval.total
        ));
    }

    let Some(base_eval) = &baseline.launch_eval else {
        warnings.push("Baseline launch eval report missing".to_string());
        return;
    };

    if cur_eval.total < base_eval.total {
        warnings.push(format!(
            "Launch eval scenario coverage dropped ({} -> {})",
            base_eval.total, cur_eval.total
        ));
    }
    if cur_eval.failed > base_eval.failed {
        regressions.push(format!(
            "Launch eval failures increased ({} -> {})",
            base_eval.failed, cur_eval.failed
        ));
    }
}

fn compare_launch_eval_candidate_snapshot(
    baseline: &ReleaseBaseline,
    current: &ReleaseBaseline,
    regressions: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    if let Some(error) = &current.launch_eval_candidate_snapshot_refresh_error {
        warnings.push(format!(
            "Launch eval candidate snapshot refresh failed: {}",
            error
        ));
    }

    let Some(cur_snapshot) = &current.launch_eval_candidate_snapshot else {
        warnings.push("Current launch eval candidate snapshot missing".to_string());
        return;
    };

    if !cur_snapshot.exists {
        warnings.push("Launch eval candidate snapshot file is missing".to_string());
        return;
    }

    let min_scenarios = env_usize("RELEASE_LAUNCH_EVAL_CANDIDATE_MIN", 5);
    if cur_snapshot.scenario_count < min_scenarios {
        warnings.push(format!(
            "Launch eval candidate snapshot is thin ({} < {})",
            cur_snapshot.scenario_count, min_scenarios
        ));
    }

    let stale_hours = env_i64("RELEASE_LAUNCH_EVAL_CANDIDATE_STALE_HOURS", 72);
    if let Some(updated_at) = &cur_snapshot.updated_at {
        if let Ok(updated) = chrono::DateTime::parse_from_rfc3339(updated_at) {
            let age = chrono::Utc::now() - updated.with_timezone(&chrono::Utc);
            if age > chrono::Duration::hours(stale_hours) {
                warnings.push(format!(
                    "Launch eval candidate snapshot is stale (older than {}h)",
                    stale_hours
                ));
            }
        }
    } else {
        warnings.push("Launch eval candidate snapshot timestamp missing".to_string());
    }

    if let Some(base_snapshot) = &baseline.launch_eval_candidate_snapshot {
        if base_snapshot.exists && cur_snapshot.scenario_count + 3 < base_snapshot.scenario_count {
            regressions.push(format!(
                "Launch eval candidate coverage regressed ({} -> {})",
                base_snapshot.scenario_count, cur_snapshot.scenario_count
            ));
        }
    }
}

fn maybe_refresh_launch_eval_candidate_snapshot(
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

fn count_severity(issues: &[semantic_verification::SemanticIssue], severity: &str) -> usize {
    let target = severity.to_lowercase();
    issues
        .iter()
        .filter(|issue| issue.severity.to_lowercase() == target)
        .count()
}

fn resolve_workdir(workdir: Option<&str>) -> PathBuf {
    workdir
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

fn load_launch_eval_summary(
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

fn launch_eval_age_hours(generated_at: &str) -> Option<i64> {
    let parsed = chrono::DateTime::parse_from_rfc3339(generated_at).ok()?;
    Some(
        chrono::Utc::now()
            .signed_duration_since(parsed.with_timezone(&chrono::Utc))
            .num_hours(),
    )
}

fn env_f64(key: &str, default_val: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default_val)
}

fn env_i64(key: &str, default_val: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(default_val)
}

fn env_usize(key: &str, default_val: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default_val)
}

fn ratio(value: i64, total: i64) -> f64 {
    if total <= 0 {
        0.0
    } else {
        value as f64 / total as f64
    }
}

fn percentage(value: i64, total: i64) -> f64 {
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

fn normalize_release_exec_approval_metrics(
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

fn visible_auto_work_pending_count() -> Option<i64> {
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

fn derive_release_quality_score(
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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn release_baseline_with_ops(
        error_routes: i64,
        low_confidence_routes: i64,
        cached_response_hit_rate: f64,
        approved: i64,
        rejected: i64,
        pending: i64,
    ) -> ReleaseBaseline {
        ReleaseBaseline {
            created_at: "2026-03-07T00:00:00Z".to_string(),
            consistency: None,
            semantic: None,
            performance: None,
            quality: None,
            launch_ops: Some(db::LaunchOpsMetrics {
                window_size: 200,
                total_requests: 100,
                blocked_requests: 3,
                intent_memory_hits: 12,
                request_memory_hits: 20,
                execution_memory_hits: 22,
                cached_response_hit_rate,
                deterministic_routes: 18,
                llm_routes: 28,
                ai_digest_routes: 6,
                ai_digest_auto_routes: 4,
                local_routes: 12,
                freshness_bypasses: 5,
                low_confidence_routes,
                unknown_routes: 2,
                error_routes,
                last_event_at: Some("2026-03-07T00:00:00Z".to_string()),
                route_breakdown: vec![],
            }),
            nl_run_metrics: Some(db::NLRunMetrics {
                total: 40,
                completed: 24,
                manual_required: 6,
                approval_required: 5,
                blocked: 2,
                error: 3,
                success_rate: 60.0,
            }),
            exec_approval_metrics: Some(db::ExecApprovalMetrics {
                window_size: 100,
                total: 12,
                pending: 2,
                approved: 6,
                rejected: 2,
                expired_pending: 0,
                allow_once: 4,
                allow_always: 2,
                deny: 2,
                approval_rate: 75.0,
                oldest_pending_created_at: Some("2026-03-07T00:00:00Z".to_string()),
                last_created_at: Some("2026-03-07T00:00:00Z".to_string()),
                last_resolved_at: Some("2026-03-07T00:00:00Z".to_string()),
            }),
            recommendation_metrics: Some(db::RecommendationMetrics {
                total: approved + rejected + pending,
                approved,
                rejected,
                failed: 0,
                pending,
                later: 0,
                legacy_other: 0,
                last_created_at: Some("2026-03-07T00:00:00Z".to_string()),
            }),
            recommendation_review_metrics: Some(db::RecommendationReviewMetrics {
                window_size: 100,
                total_events: 12,
                approve_actions: approved,
                reject_actions: rejected,
                later_actions: 1,
                restore_actions: 1,
                feedback_positive: 4,
                feedback_refine: 1,
                feedback_negative: 1,
                failed_actions: 0,
                action_failure_rate: 0.0,
                non_positive_feedback_rate: 33.3,
                last_event_at: Some("2026-03-07T00:00:00Z".to_string()),
            }),
            launch_eval: Some(LaunchEvalSummary {
                generated_at: "2026-03-07T00:00:00Z".to_string(),
                config_path: Some("configs/launch_eval.yaml".to_string()),
                total: 8,
                passed: 8,
                failed: 0,
                failed_case_ids: vec![],
            }),
            launch_eval_candidate_snapshot: Some(launch_eval::LaunchEvalCandidateSnapshotInfo {
                output_path: "configs/launch_eval.generated.yaml".to_string(),
                exists: true,
                provenance_filter: "real".to_string(),
                scenario_count: 12,
                updated_at: Some("2026-03-07T00:00:00Z".to_string()),
                scenario_ids: vec![
                    "request-memory-a".to_string(),
                    "execution-memory-a".to_string(),
                ],
            }),
            launch_eval_candidate_snapshot_refresh_error: None,
        }
    }

    #[test]
    fn release_gate_fails_on_launch_ops_regression() {
        let baseline = release_baseline_with_ops(2, 4, 48.0, 8, 2, 1);
        let current = release_baseline_with_ops(14, 18, 28.0, 8, 2, 1);

        let result =
            evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

        assert!(!result.ok);
        assert!(result
            .regressions
            .iter()
            .any(|item| item.contains("Launch ops error rate too high")));
        assert!(result
            .regressions
            .iter()
            .any(|item| item.contains("Launch ops low-confidence rate too high")));
        assert!(result
            .regressions
            .iter()
            .any(|item| item.contains("Launch ops cache hit rate dropped")));
    }

    #[test]
    fn release_gate_warns_on_small_ops_sample() {
        let mut baseline = release_baseline_with_ops(1, 2, 40.0, 3, 2, 1);
        let mut current = release_baseline_with_ops(1, 2, 39.0, 3, 2, 1);
        baseline.launch_ops.as_mut().unwrap().total_requests = 10;
        current.launch_ops.as_mut().unwrap().total_requests = 8;

        let result =
            evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

        assert!(result.ok);
        assert!(result
            .warnings
            .iter()
            .any(|item| item.contains("Launch ops sample is small")));
    }

    #[test]
    fn release_gate_catches_nl_run_regression_and_exec_backlog() {
        let baseline = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
        let mut current = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
        current.nl_run_metrics = Some(db::NLRunMetrics {
            total: 40,
            completed: 10,
            manual_required: 16,
            approval_required: 8,
            blocked: 4,
            error: 18,
            success_rate: 25.0,
        });
        current.exec_approval_metrics = Some(db::ExecApprovalMetrics {
            window_size: 100,
            total: 14,
            pending: 9,
            approved: 2,
            rejected: 5,
            expired_pending: 2,
            allow_once: 1,
            allow_always: 1,
            deny: 5,
            approval_rate: 28.5,
            oldest_pending_created_at: Some("2026-03-07T00:00:00Z".to_string()),
            last_created_at: Some("2026-03-07T00:00:00Z".to_string()),
            last_resolved_at: Some("2026-03-07T00:00:00Z".to_string()),
        });

        let result =
            evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

        assert!(!result.ok);
        assert!(result
            .regressions
            .iter()
            .any(|item| item.contains("NL run success rate too low")));
        assert!(result
            .regressions
            .iter()
            .any(|item| item.contains("NL run error rate too high")));
        assert!(result
            .warnings
            .iter()
            .any(|item| item.contains("Exec approval backlog is high")));
        assert!(result
            .warnings
            .iter()
            .any(|item| item.contains("Expired exec approvals detected")));
    }

    #[test]
    fn release_gate_fails_on_recommendation_approval_drop() {
        let baseline = release_baseline_with_ops(1, 2, 45.0, 9, 1, 1);
        let current = release_baseline_with_ops(1, 2, 44.0, 1, 9, 10);

        let result =
            evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

        assert!(!result.ok);
        assert!(result
            .regressions
            .iter()
            .any(|item| item.contains("Recommendation approval rate too low")));
        assert!(result
            .regressions
            .iter()
            .any(|item| item.contains("Recommendation approval rate regressed")));
        assert!(result
            .warnings
            .iter()
            .any(|item| item.contains("Visible auto recommendation queue is high")));
    }

    #[test]
    fn release_gate_warns_on_recommendation_review_friction_regression() {
        let baseline = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
        let mut current = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
        current.recommendation_review_metrics = Some(db::RecommendationReviewMetrics {
            window_size: 100,
            total_events: 16,
            approve_actions: 4,
            reject_actions: 3,
            later_actions: 4,
            restore_actions: 1,
            feedback_positive: 1,
            feedback_refine: 2,
            feedback_negative: 1,
            failed_actions: 4,
            action_failure_rate: 33.3,
            non_positive_feedback_rate: 75.0,
            last_event_at: Some("2026-03-07T00:00:00Z".to_string()),
        });

        let result =
            evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

        assert!(result.ok);
        assert!(result.warnings.iter().any(|item| {
            item.contains("Recommendation review action-failure rate is elevated")
        }));
        assert!(result.warnings.iter().any(|item| {
            item.contains("Recommendation review non-positive feedback rate is elevated")
        }));
        assert!(result
            .warnings
            .iter()
            .any(|item| { item.contains("Recommendation review action-failure rate regressed") }));
        assert!(result.warnings.iter().any(|item| {
            item.contains("Recommendation review non-positive feedback rate regressed")
        }));
    }

    #[test]
    fn stale_exec_approval_metrics_are_ignored_for_release_gate() {
        let baseline = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
        let mut current = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
        current.exec_approval_metrics = Some(normalize_release_exec_approval_metrics(
            db::ExecApprovalMetrics {
                window_size: 100,
                total: 14,
                pending: 9,
                approved: 0,
                rejected: 0,
                expired_pending: 4,
                allow_once: 0,
                allow_always: 0,
                deny: 0,
                approval_rate: 0.0,
                oldest_pending_created_at: Some("2026-02-01T00:00:00Z".to_string()),
                last_created_at: Some("2026-02-01T00:00:00Z".to_string()),
                last_resolved_at: None,
            },
        ));

        let result =
            evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

        assert!(result.ok);
        assert!(!result
            .warnings
            .iter()
            .any(|item| item.contains("Exec approval backlog is high")));
        assert!(!result
            .warnings
            .iter()
            .any(|item| item.contains("Expired exec approvals detected")));
    }

    #[test]
    #[serial]
    fn visible_work_pending_count_caps_release_queue_to_display_limit() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let db_path = temp_dir.path().join("release-gate-visible.db");
        let prev_db_path = std::env::var("STEER_DB_PATH").ok();
        std::env::set_var("STEER_DB_PATH", db_path.to_string_lossy().to_string());
        crate::db::reset_connection();
        crate::db::init().expect("init db");
        crate::db::clear_recommendations_for_tests();

        for idx in 0..10 {
            let proposal = crate::recommendation::AutomationProposal {
                title: format!("Ready Workflow {}", idx),
                summary: "ready".to_string(),
                trigger: format!("pattern-{}", idx),
                actions: vec!["notify".to_string()],
                confidence: 0.9,
                n8n_prompt: "Create a workflow that posts work updates to Notion.".to_string(),
                evidence: vec![
                    "Frequency: Found 6 occurrences".to_string(),
                    "Span: 4 distinct day(s)".to_string(),
                    "policy.reason=strong_work_signals=notion,calendar".to_string(),
                    "policy.reason=pattern.context_score=0.84".to_string(),
                    "policy.reason=pattern.work_context=strong".to_string(),
                ],
                pattern_id: Some(format!("pattern-{}", idx)),
                category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
                business_score: 0.82,
            };
            crate::db::insert_recommendation(&proposal).expect("insert ready proposal");
        }

        let count = visible_auto_work_pending_count().expect("visible pending count");
        assert_eq!(
            count,
            crate::recommendation_policy::pending_recommendation_display_limit() as i64
        );

        crate::db::reset_connection();
        match prev_db_path {
            Some(value) => std::env::set_var("STEER_DB_PATH", value),
            None => std::env::remove_var("STEER_DB_PATH"),
        }
        crate::db::reset_connection();
    }

    #[test]
    fn release_gate_fails_on_launch_eval_regression() {
        let baseline = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
        let mut current = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
        current.launch_eval = Some(LaunchEvalSummary {
            generated_at: "2026-03-07T01:00:00Z".to_string(),
            config_path: Some("configs/launch_eval.yaml".to_string()),
            total: 8,
            passed: 6,
            failed: 2,
            failed_case_ids: vec!["ai-digest-web-fallback".to_string()],
        });

        let result =
            evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

        assert!(!result.ok);
        assert!(result
            .regressions
            .iter()
            .any(|item| item.contains("Launch eval has failing scenarios")));
        assert!(result
            .regressions
            .iter()
            .any(|item| item.contains("Launch eval failures increased")));
    }

    #[test]
    fn release_gate_warns_on_stale_launch_eval_report() {
        let baseline = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
        let mut current = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
        current.launch_eval = Some(LaunchEvalSummary {
            generated_at: "2024-03-01T00:00:00Z".to_string(),
            config_path: Some("configs/launch_eval.yaml".to_string()),
            total: 8,
            passed: 8,
            failed: 0,
            failed_case_ids: vec![],
        });

        let result =
            evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

        assert!(result.ok);
        assert!(result
            .warnings
            .iter()
            .any(|item| item.contains("Launch eval report is stale")));
    }

    #[test]
    fn release_gate_warns_on_thin_launch_eval_candidate_snapshot() {
        let baseline = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
        let mut current = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
        current.launch_eval_candidate_snapshot =
            Some(launch_eval::LaunchEvalCandidateSnapshotInfo {
                output_path: "configs/launch_eval.generated.yaml".to_string(),
                exists: true,
                provenance_filter: "real".to_string(),
                scenario_count: 0,
                updated_at: Some(chrono::Utc::now().to_rfc3339()),
                scenario_ids: vec![],
            });

        let result =
            evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

        assert!(result
            .warnings
            .iter()
            .any(|item| item.contains("candidate snapshot is thin")));
        assert!(result
            .regressions
            .iter()
            .any(|item| item.contains("candidate coverage regressed")));
    }

    #[test]
    fn release_gate_warns_on_candidate_snapshot_refresh_error() {
        let baseline = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
        let mut current = release_baseline_with_ops(1, 2, 45.0, 8, 2, 1);
        current.launch_eval_candidate_snapshot_refresh_error =
            Some("permission denied".to_string());

        let result =
            evaluate_release_gate(current, Some(baseline), None, None, None, None, None, None);

        assert!(result
            .warnings
            .iter()
            .any(|item| item.contains("snapshot refresh failed")));
    }

    #[test]
    #[serial]
    fn build_baseline_refreshes_launch_eval_candidate_snapshot_by_default() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let db_path = temp_dir.path().join("release-gate.db");
        let prev_db_path = std::env::var("STEER_DB_PATH").ok();
        std::env::set_var("STEER_DB_PATH", db_path.to_string_lossy().to_string());
        crate::db::init().expect("init db");
        crate::db::clear_request_memory_for_tests();
        crate::db::clear_execution_memory_for_tests();
        crate::db::clear_launch_ops_events_for_tests();

        let request_intent = serde_json::json!({
            "command": "calendar_today",
            "params": {},
            "confidence": 0.94
        });
        crate::db::upsert_request_memory_scoped(
            Some("channel_web__type_direct__sender_release"),
            "오늘 일정 보여줘",
            Some(&request_intent),
            Some("RELEASE_GATE_CACHE"),
            "ttl_response_signature",
            "api.chat.deterministic",
            0.94,
        )
        .expect("seed request memory");

        let baseline = build_baseline(ReleaseBaselineRequest {
            workdir: Some(temp_dir.path().display().to_string()),
            max_files: Some(10),
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
            launch_eval_candidate_limit: Some(10),
            launch_eval_snapshot_output_path: Some("generated.yaml".to_string()),
        });

        assert_eq!(baseline.launch_eval_candidate_snapshot_refresh_error, None);
        let snapshot = baseline
            .launch_eval_candidate_snapshot
            .expect("snapshot info should exist");
        assert!(snapshot.exists);
        assert!(snapshot.scenario_count >= 1);
        assert!(snapshot.output_path.ends_with("generated.yaml"));
        assert!(baseline.quality.is_some());

        match prev_db_path {
            Some(value) => std::env::set_var("STEER_DB_PATH", value),
            None => std::env::remove_var("STEER_DB_PATH"),
        }
        crate::db::reset_connection();
    }

    #[test]
    fn derive_release_quality_score_produces_structured_score() {
        let baseline = release_baseline_with_ops(0, 0, 40.0, 8, 2, 1);
        let quality = derive_release_quality_score(
            baseline.consistency.as_ref(),
            baseline.semantic.as_ref(),
            baseline.performance.as_ref(),
            baseline.launch_ops.as_ref(),
            baseline.launch_eval.as_ref(),
        );

        assert!(quality.overall >= 0.0);
        assert!(quality.breakdown.contains_key("functionality"));
        assert!(quality.breakdown.contains_key("ui_ux"));
        assert!(quality.breakdown.contains_key("code_quality"));
        assert!(quality.breakdown.contains_key("api_compatibility"));
        assert!(!quality.recommendation.is_empty());
    }
}
