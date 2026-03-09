use super::super::support::{env_f64, env_i64, percentage};
use super::super::ReleaseBaseline;

pub(super) fn compare_recommendation_health(
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

pub(super) fn compare_recommendation_review_health(
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
