use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use super::support::persist_recovery_event;
use super::types::{AgentRecoveryEventRequest, AgentRecoveryEventResponse};

pub(crate) async fn agent_recovery_event_handler(
    Json(payload): Json<AgentRecoveryEventRequest>,
) -> impl IntoResponse {
    let run_id = payload.run_id.trim().to_string();
    let action_key = payload.action_key.trim().to_string();
    let status = payload.status.trim().to_string();
    if run_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "missing_run_id" })),
        )
            .into_response();
    }
    if action_key.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "missing_action_key" })),
        )
            .into_response();
    }
    if status.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "missing_status" })),
        )
            .into_response();
    }

    let stage_name = payload
        .stage_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("recovery")
        .to_string();
    match persist_recovery_event(
        &run_id,
        &stage_name,
        &action_key,
        &status,
        payload.expected.as_deref(),
        payload.actual.as_deref(),
        payload.details.as_deref(),
    ) {
        Ok(true) => (
            StatusCode::OK,
            Json(json!(AgentRecoveryEventResponse {
                ok: true,
                recorded: true,
                run_id,
                stage_name,
                action_key,
                status,
                recorded_at: chrono::Utc::now().to_rfc3339(),
                reason: None,
            })),
        )
            .into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!(AgentRecoveryEventResponse {
                ok: false,
                recorded: false,
                run_id,
                stage_name,
                action_key,
                status,
                recorded_at: chrono::Utc::now().to_rfc3339(),
                reason: Some("run_not_found".to_string()),
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "record_recovery_failed", "details": err })),
        )
            .into_response(),
    }
}
