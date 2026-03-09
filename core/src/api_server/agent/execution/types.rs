use serde::{Deserialize, Serialize};

use super::super::contract::{AgentCompletionScore, AgentStageDodCheck};
use super::support::AgentExecutionGuard;
use crate::{
    execution_controller::ExecutionOptions,
    nl_automation::{ExecutionResult, Plan, VerificationResult},
    nl_store::SessionState,
};

#[derive(Deserialize)]
pub(crate) struct AgentExecuteRequest {
    pub plan_id: String,
    pub profile: Option<AgentExecutionProfile>,
    #[serde(default)]
    pub resume_from: Option<usize>,
    #[serde(default)]
    pub resume_token: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub(crate) enum AgentExecutionProfile {
    #[default]
    Strict,
    Test,
    Fast,
}

impl AgentExecutionProfile {
    pub(super) fn as_str(&self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Test => "test",
            Self::Fast => "fast",
        }
    }

    pub(super) fn execution_options(&self) -> crate::execution_controller::ExecutionOptions {
        match self {
            Self::Strict => crate::execution_controller::ExecutionOptions::strict(),
            Self::Test => crate::execution_controller::ExecutionOptions::test(),
            Self::Fast => crate::execution_controller::ExecutionOptions::fast(),
        }
    }

    pub(super) fn default_auto_replan_enabled(&self) -> bool {
        matches!(self, Self::Test)
    }
}

#[derive(Serialize)]
pub(crate) struct AgentExecuteResponse {
    pub status: String,
    pub logs: Vec<String>,
    pub approval: Option<crate::nl_automation::ApprovalContext>,
    #[serde(default)]
    pub manual_steps: Vec<String>,
    pub resume_from: Option<usize>,
    #[serde(default)]
    pub resume_token: Option<String>,
    pub run_id: Option<String>,
    #[serde(default)]
    pub planner_complete: bool,
    #[serde(default)]
    pub execution_complete: bool,
    #[serde(default)]
    pub business_complete: bool,
    #[serde(default)]
    pub completion_score: Option<AgentCompletionScore>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub collision_policy: Option<String>,
    #[serde(default)]
    pub stage_dod: Vec<AgentStageDodCheck>,
}

#[derive(Debug, Clone)]
pub(crate) struct ParsedResumeToken {
    pub(crate) plan_id: String,
    pub(crate) step_index: usize,
    pub(crate) reason: String,
}

pub(super) struct AgentExecutionContext {
    pub(super) plan: Plan,
    pub(super) session: Option<SessionState>,
    pub(super) run_id: String,
    pub(super) resume_from: usize,
    pub(super) resume_hint: Option<String>,
    pub(super) execution_profile: AgentExecutionProfile,
    pub(super) execution_options: ExecutionOptions,
    pub(super) _exec_guard: AgentExecutionGuard,
}

pub(super) struct AgentExecutionRuntime {
    pub(super) result: ExecutionResult,
    pub(super) verify: VerificationResult,
}

pub(super) struct AgentExecutionBusinessState {
    pub(super) planner_complete: bool,
    pub(super) execution_complete: bool,
    pub(super) verification_ok: bool,
    pub(super) evidence_ok: bool,
    pub(super) evidence_detail: String,
    pub(super) business_complete: bool,
}

pub(super) struct AgentExecutionFinalization {
    pub(super) final_nl_status: String,
    pub(super) completion_score: AgentCompletionScore,
    pub(super) stage_dod: Vec<AgentStageDodCheck>,
}
