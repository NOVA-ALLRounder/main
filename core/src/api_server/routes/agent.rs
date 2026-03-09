use axum::{
    routing::{get, post},
    Router,
};

use super::super::agent::{
    agent_approve_handler, agent_execute_handler, agent_intent_handler, agent_plan_handler,
    agent_preflight_fix_handler, agent_preflight_handler, agent_recovery_event_handler,
    agent_verify_handler, delete_session_handler, execute_goal_handler, get_current_goal,
    get_selection_context, get_session_handler, handle_feedback, list_sessions_handler,
    resume_session_handler, run_goal_sync_handler,
};
use super::super::AppState;

pub(crate) fn build_agent_routes() -> Router<AppState> {
    Router::new()
        .route("/api/agent/intent", post(agent_intent_handler))
        .route("/api/agent/plan", post(agent_plan_handler))
        .route("/api/agent/execute", post(agent_execute_handler))
        .route("/api/agent/verify", post(agent_verify_handler))
        .route("/api/agent/approve", post(agent_approve_handler))
        .route("/api/agent/preflight", get(agent_preflight_handler))
        .route(
            "/api/agent/preflight/fix",
            post(agent_preflight_fix_handler),
        )
        .route(
            "/api/agent/recovery-event",
            post(agent_recovery_event_handler),
        )
        .route("/api/agent/goal", post(execute_goal_handler))
        .route("/api/agent/goal/run", post(run_goal_sync_handler))
        .route("/api/agent/goal/current", get(get_current_goal))
        .route("/api/agent/feedback", post(handle_feedback))
        .route("/api/context/selection", get(get_selection_context))
        .route("/api/sessions", get(list_sessions_handler))
        .route(
            "/api/sessions/:id",
            get(get_session_handler).delete(delete_session_handler),
        )
        .route("/api/sessions/:id/resume", post(resume_session_handler))
}
