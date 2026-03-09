use axum::{routing::get, Router};

use super::super::control::{
    add_exec_allowlist, approve_exec_approval, create_routine_handler, get_task_run_handler,
    ingest_collector_handoff_handler, list_collector_handoff_receipts_handler, list_exec_allowlist,
    list_exec_approvals, list_exec_results, list_nl_approval_policies, list_nl_runs_handler,
    list_routine_runs, list_routines, list_task_run_artifacts_handler, list_task_runs_handler,
    list_task_stage_assertions_handler, list_task_stage_runs_handler, list_verification_runs,
    list_workflow_provision_ops_handler, nl_run_metrics_handler, reject_exec_approval,
    remove_exec_allowlist, remove_nl_approval_policy, set_nl_approval_policy,
    toggle_routine_handler,
};
use super::super::AppState;

pub(crate) fn build_control_routes() -> Router<AppState> {
    Router::new()
        .route("/api/exec-approvals", get(list_exec_approvals))
        .route(
            "/api/exec-approvals/:id/approve",
            axum::routing::post(approve_exec_approval),
        )
        .route(
            "/api/exec-approvals/:id/reject",
            axum::routing::post(reject_exec_approval),
        )
        .route(
            "/api/exec-allowlist",
            get(list_exec_allowlist).post(add_exec_allowlist),
        )
        .route(
            "/api/exec-allowlist/:id",
            axum::routing::delete(remove_exec_allowlist),
        )
        .route("/api/exec-results", get(list_exec_results))
        .route(
            "/api/routines",
            get(list_routines).post(create_routine_handler),
        )
        .route(
            "/api/collector/handoff/ingest",
            axum::routing::post(ingest_collector_handoff_handler),
        )
        .route(
            "/api/collector/handoff/receipts",
            get(list_collector_handoff_receipts_handler),
        )
        .route(
            "/api/workflow/provision-ops",
            get(list_workflow_provision_ops_handler),
        )
        .route(
            "/api/routines/:id",
            axum::routing::patch(toggle_routine_handler),
        )
        .route("/api/routine-runs", get(list_routine_runs))
        .route("/api/verify/runs", get(list_verification_runs))
        .route("/api/agent/nl-runs", get(list_nl_runs_handler))
        .route("/api/agent/nl-metrics", get(nl_run_metrics_handler))
        .route("/api/agent/task-runs", get(list_task_runs_handler))
        .route("/api/agent/task-runs/:run_id", get(get_task_run_handler))
        .route(
            "/api/agent/task-runs/:run_id/stages",
            get(list_task_stage_runs_handler),
        )
        .route(
            "/api/agent/task-runs/:run_id/assertions",
            get(list_task_stage_assertions_handler),
        )
        .route(
            "/api/agent/task-runs/:run_id/artifacts",
            get(list_task_run_artifacts_handler),
        )
        .route(
            "/api/agent/approval-policies",
            get(list_nl_approval_policies).post(set_nl_approval_policy),
        )
        .route(
            "/api/agent/approval-policies/:key",
            axum::routing::delete(remove_nl_approval_policy),
        )
}
