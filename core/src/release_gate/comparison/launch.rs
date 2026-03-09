use super::super::support::{env_f64, env_i64, env_usize, launch_eval_age_hours, ratio};
use super::super::ReleaseBaseline;

pub(super) fn compare_launch_ops(
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

pub(super) fn compare_launch_eval(
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

pub(super) fn compare_launch_eval_candidate_snapshot(
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
