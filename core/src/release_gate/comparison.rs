use super::support::{count_severity, env_f64};
#[path = "comparison/execution.rs"]
mod execution;
#[path = "comparison/launch.rs"]
mod launch;
#[path = "comparison/recommendations.rs"]
mod recommendations;
use super::{ReleaseBaseline, ReleaseGateResult};

pub(super) fn evaluate_release_gate(
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
    launch::compare_launch_ops(
        &base,
        &current,
        &mut regressions,
        &mut warnings,
        launch_error_override,
        launch_low_conf_override,
        launch_cache_drop_override,
    );
    execution::compare_execution_safety(&base, &current, &mut regressions, &mut warnings);
    recommendations::compare_recommendation_health(
        &base,
        &current,
        &mut regressions,
        &mut warnings,
        recommendation_approval_min,
    );
    recommendations::compare_recommendation_review_health(&base, &current, &mut warnings);
    launch::compare_launch_eval(&base, &current, &mut regressions, &mut warnings);
    launch::compare_launch_eval_candidate_snapshot(
        &base,
        &current,
        &mut regressions,
        &mut warnings,
    );

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
