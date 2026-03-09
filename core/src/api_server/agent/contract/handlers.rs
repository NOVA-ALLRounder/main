use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{approval_gate, nl_store, verification_engine};

#[derive(Deserialize)]
pub(crate) struct AgentVerifyRequest {
    pub plan_id: String,
}

#[derive(Serialize)]
pub(crate) struct AgentVerifyResponse {
    pub ok: bool,
    pub issues: Vec<String>,
}

#[derive(Deserialize)]
pub(crate) struct AgentApproveRequest {
    pub plan_id: String,
    pub action: String,
    pub decision: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct AgentApproveResponse {
    pub status: String,
    pub requires_approval: bool,
    pub message: String,
    pub risk_level: String,
    pub policy: String,
}

pub(crate) async fn agent_verify_handler(
    Json(payload): Json<AgentVerifyRequest>,
) -> impl IntoResponse {
    let Some(plan) = nl_store::get_plan(&payload.plan_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "plan_not_found" })),
        )
            .into_response();
    };

    let result = verification_engine::verify_plan(&plan);
    let response = AgentVerifyResponse {
        ok: result.ok,
        issues: result.issues,
    };

    (StatusCode::OK, Json(response)).into_response()
}

pub(crate) async fn agent_approve_handler(
    Json(payload): Json<AgentApproveRequest>,
) -> impl IntoResponse {
    let Some(plan) = nl_store::get_plan(&payload.plan_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "plan_not_found" })),
        )
            .into_response();
    };

    if let Some(decision) = payload.decision.as_deref() {
        approval_gate::register_decision(decision, &payload.action, &plan);
    }
    let decision = approval_gate::preview_approval(&payload.action, &plan);
    let response = AgentApproveResponse {
        status: decision.status,
        requires_approval: decision.requires_approval,
        message: decision.message,
        risk_level: decision.risk_level,
        policy: decision.policy,
    };

    (StatusCode::OK, Json(response)).into_response()
}
