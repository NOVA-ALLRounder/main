use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::{db, workflow_intake};

use super::agent::{
    ApprovalPolicyQuery, ApprovalPolicyRequest, ApprovalPolicyResponse,
    CollectorHandoffReceiptsQuery, ExecAllowlistQuery, ExecAllowlistRequest, ExecApprovalQuery,
    ExecApprovalResolve, ExecResultsQuery, NLRunMetricsQuery, NLRunQuery, RoutineRunsQuery,
    TaskRunArtifactsQuery, TaskRunAssertionsQuery, TaskRunsQuery, VerificationRunsQuery,
    WorkflowProvisionOpsQuery,
};

// --- Routine Handlers ---

#[derive(serde::Deserialize)]
pub(crate) struct IngestCollectorHandoffRequest {
    config_path: Option<String>,
}

#[derive(serde::Serialize)]
struct IngestCollectorHandoffResponse {
    status: String,
    detail: String,
    package_id: Option<String>,
    recommendation_id: Option<i64>,
    inserted: bool,
}

pub(crate) async fn ingest_collector_handoff_handler(
    Json(payload): Json<IngestCollectorHandoffRequest>,
) -> impl IntoResponse {
    match workflow_intake::ingest_latest_collector_handoff(payload.config_path.as_deref()) {
        Ok(outcome) => (
            StatusCode::OK,
            Json(IngestCollectorHandoffResponse {
                status: outcome.status,
                detail: outcome.detail,
                package_id: outcome.package_id,
                recommendation_id: outcome.recommendation_id,
                inserted: outcome.inserted,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(IngestCollectorHandoffResponse {
                status: "error".to_string(),
                detail: e.to_string(),
                package_id: None,
                recommendation_id: None,
                inserted: false,
            }),
        ),
    }
}

pub(crate) async fn list_routines() -> Json<Vec<crate::db::Routine>> {
    match crate::db::get_all_routines() {
        Ok(routines) => Json(routines),
        Err(e) => {
            eprintln!("Failed to list routines: {}", e);
            Json(Vec::new())
        }
    }
}

#[derive(serde::Deserialize)]
pub(crate) struct CreateRoutineRequest {
    name: String,
    #[serde(alias = "cron_expression")] // Accept both "cron" and "cron_expression"
    cron: String,
    prompt: String,
}

pub(crate) async fn create_routine_handler(
    Json(payload): Json<CreateRoutineRequest>,
) -> Json<serde_json::Value> {
    match crate::db::create_routine(&payload.name, &payload.cron, &payload.prompt) {
        Ok(id) => Json(serde_json::json!({ "status": "ok", "id": id })),
        Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
    }
}

// --- Issue #2 Fix: Toggle Routine ---
#[derive(serde::Deserialize)]
pub(crate) struct ToggleRoutineRequest {
    enabled: bool,
}

pub(crate) async fn toggle_routine_handler(
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(payload): Json<ToggleRoutineRequest>,
) -> Json<serde_json::Value> {
    match crate::db::toggle_routine(id, payload.enabled) {
        Ok(_) => Json(serde_json::json!({ "status": "ok" })),
        Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
    }
}

pub(crate) async fn list_exec_approvals(
    Query(query): Query<ExecApprovalQuery>,
) -> Json<Vec<db::ExecApproval>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let status = match query.status.as_deref() {
        Some("all") => None,
        other => other,
    };
    let approvals = db::list_exec_approvals(status, limit).unwrap_or_default();
    Json(approvals)
}

pub(crate) async fn approve_exec_approval(
    Path(id): Path<String>,
    payload: Option<Json<ExecApprovalResolve>>,
) -> StatusCode {
    let resolved_by = payload.as_ref().and_then(|p| p.resolved_by.as_deref());
    let decision = payload
        .as_ref()
        .and_then(|p| p.decision.as_deref())
        .unwrap_or("allow-once");

    if decision == "allow-always" {
        if let Ok(Some(approval)) = db::get_exec_approval(&id) {
            let _ = db::add_exec_allowlist(&approval.command, approval.cwd.as_deref());
        }
    }

    match db::resolve_exec_approval(&id, "approved", resolved_by, Some(decision)) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(crate) async fn reject_exec_approval(
    Path(id): Path<String>,
    payload: Option<Json<ExecApprovalResolve>>,
) -> StatusCode {
    let resolved_by = payload.as_ref().and_then(|p| p.resolved_by.as_deref());
    match db::resolve_exec_approval(&id, "rejected", resolved_by, Some("deny")) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(crate) async fn list_routine_runs(
    Query(query): Query<RoutineRunsQuery>,
) -> Json<Vec<db::RoutineRun>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let runs = db::list_routine_runs(limit).unwrap_or_default();
    Json(runs)
}

pub(crate) async fn list_exec_allowlist(
    Query(query): Query<ExecAllowlistQuery>,
) -> Json<Vec<db::ExecAllowlistEntry>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let entries = db::list_exec_allowlist(limit).unwrap_or_default();
    Json(entries)
}

pub(crate) async fn list_exec_results(
    Query(query): Query<ExecResultsQuery>,
) -> Json<Vec<db::ExecResult>> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let status = query.status.as_deref();
    let results = db::list_exec_results(status, limit).unwrap_or_default();
    Json(results)
}

pub(crate) async fn list_verification_runs(
    Query(query): Query<VerificationRunsQuery>,
) -> Json<Vec<db::VerificationRun>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let runs = db::list_verification_runs(limit).unwrap_or_default();
    Json(runs)
}

pub(crate) async fn list_nl_runs_handler(Query(query): Query<NLRunQuery>) -> Json<Vec<db::NLRun>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let runs = db::list_nl_runs(limit).unwrap_or_default();
    Json(runs)
}

pub(crate) async fn nl_run_metrics_handler(
    Query(query): Query<NLRunMetricsQuery>,
) -> Json<db::NLRunMetrics> {
    let limit = query.limit.unwrap_or(50).clamp(1, 500);
    let metrics = db::get_nl_run_metrics(limit).unwrap_or(db::NLRunMetrics {
        total: 0,
        completed: 0,
        manual_required: 0,
        approval_required: 0,
        blocked: 0,
        error: 0,
        success_rate: 0.0,
    });
    Json(metrics)
}

pub(crate) async fn list_task_runs_handler(
    Query(query): Query<TaskRunsQuery>,
) -> Json<Vec<db::TaskRunRecord>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let status = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let runs = db::list_task_runs(limit, status).unwrap_or_default();
    Json(runs)
}

pub(crate) async fn list_collector_handoff_receipts_handler(
    Query(query): Query<CollectorHandoffReceiptsQuery>,
) -> Json<Vec<db::CollectorHandoffReceiptRecord>> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let rows = db::list_collector_handoff_receipts(limit).unwrap_or_default();
    Json(rows)
}

pub(crate) async fn list_workflow_provision_ops_handler(
    Query(query): Query<WorkflowProvisionOpsQuery>,
) -> Json<Vec<db::WorkflowProvisionOpRecord>> {
    // Opportunistically reconcile stale requested/created ops on read so UI polling
    // can surface terminal status without relying on a separate scheduler tick.
    let _ = db::reconcile_workflow_provision_ops(50);
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let status = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let rows =
        db::list_workflow_provision_ops(limit, status, query.recommendation_id).unwrap_or_default();
    Json(rows)
}

pub(crate) async fn get_task_run_handler(Path(run_id): Path<String>) -> impl IntoResponse {
    match db::get_task_run(&run_id) {
        Ok(Some(run)) => (StatusCode::OK, Json(json!(run))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "task_run_not_found", "run_id": run_id })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "task_run_lookup_failed", "details": e.to_string() })),
        )
            .into_response(),
    }
}

pub(crate) async fn list_task_stage_runs_handler(Path(run_id): Path<String>) -> impl IntoResponse {
    match db::get_task_run_readonly(&run_id) {
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "task_run_not_found", "run_id": run_id })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "task_run_lookup_failed", "details": e.to_string() })),
        )
            .into_response(),
        Ok(Some(_)) => match db::list_task_stage_runs(&run_id) {
            Ok(stages) => (StatusCode::OK, Json(json!(stages))).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "task_stage_runs_failed", "details": e.to_string() })),
            )
                .into_response(),
        },
    }
}

pub(crate) async fn list_task_stage_assertions_handler(
    Path(run_id): Path<String>,
    Query(query): Query<TaskRunAssertionsQuery>,
) -> impl IntoResponse {
    let options = db::TaskStageAssertionListOptions {
        stage_name: query.stage_name.and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }),
        failed_only: query.failed_only.unwrap_or(false),
        limit: query.limit.map(|value| value.clamp(1, 500)),
        offset: query.offset.unwrap_or(0).max(0),
    };
    match db::get_task_run_readonly(&run_id) {
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "task_run_not_found", "run_id": run_id })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "task_run_lookup_failed", "details": e.to_string() })),
        )
            .into_response(),
        Ok(Some(_)) => match db::list_task_stage_assertions_with_options(&run_id, &options) {
            Ok(assertions) => (StatusCode::OK, Json(json!(assertions))).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "task_stage_assertions_failed", "details": e.to_string() })),
            )
                .into_response(),
        },
    }
}

pub(crate) async fn list_task_run_artifacts_handler(
    Path(run_id): Path<String>,
    Query(query): Query<TaskRunArtifactsQuery>,
) -> impl IntoResponse {
    let options = db::TaskRunArtifactListOptions {
        artifact_type: query.artifact_type.and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }),
        limit: query.limit.map(|value| value.clamp(1, 500)),
        offset: query.offset.unwrap_or(0).max(0),
    };
    match db::get_task_run_readonly(&run_id) {
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "task_run_not_found", "run_id": run_id })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "task_run_lookup_failed", "details": e.to_string() })),
        )
            .into_response(),
        Ok(Some(_)) => match db::list_task_run_artifacts_with_options(&run_id, &options) {
            Ok(artifacts) => Json(json!({ "artifacts": artifacts })).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "task_run_artifacts_failed", "details": e.to_string() })),
            )
                .into_response(),
        },
    }
}

pub(crate) async fn list_nl_approval_policies(
    Query(query): Query<ApprovalPolicyQuery>,
) -> Json<Vec<ApprovalPolicyResponse>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let policies = db::list_approval_policies(limit).unwrap_or_default();
    let mapped = policies
        .into_iter()
        .map(|policy| ApprovalPolicyResponse {
            policy_key: policy.policy_key,
            decision: policy.decision,
            updated_at: policy.updated_at,
        })
        .collect();
    Json(mapped)
}

pub(crate) async fn set_nl_approval_policy(
    Json(payload): Json<ApprovalPolicyRequest>,
) -> StatusCode {
    if payload.policy_key.trim().is_empty() || payload.decision.trim().is_empty() {
        return StatusCode::BAD_REQUEST;
    }
    match db::upsert_approval_policy(&payload.policy_key, &payload.decision) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(crate) async fn remove_nl_approval_policy(Path(key): Path<String>) -> StatusCode {
    if key.trim().is_empty() {
        return StatusCode::BAD_REQUEST;
    }
    match db::delete_approval_policy(&key) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(crate) async fn add_exec_allowlist(Json(payload): Json<ExecAllowlistRequest>) -> StatusCode {
    if payload.pattern.trim().is_empty() {
        return StatusCode::BAD_REQUEST;
    }
    match db::add_exec_allowlist(&payload.pattern, payload.cwd.as_deref()) {
        Ok(_) => StatusCode::CREATED,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("exec allowlist pattern rejected") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

pub(crate) async fn remove_exec_allowlist(Path(id): Path<i64>) -> StatusCode {
    match db::remove_exec_allowlist(id) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
