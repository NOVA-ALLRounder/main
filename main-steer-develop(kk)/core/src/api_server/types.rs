use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::{llm_gateway, quality_scorer, runtime_verification};

#[derive(Clone)]
pub struct AppState {
    pub llm_client: Option<std::sync::Arc<dyn llm_gateway::LLMClient>>,
    pub current_goal: Arc<Mutex<Option<String>>>,
    pub runtime_control: Arc<Mutex<crate::runtime_mode::RuntimeControl>>,
}

#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub channel: Option<String>,
    pub chat_type: Option<String>,
    pub sender: Option<String>,
    pub mentioned: Option<bool>,
}

#[derive(Deserialize)]
pub struct JarvisCommandRequest {
    pub text: String,
}

#[derive(Deserialize)]
pub struct JarvisWebMessageRequest {
    pub text: String,
    pub session_key: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
pub struct JarvisSkillExecuteRequest {
    pub action: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct VersionResponse {
    pub core_version: String,
    pub build_profile: String,
    pub git_sha: Option<String>,
}

#[derive(Serialize)]
pub struct RuntimeModeResponse {
    pub mode: String,
    pub emergency_stop: bool,
    pub allow_automation: bool,
}

#[derive(Serialize)]
pub struct PreflightResponse {
    pub ok: bool,
    pub api_port: u16,
    pub api_reachable: bool,
    pub env_present: bool,
    pub steer_home_writable: bool,
    pub release_dir_writable: bool,
    pub gmail_credentials_set: bool,
    pub notion_ready: bool,
    pub operation_mode: String,
    pub emergency_stop: bool,
    pub allow_automation: bool,
    pub notes: Vec<String>,
}

#[derive(Deserialize)]
pub struct SetRuntimeModeRequest {
    pub mode: String,
}

#[derive(Deserialize)]
pub struct EmergencyStopRequest {
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct FeedbackRequest {
    pub goal: String,
    pub feedback: String,
    pub history_summary: Option<String>,
}

#[derive(Serialize)]
pub struct FeedbackResponse {
    pub action: String,
    pub new_goal: Option<String>,
    pub message: String,
}

#[derive(Deserialize)]
pub struct AgentIntentRequest {
    pub text: String,
}

#[derive(Serialize)]
pub struct AgentIntentResponse {
    pub session_id: String,
    pub intent: String,
    pub confidence: f32,
    pub slots: HashMap<String, String>,
    pub missing_slots: Vec<String>,
    pub follow_up: Option<String>,
}

#[derive(Deserialize)]
pub struct AgentPlanRequest {
    pub session_id: String,
    pub slots: Option<HashMap<String, String>>,
}

#[derive(Serialize)]
pub struct AgentPlanResponse {
    pub plan_id: String,
    pub intent: String,
    pub steps: Vec<crate::nl_automation::PlanStep>,
    pub missing_slots: Vec<String>,
}

#[derive(Deserialize)]
pub struct AgentExecuteRequest {
    pub plan_id: String,
}

#[derive(Serialize)]
pub struct AgentExecuteResponse {
    pub status: String,
    pub logs: Vec<String>,
    pub approval: Option<crate::nl_automation::ApprovalContext>,
    #[serde(default)]
    pub manual_steps: Vec<String>,
    pub resume_from: Option<usize>,
}

#[derive(Deserialize)]
pub struct AgentVerifyRequest {
    pub plan_id: String,
}

#[derive(Serialize)]
pub struct AgentVerifyResponse {
    pub ok: bool,
    pub issues: Vec<String>,
}

#[derive(Deserialize)]
pub struct AgentApproveRequest {
    pub plan_id: String,
    pub action: String,
    pub decision: Option<String>,
}

#[derive(Serialize)]
pub struct AgentApproveResponse {
    pub status: String,
    pub requires_approval: bool,
    pub message: String,
    pub risk_level: String,
    pub policy: String,
}

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
pub struct ProjectScanQuery {
    pub max_files: Option<usize>,
    pub workdir: Option<String>,
}

#[derive(Serialize)]
pub struct ProjectScanResponse {
    pub project_type: String,
    pub files: Vec<String>,
    pub key_files: std::collections::HashMap<String, String>,
}

#[derive(Deserialize)]
pub struct RuntimeVerifyRequest {
    pub workdir: Option<String>,
    pub run_backend: Option<bool>,
    pub run_frontend: Option<bool>,
    pub run_e2e: Option<bool>,
    pub run_build_checks: Option<bool>,
    pub backend_port: Option<u16>,
    pub frontend_port: Option<u16>,
    pub backend_health_path: Option<String>,
}

#[derive(Deserialize)]
pub struct QualityScoreRequest {
    pub runtime: Option<runtime_verification::RuntimeVerifyResult>,
    pub runtime_options: Option<RuntimeVerifyRequest>,
    pub code_review: Option<quality_scorer::CodeReviewInput>,
    pub goal: Option<String>,
    pub use_llm: Option<bool>,
}

#[derive(Serialize)]
pub struct QualityScoreResponse {
    pub created_at: String,
    pub score: quality_scorer::QualityScore,
}

#[derive(Deserialize)]
pub struct SemanticVerifyRequest {
    pub workdir: Option<String>,
    pub max_files: Option<usize>,
}

#[derive(Deserialize)]
pub struct PerformanceVerifyRequest {
    pub workdir: Option<String>,
    pub max_files: Option<usize>,
}

#[derive(Serialize)]
pub struct ChatResponse {
    pub response: String,
    pub command: Option<String>,
}

#[derive(Serialize)]
pub struct RecommendationItem {
    pub id: i64,
    pub status: String,
    pub title: String,
    pub summary: String,
    pub confidence: f64,
    pub evidence: Vec<String>,
    pub last_error: Option<String>,
}

#[derive(Serialize)]
pub struct QualityMetrics {
    pub total: u32,
    pub success: u32,
    pub rate: f64,
}

#[derive(Deserialize)]
pub struct CreateRoutineRequest {
    pub name: String,
    #[serde(alias = "cron_expression")]
    pub cron: String,
    pub prompt: String,
}

#[derive(Deserialize)]
pub struct ToggleRoutineRequest {
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct RecQueryParams {
    pub status: Option<String>,
}

#[derive(Serialize)]
pub struct RecommendationMetricsResponse {
    pub total: i64,
    pub approved: i64,
    pub rejected: i64,
    pub failed: i64,
    pub pending: i64,
    pub later: i64,
    pub approval_rate: f64,
    pub last_created_at: Option<String>,
}

#[derive(Deserialize)]
pub struct GoalRequest {
    pub goal: String,
}
