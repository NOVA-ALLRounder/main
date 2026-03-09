use serde::{Deserialize, Serialize};

mod context;
mod contract;
mod execution;
mod planning;
mod preflight;
pub(crate) use self::context::{
    delete_session_handler, get_selection_context, get_session_handler, handle_feedback,
    list_sessions_handler, resume_session_handler,
};
pub(crate) use self::contract::{
    agent_approve_handler, agent_verify_handler, evaluate_business_evidence_with_assertions,
};
#[cfg(test)]
pub(crate) use self::contract::{compute_completion_score, evaluate_business_evidence};
pub(crate) use self::execution::agent_execute_handler;
#[cfg(test)]
pub(crate) use self::execution::parse_resume_token;
pub(crate) use self::planning::{
    agent_intent_handler, agent_plan_handler, execute_goal_handler, get_current_goal,
    run_goal_sync_handler,
};
pub(crate) use self::preflight::{
    agent_preflight_fix_handler, agent_preflight_handler, agent_recovery_event_handler,
};
#[derive(Deserialize)]
pub struct ExecApprovalQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct ApprovalPolicyQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct NLRunMetricsQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct ApprovalPolicyRequest {
    pub policy_key: String,
    pub decision: String,
}

#[derive(Serialize)]
pub struct ApprovalPolicyResponse {
    pub policy_key: String,
    pub decision: String,
    pub updated_at: String,
}

#[derive(Deserialize, Default)]
pub struct ExecApprovalResolve {
    pub resolved_by: Option<String>,
    pub decision: Option<String>,
}

#[derive(Deserialize)]
pub struct RoutineRunsQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct ExecAllowlistRequest {
    pub pattern: String,
    pub cwd: Option<String>,
}

#[derive(Deserialize)]
pub struct ExecAllowlistQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct ExecResultsQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct VerificationRunsQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct NLRunQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct TaskRunsQuery {
    pub limit: Option<i64>,
    pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct CollectorHandoffReceiptsQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct WorkflowProvisionOpsQuery {
    pub limit: Option<i64>,
    pub status: Option<String>,
    pub recommendation_id: Option<i64>,
}

fn env_truthy_default(name: &str, default_value: bool) -> bool {
    match std::env::var(name) {
        Ok(raw) => matches!(raw.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"),
        Err(_) => default_value,
    }
}
