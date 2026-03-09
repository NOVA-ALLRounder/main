use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::{intent_router, nl_store, plan_builder, slot_filler};

use super::types::{AgentIntentRequest, AgentIntentResponse, AgentPlanRequest, AgentPlanResponse};

pub(crate) async fn agent_intent_handler(
    Json(payload): Json<AgentIntentRequest>,
) -> impl IntoResponse {
    let intent_result = intent_router::classify_intent(&payload.text);
    let fill = slot_filler::fill_slots(&intent_result.intent, intent_result.slots.clone());
    let session = nl_store::create_session(
        intent_result.clone(),
        fill.slots.clone(),
        payload.text.clone(),
    );

    let response = AgentIntentResponse {
        session_id: session.session_id,
        intent: intent_result.intent.as_str().to_string(),
        confidence: intent_result.confidence,
        slots: fill.slots,
        missing_slots: fill.missing,
        follow_up: fill.follow_up,
    };

    (StatusCode::OK, Json(response)).into_response()
}

pub(crate) async fn agent_plan_handler(Json(payload): Json<AgentPlanRequest>) -> impl IntoResponse {
    let Some(mut session) = nl_store::get_session(&payload.session_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "session_not_found" })),
        )
            .into_response();
    };

    if let Some(updates) = payload.slots.as_ref() {
        if let Some(updated) = nl_store::update_session_slots(&payload.session_id, updates) {
            session = updated;
        }
    }

    let fill = slot_filler::fill_slots(&session.intent.intent, session.slots.clone());
    let plan = plan_builder::build_plan(&session.intent.intent, &fill.slots);
    let _ = nl_store::set_session_plan(&payload.session_id, plan.clone());

    let response = AgentPlanResponse {
        plan_id: plan.plan_id,
        intent: session.intent.intent.as_str().to_string(),
        steps: plan.steps,
        missing_slots: fill.missing,
    };

    (StatusCode::OK, Json(response)).into_response()
}
