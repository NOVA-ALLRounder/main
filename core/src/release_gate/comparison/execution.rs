use super::super::support::{env_f64, env_i64, ratio};
use super::super::ReleaseBaseline;

pub(super) fn compare_execution_safety(
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
