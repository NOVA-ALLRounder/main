#![allow(dead_code)] // Allow unused library functions for future use

#[path = "db/memory/mod.rs"]
mod memory;
pub(crate) use memory::{
    backfill_execution_memory_scope_keys, backfill_request_memory_cache_columns,
    backfill_request_memory_scope_keys,
};
#[cfg(test)]
pub use memory::{clear_execution_memory_for_tests, clear_request_memory_for_tests};
pub use memory::{
    delete_execution_memory, delete_execution_memory_scoped, delete_request_memory,
    delete_request_memory_scoped, get_execution_memory, get_execution_memory_scoped,
    get_request_memory, get_request_memory_by_signature, get_request_memory_by_signature_scoped,
    get_request_memory_scoped, list_execution_memory_records, list_request_memory_records,
    record_execution_memory_feedback, record_execution_memory_feedback_scoped,
    record_request_memory_feedback, record_request_memory_feedback_scoped,
    restore_execution_memory, restore_execution_memory_scoped, restore_request_memory,
    restore_request_memory_scoped, suppress_execution_memory, suppress_execution_memory_scoped,
    suppress_request_memory, suppress_request_memory_scoped, upsert_execution_memory,
    upsert_execution_memory_scoped, upsert_request_memory, upsert_request_memory_scoped,
    ExecutionMemoryRecord, RequestMemoryRecord,
};

#[path = "db/approval.rs"]
mod approval;
#[cfg(test)]
pub use approval::clear_exec_approvals_for_tests;
pub use approval::{
    add_exec_allowlist, create_exec_approval, create_exec_result, delete_approval_policy,
    find_valid_exec_approval, get_active_approval_decision, get_approval_policy_decision,
    get_exec_approval, get_exec_approval_metrics, get_exec_approval_status, list_approval_policies,
    list_exec_allowlist, list_exec_approvals, list_exec_results, list_pending_exec_results,
    remove_exec_allowlist, resolve_exec_approval, update_exec_result, upsert_approval_decision,
    upsert_approval_policy, ActiveApprovalDecision, ApprovalPolicy, ExecAllowlistEntry,
    ExecApproval, ExecApprovalMetrics, ExecResult,
};

#[path = "db/runs/mod.rs"]
mod runs;
#[cfg(test)]
pub use runs::clear_nl_runs_for_tests;
pub use runs::{
    claim_task_run, create_task_run, get_latest_inflight_task_run, get_nl_run_metrics,
    get_release_nl_run_metrics, get_task_run, insert_nl_run, insert_nl_run_with_source_key,
    list_nl_runs, list_task_run_artifacts, list_task_runs, list_task_stage_assertions,
    list_task_stage_runs, mark_orphaned_inflight_task_runs_failed,
    mark_stale_running_task_runs_finished, record_task_stage_assertion, record_task_stage_run,
    sync_release_nl_runs_from_launch_ops, update_task_run_outcome, upsert_task_run_artifact, NLRun,
    NLRunMetrics, TaskRunArtifactRecord, TaskRunRecord, TaskStageAssertionRecord,
    TaskStageRunRecord,
};

#[path = "db/recommendation_review.rs"]
mod recommendation_review;
#[cfg(test)]
pub use recommendation_review::clear_recommendation_review_events_for_tests;
pub use recommendation_review::{
    get_recommendation_review_metrics, list_recommendation_review_events,
    record_recommendation_review_event, restore_recommendation, snooze_recommendation,
    update_recommendation_review_status, RecommendationReviewEventRecord,
    RecommendationReviewMetrics,
};

#[path = "db/ops.rs"]
mod ops;
#[cfg(test)]
pub use ops::{clear_launch_ops_events_for_tests, clear_memory_admin_events_for_tests};
pub use ops::{
    get_launch_ops_metrics, get_memory_ops_metrics, list_launch_ops_events,
    list_memory_admin_events, record_launch_ops_event, record_memory_admin_event,
    LaunchOpsEventRecord, LaunchOpsMetrics, LaunchOpsRouteBreakdown, MemoryAdminEventRecord,
    MemoryOpsMetrics,
};

#[path = "db/provisioning.rs"]
mod provisioning;
pub use provisioning::{
    commit_workflow_provision_success, create_workflow_provision_op, latest_workflow_provision_op,
    list_collector_handoff_receipts, list_workflow_provision_ops, mark_recommendation_approved,
    mark_workflow_provision_created, mark_workflow_provision_failed,
    mark_workflow_provision_in_progress, mark_workflow_provision_reconcile_needed,
    reconcile_workflow_provision_ops, record_collector_handoff_receipt,
    release_recommendation_provisioning_claim, CollectorHandoffReceiptRecord,
    WorkflowProvisionOpRecord,
};

#[path = "db/meta.rs"]
mod meta;
pub use meta::{
    create_routine_run, finish_routine_run, get_judgment_state, get_latest_quality_score,
    get_release_baseline_json, insert_quality_score, insert_verification_run, list_routine_runs,
    list_verification_runs, upsert_judgment_state, upsert_release_baseline_json, JudgmentState,
    QualityScoreRecord, ReleaseBaselineRecord, RoutineRun, VerificationRun,
};

#[path = "db/recommendations.rs"]
mod recommendations;
#[cfg(test)]
pub use recommendations::clear_recommendations_for_tests;
pub use recommendations::{
    claim_recommendation_provisioning, count_recent_recommendations, get_recent_recommendations,
    get_recommendation, get_recommendation_metrics, get_recommendations,
    get_recommendations_with_filter, has_recent_pattern_recommendation, insert_recommendation,
    insert_routine_candidate, list_recommendations, mark_recommendation_failed,
    record_recommendation_feedback, seed_advanced_examples, update_recommendation_status,
    Recommendation, RecommendationMetrics,
};

#[path = "db/events.rs"]
mod events;
pub use events::{
    fetch_all_events_v2, get_recent_chat_history, get_recent_events, init_sessions_table, init_v2,
    insert_chat_message, insert_event, insert_event_v2, insert_session, ChatMessage,
};
#[path = "db/routines.rs"]
mod routines;
pub(crate) use routines::validate_exec_allowlist_pattern;
pub use routines::{
    claim_routine_execution, create_routine, delete_learned_routine, get_active_policy_config,
    get_active_routines, get_all_routines, get_dashboard_stats, get_due_routines,
    get_learned_routine, is_exec_allowlisted, list_learned_routines, release_routine_execution,
    save_learned_routine, toggle_routine, update_routine_execution, DashboardStats, LearnedRoutine,
    PolicyConfigReport, Routine,
};
#[cfg(test)]
pub(crate) use routines::{
    exec_pattern_match_with_flags, validate_exec_allowlist_pattern_with_flags,
};

#[path = "db/bootstrap.rs"]
mod bootstrap;
pub use bootstrap::{current_db_path, init, reset_connection};
pub(crate) use bootstrap::{ensure_approval_decisions_table, get_db_lock};

fn truncate_text(input: &str, max_chars: usize) -> String {
    input.trim().chars().take(max_chars).collect::<String>()
}

const GLOBAL_MEMORY_SCOPE: &str = "global";

fn normalize_memory_scope(scope: Option<&str>) -> String {
    let raw = scope.unwrap_or(GLOBAL_MEMORY_SCOPE).trim().to_lowercase();
    let normalized = raw
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");

    if normalized.is_empty() {
        GLOBAL_MEMORY_SCOPE.to_string()
    } else {
        truncate_text(&normalized, 96)
    }
}

fn row_bool(row: &rusqlite::Row<'_>, idx: usize, default: bool) -> bool {
    row.get::<_, i64>(idx)
        .map(|value| value != 0)
        .unwrap_or(default)
}

fn normalize_admin_limit(limit: i64) -> i64 {
    limit.clamp(1, 200)
}

#[cfg(test)]
#[path = "db/tests/mod.rs"]
mod tests;
