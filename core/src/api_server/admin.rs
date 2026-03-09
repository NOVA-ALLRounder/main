use axum::{extract::Query, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::db;

use super::recommendations::{
    recommendation_effective_category_and_score, recommendation_effective_status,
};

#[derive(Deserialize)]
pub struct MemoryRecordsQuery {
    pub limit: Option<i64>,
    pub include_suppressed: Option<bool>,
}

#[derive(Deserialize)]
pub struct RecommendationReviewEventsQuery {
    pub limit: Option<i64>,
}

#[derive(Serialize)]
pub struct MemoryRecordsResponse {
    pub metrics: crate::db::MemoryOpsMetrics,
    pub request_memory: Vec<crate::db::RequestMemoryRecord>,
    pub execution_memory: Vec<crate::db::ExecutionMemoryRecord>,
    pub recent_admin_events: Vec<crate::db::MemoryAdminEventRecord>,
}

#[derive(Deserialize)]
pub struct LaunchOpsQuery {
    pub limit: Option<i64>,
}

#[derive(Serialize)]
pub struct RecommendationMetricsResponse {
    pub total: i64,
    pub approved: i64,
    pub rejected: i64,
    pub failed: i64,
    pub pending: i64,
    pub later: i64,
    pub legacy_other: i64,
    pub approval_rate: f64,
    pub last_created_at: Option<String>,
}

#[derive(Serialize)]
pub struct LaunchOpsResponse {
    pub chat_metrics: crate::db::LaunchOpsMetrics,
    pub memory_metrics: crate::db::MemoryOpsMetrics,
    pub nl_run_metrics: crate::db::NLRunMetrics,
    pub exec_approval_metrics: crate::db::ExecApprovalMetrics,
    pub recommendation_metrics: RecommendationMetricsResponse,
    pub recommendation_review_metrics: crate::db::RecommendationReviewMetrics,
    pub recent_events: Vec<crate::db::LaunchOpsEventRecord>,
}

#[derive(Deserialize)]
pub struct RequestMemoryAdminActionRequest {
    pub request_text: String,
    pub memory_scope: Option<String>,
    pub reason: Option<String>,
    pub actor: Option<String>,
}

#[derive(Deserialize)]
pub struct ExecutionMemoryAdminActionRequest {
    pub intent_command: String,
    pub params_key: String,
    pub memory_scope: Option<String>,
    pub reason: Option<String>,
    pub actor: Option<String>,
}

#[derive(Serialize)]
pub struct MemoryAdminActionResponse {
    pub ok: bool,
    pub kind: String,
    pub action: String,
    pub message: String,
}

pub(super) async fn get_recommendation_metrics() -> Json<RecommendationMetricsResponse> {
    let metrics = db::get_recommendation_metrics().unwrap_or(crate::db::RecommendationMetrics {
        total: 0,
        approved: 0,
        rejected: 0,
        failed: 0,
        pending: 0,
        later: 0,
        legacy_other: 0,
        last_created_at: None,
    });
    let visible_queue_counts = db::get_recommendations_with_filter(Some("all"))
        .ok()
        .map(|recs| {
            let work_filter =
                crate::recommendation_policy::normalize_list_category_filter(None);
            let mut pending = 0_i64;
            let mut later = 0_i64;
            let mut failed = 0_i64;
            for rec in recs {
                let (category, _) = recommendation_effective_category_and_score(&rec);
                if !crate::recommendation_policy::matches_category_filter(
                    &category,
                    work_filter.as_deref(),
                ) {
                    continue;
                }
                match recommendation_effective_status(&rec).as_str() {
                    "pending" => {
                        if rec.pattern_id.is_some()
                            && !crate::recommendation_policy::evaluate_recommendation_approval_readiness(&rec).ready
                        {
                            continue;
                        }
                        pending += 1;
                    }
                    "later" => later += 1,
                    "failed" => failed += 1,
                    _ => {}
                }
            }
            (pending, later, failed)
        })
        .unwrap_or((metrics.pending, metrics.later, metrics.failed));

    let approval_rate = if metrics.total > 0 {
        (metrics.approved as f64 / metrics.total as f64) * 100.0
    } else {
        0.0
    };

    Json(RecommendationMetricsResponse {
        total: metrics.total,
        approved: metrics.approved,
        rejected: metrics.rejected,
        failed: visible_queue_counts.2,
        pending: visible_queue_counts.0,
        later: visible_queue_counts.1,
        legacy_other: metrics.legacy_other,
        approval_rate,
        last_created_at: metrics.last_created_at,
    })
}

pub(super) async fn list_recommendation_review_events_handler(
    Query(query): Query<RecommendationReviewEventsQuery>,
) -> Json<Vec<db::RecommendationReviewEventRecord>> {
    let limit = memory_admin_limit(query.limit);
    Json(db::list_recommendation_review_events(limit).unwrap_or_default())
}

fn launch_ops_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(200).clamp(20, 500)
}

pub(super) async fn get_launch_ops_handler(
    Query(query): Query<LaunchOpsQuery>,
) -> Json<LaunchOpsResponse> {
    let limit = launch_ops_limit(query.limit);
    let chat_metrics = db::get_launch_ops_metrics(limit).unwrap_or(db::LaunchOpsMetrics {
        window_size: limit,
        total_requests: 0,
        blocked_requests: 0,
        intent_memory_hits: 0,
        request_memory_hits: 0,
        execution_memory_hits: 0,
        cached_response_hit_rate: 0.0,
        deterministic_routes: 0,
        llm_routes: 0,
        ai_digest_routes: 0,
        ai_digest_auto_routes: 0,
        local_routes: 0,
        freshness_bypasses: 0,
        low_confidence_routes: 0,
        unknown_routes: 0,
        error_routes: 0,
        last_event_at: None,
        route_breakdown: Vec::new(),
    });
    let memory_metrics = db::get_memory_ops_metrics().unwrap_or(db::MemoryOpsMetrics {
        request_active: 0,
        request_suppressed: 0,
        execution_active: 0,
        execution_suppressed: 0,
        last_request_used_at: None,
        last_execution_used_at: None,
    });
    let nl_run_metrics = db::get_nl_run_metrics(limit).unwrap_or(db::NLRunMetrics {
        total: 0,
        completed: 0,
        manual_required: 0,
        approval_required: 0,
        blocked: 0,
        error: 0,
        success_rate: 0.0,
    });
    let exec_approval_metrics =
        db::get_exec_approval_metrics(limit).unwrap_or(db::ExecApprovalMetrics {
            window_size: limit,
            total: 0,
            pending: 0,
            approved: 0,
            rejected: 0,
            expired_pending: 0,
            allow_once: 0,
            allow_always: 0,
            deny: 0,
            approval_rate: 0.0,
            oldest_pending_created_at: None,
            last_created_at: None,
            last_resolved_at: None,
        });
    let recommendation_metrics = get_recommendation_metrics().await.0;
    let recommendation_review_metrics =
        db::get_recommendation_review_metrics(limit).unwrap_or(db::RecommendationReviewMetrics {
            window_size: limit,
            total_events: 0,
            approve_actions: 0,
            reject_actions: 0,
            later_actions: 0,
            restore_actions: 0,
            feedback_positive: 0,
            feedback_refine: 0,
            feedback_negative: 0,
            failed_actions: 0,
            action_failure_rate: 0.0,
            non_positive_feedback_rate: 0.0,
            last_event_at: None,
        });
    let recent_events = db::list_launch_ops_events(limit.min(12)).unwrap_or_default();

    Json(LaunchOpsResponse {
        chat_metrics,
        memory_metrics,
        nl_run_metrics,
        exec_approval_metrics,
        recommendation_metrics,
        recommendation_review_metrics,
        recent_events,
    })
}

fn memory_admin_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(12).clamp(1, 50)
}

fn memory_admin_action_message(ok: bool, kind: &str, action: &str) -> String {
    if ok {
        format!("{} {} succeeded.", kind, action)
    } else {
        format!("{} {} target was not found.", kind, action)
    }
}

fn memory_admin_action_response(
    ok: bool,
    kind: &str,
    action: &str,
) -> (StatusCode, Json<MemoryAdminActionResponse>) {
    let message = memory_admin_action_message(ok, kind, action);
    (
        if ok {
            StatusCode::OK
        } else {
            StatusCode::NOT_FOUND
        },
        Json(MemoryAdminActionResponse {
            ok,
            kind: kind.to_string(),
            action: action.to_string(),
            message,
        }),
    )
}

pub(super) async fn list_memory_records_handler(
    Query(query): Query<MemoryRecordsQuery>,
) -> Json<MemoryRecordsResponse> {
    let limit = memory_admin_limit(query.limit);
    let include_suppressed = query.include_suppressed.unwrap_or(true);
    let metrics = db::get_memory_ops_metrics().unwrap_or(crate::db::MemoryOpsMetrics {
        request_active: 0,
        request_suppressed: 0,
        execution_active: 0,
        execution_suppressed: 0,
        last_request_used_at: None,
        last_execution_used_at: None,
    });
    let request_memory =
        db::list_request_memory_records(limit, include_suppressed).unwrap_or_default();
    let execution_memory =
        db::list_execution_memory_records(limit, include_suppressed).unwrap_or_default();
    let recent_admin_events = db::list_memory_admin_events(limit).unwrap_or_default();
    Json(MemoryRecordsResponse {
        metrics,
        request_memory,
        execution_memory,
        recent_admin_events,
    })
}

pub(super) async fn suppress_request_memory_handler(
    Json(req): Json<RequestMemoryAdminActionRequest>,
) -> (StatusCode, Json<MemoryAdminActionResponse>) {
    let ok = if req.request_text.trim().is_empty() {
        false
    } else {
        db::suppress_request_memory_scoped(
            req.memory_scope.as_deref(),
            &req.request_text,
            req.reason.as_deref(),
        )
        .unwrap_or(false)
    };
    let message = memory_admin_action_message(ok, "request_memory", "suppress");
    let actor = req.actor.as_deref().unwrap_or("api.memory_admin");
    let _ = db::record_memory_admin_event(
        "request_memory",
        "suppress",
        req.memory_scope.as_deref(),
        &req.request_text,
        req.reason.as_deref(),
        Some(actor),
        ok,
        Some(&message),
    );
    memory_admin_action_response(ok, "request_memory", "suppress")
}

pub(super) async fn restore_request_memory_handler(
    Json(req): Json<RequestMemoryAdminActionRequest>,
) -> (StatusCode, Json<MemoryAdminActionResponse>) {
    let ok = if req.request_text.trim().is_empty() {
        false
    } else {
        db::restore_request_memory_scoped(req.memory_scope.as_deref(), &req.request_text)
            .unwrap_or(false)
    };
    let message = memory_admin_action_message(ok, "request_memory", "restore");
    let actor = req.actor.as_deref().unwrap_or("api.memory_admin");
    let _ = db::record_memory_admin_event(
        "request_memory",
        "restore",
        req.memory_scope.as_deref(),
        &req.request_text,
        req.reason.as_deref(),
        Some(actor),
        ok,
        Some(&message),
    );
    memory_admin_action_response(ok, "request_memory", "restore")
}

pub(super) async fn delete_request_memory_handler(
    Json(req): Json<RequestMemoryAdminActionRequest>,
) -> (StatusCode, Json<MemoryAdminActionResponse>) {
    let ok = if req.request_text.trim().is_empty() {
        false
    } else {
        db::delete_request_memory_scoped(req.memory_scope.as_deref(), &req.request_text)
            .unwrap_or(false)
    };
    let message = memory_admin_action_message(ok, "request_memory", "delete");
    let actor = req.actor.as_deref().unwrap_or("api.memory_admin");
    let _ = db::record_memory_admin_event(
        "request_memory",
        "delete",
        req.memory_scope.as_deref(),
        &req.request_text,
        req.reason.as_deref(),
        Some(actor),
        ok,
        Some(&message),
    );
    memory_admin_action_response(ok, "request_memory", "delete")
}

pub(super) async fn suppress_execution_memory_handler(
    Json(req): Json<ExecutionMemoryAdminActionRequest>,
) -> (StatusCode, Json<MemoryAdminActionResponse>) {
    let ok = if req.intent_command.trim().is_empty() || req.params_key.trim().is_empty() {
        false
    } else {
        db::suppress_execution_memory_scoped(
            req.memory_scope.as_deref(),
            &req.intent_command,
            &req.params_key,
            req.reason.as_deref(),
        )
        .unwrap_or(false)
    };
    let message = memory_admin_action_message(ok, "execution_memory", "suppress");
    let actor = req.actor.as_deref().unwrap_or("api.memory_admin");
    let target_key = format!("{}::{}", req.intent_command.trim(), req.params_key.trim());
    let _ = db::record_memory_admin_event(
        "execution_memory",
        "suppress",
        req.memory_scope.as_deref(),
        &target_key,
        req.reason.as_deref(),
        Some(actor),
        ok,
        Some(&message),
    );
    memory_admin_action_response(ok, "execution_memory", "suppress")
}

pub(super) async fn restore_execution_memory_handler(
    Json(req): Json<ExecutionMemoryAdminActionRequest>,
) -> (StatusCode, Json<MemoryAdminActionResponse>) {
    let ok = if req.intent_command.trim().is_empty() || req.params_key.trim().is_empty() {
        false
    } else {
        db::restore_execution_memory_scoped(
            req.memory_scope.as_deref(),
            &req.intent_command,
            &req.params_key,
        )
        .unwrap_or(false)
    };
    let message = memory_admin_action_message(ok, "execution_memory", "restore");
    let actor = req.actor.as_deref().unwrap_or("api.memory_admin");
    let target_key = format!("{}::{}", req.intent_command.trim(), req.params_key.trim());
    let _ = db::record_memory_admin_event(
        "execution_memory",
        "restore",
        req.memory_scope.as_deref(),
        &target_key,
        req.reason.as_deref(),
        Some(actor),
        ok,
        Some(&message),
    );
    memory_admin_action_response(ok, "execution_memory", "restore")
}

pub(super) async fn delete_execution_memory_handler(
    Json(req): Json<ExecutionMemoryAdminActionRequest>,
) -> (StatusCode, Json<MemoryAdminActionResponse>) {
    let ok = if req.intent_command.trim().is_empty() || req.params_key.trim().is_empty() {
        false
    } else {
        db::delete_execution_memory_scoped(
            req.memory_scope.as_deref(),
            &req.intent_command,
            &req.params_key,
        )
        .unwrap_or(false)
    };
    let message = memory_admin_action_message(ok, "execution_memory", "delete");
    let actor = req.actor.as_deref().unwrap_or("api.memory_admin");
    let target_key = format!("{}::{}", req.intent_command.trim(), req.params_key.trim());
    let _ = db::record_memory_admin_event(
        "execution_memory",
        "delete",
        req.memory_scope.as_deref(),
        &target_key,
        req.reason.as_deref(),
        Some(actor),
        ok,
        Some(&message),
    );
    memory_admin_action_response(ok, "execution_memory", "delete")
}
