use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize)]
pub(crate) struct AgentIntentRequest {
    pub text: String,
}

#[derive(Serialize)]
pub(crate) struct AgentIntentResponse {
    pub session_id: String,
    pub intent: String,
    pub confidence: f32,
    pub slots: HashMap<String, String>,
    pub missing_slots: Vec<String>,
    pub follow_up: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct AgentPlanRequest {
    pub session_id: String,
    pub slots: Option<HashMap<String, String>>,
}

#[derive(Serialize)]
pub(crate) struct AgentPlanResponse {
    pub plan_id: String,
    pub intent: String,
    pub steps: Vec<crate::nl_automation::PlanStep>,
    pub missing_slots: Vec<String>,
}

#[derive(Deserialize)]
pub(crate) struct GoalRequest {
    pub(crate) goal: String,
}

#[derive(Deserialize)]
pub(crate) struct RunGoalRequest {
    pub(crate) goal: String,
    pub(crate) session_key: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct RunGoalResponse {
    pub run_id: String,
    pub planner_complete: bool,
    pub execution_complete: bool,
    pub business_complete: bool,
    pub status: String,
    pub summary: Option<String>,
}
