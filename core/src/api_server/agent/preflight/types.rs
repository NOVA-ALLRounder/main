use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(crate) struct AgentPreflightCheckItem {
    pub key: String,
    pub label: String,
    pub ok: bool,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub message: String,
}

#[derive(Serialize)]
pub(crate) struct AgentPreflightResponse {
    pub ok: bool,
    pub checks: Vec<AgentPreflightCheckItem>,
    pub active_app: Option<String>,
    pub checked_at: String,
}

#[derive(Deserialize)]
pub(crate) struct AgentPreflightFixRequest {
    pub action: String,
    pub run_id: Option<String>,
    pub stage_name: Option<String>,
    pub assertion_key: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct AgentPreflightFixResponse {
    pub ok: bool,
    pub action: String,
    pub message: String,
    pub active_app: Option<String>,
    pub fixed_at: String,
    pub recorded: bool,
    pub run_id: Option<String>,
    pub stage_name: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct AgentRecoveryEventRequest {
    pub run_id: String,
    pub action_key: String,
    pub status: String,
    pub details: Option<String>,
    pub stage_name: Option<String>,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct AgentRecoveryEventResponse {
    pub ok: bool,
    pub recorded: bool,
    pub run_id: String,
    pub stage_name: String,
    pub action_key: String,
    pub status: String,
    pub recorded_at: String,
    pub reason: Option<String>,
}
