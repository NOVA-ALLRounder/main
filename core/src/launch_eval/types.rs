use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::api_server::ChatRequest;
use crate::recommendation::AutomationProposal;

use super::seeding::build_memory_scope;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalConfig {
    #[serde(default = "default_report_dir")]
    pub report_dir: String,
    #[serde(default)]
    pub include_paths: Vec<String>,
    #[serde(default)]
    pub scenarios: Vec<LaunchEvalScenario>,
}

pub(crate) fn default_report_dir() -> String {
    "reports/launch_eval".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LaunchEvalScenario {
    Chat {
        id: String,
        #[serde(default)]
        description: Option<String>,
        request: LaunchEvalChatInput,
        #[serde(default)]
        ai_digest_mock: Option<AiDigestMock>,
        expect: LaunchEvalChatExpectation,
    },
    RequestMemoryReuse {
        id: String,
        #[serde(default)]
        description: Option<String>,
        seed: RequestMemorySeed,
        request: LaunchEvalChatInput,
        expect: LaunchEvalChatExpectation,
    },
    ExecutionMemoryReuse {
        id: String,
        #[serde(default)]
        description: Option<String>,
        seed: ExecutionMemorySeed,
        request: LaunchEvalChatInput,
        expect: LaunchEvalChatExpectation,
    },
    RequestMemoryPolicy {
        id: String,
        #[serde(default)]
        description: Option<String>,
        seed: RequestMemorySeed,
        request: LaunchEvalChatInput,
        #[serde(default = "default_request_memory_policy_mode")]
        mode: RequestMemoryPolicyMode,
        #[serde(default)]
        feedback: Option<String>,
        #[serde(default)]
        suppress: bool,
        expect_cached: bool,
        #[serde(default)]
        expect_command: Option<String>,
        #[serde(default)]
        expect_response_contains: Vec<String>,
    },
    ExecutionMemoryPolicy {
        id: String,
        #[serde(default)]
        description: Option<String>,
        seed: ExecutionMemorySeed,
        request: LaunchEvalChatInput,
        #[serde(default)]
        lookup_params: Option<Value>,
        #[serde(default)]
        feedback: Option<String>,
        #[serde(default)]
        suppress: bool,
        expect_cached: bool,
        #[serde(default)]
        expect_response_contains: Vec<String>,
    },
    BusinessContract {
        id: String,
        #[serde(default)]
        description: Option<String>,
        plan: LaunchEvalBusinessPlan,
        logs: Vec<String>,
        expect_ok: bool,
        #[serde(default)]
        expect_detail_contains: Vec<String>,
        #[serde(default)]
        expect_assertions: Vec<LaunchEvalAssertionExpectation>,
    },
    MemoryScopeIsolation {
        id: String,
        #[serde(default)]
        description: Option<String>,
        seed: ScopedRequestMemorySeed,
        request: LaunchEvalChatInput,
        expect: LaunchEvalChatExpectation,
    },
    RecommendationGate {
        id: String,
        #[serde(default)]
        description: Option<String>,
        stage: RecommendationGateStage,
        proposal: AutomationProposal,
        expect_ready: bool,
        #[serde(default)]
        expect_reasons_contains: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalChatInput {
    pub message: String,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub chat_type: Option<String>,
    #[serde(default)]
    pub sender: Option<String>,
    #[serde(default)]
    pub mentioned: Option<bool>,
}

impl LaunchEvalChatInput {
    pub(crate) fn as_request(&self) -> ChatRequest {
        ChatRequest {
            message: self.message.clone(),
            channel: self.channel.clone(),
            chat_type: self.chat_type.clone(),
            sender: self.sender.clone(),
            mentioned: self.mentioned,
        }
    }

    pub(crate) fn memory_scope(&self) -> Option<String> {
        build_memory_scope(
            self.channel.as_deref(),
            self.chat_type.as_deref(),
            self.sender.as_deref(),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalChatExpectation {
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub response_contains: Vec<String>,
    #[serde(default)]
    pub response_contains_any: Vec<String>,
    #[serde(default)]
    pub response_not_contains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDigestMock {
    #[serde(default = "default_ai_digest_status")]
    pub status: String,
    #[serde(default)]
    pub notion_url: Option<String>,
    #[serde(default)]
    pub top_headlines_text: Option<String>,
}

fn default_ai_digest_status() -> String {
    "ok".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMemorySeed {
    pub request_text: String,
    pub command: String,
    #[serde(default = "default_json_object")]
    pub params: Value,
    pub response_text: String,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    #[serde(default = "default_request_memory_source")]
    pub source: String,
    #[serde(default)]
    pub response_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMemorySeed {
    pub original_request: String,
    pub command: String,
    #[serde(default = "default_json_object")]
    pub params: Value,
    pub response_text: String,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    #[serde(default = "default_execution_memory_source")]
    pub source: String,
    #[serde(default)]
    pub ttl_seconds: Option<i64>,
    #[serde(default)]
    pub tool_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedRequestMemorySeed {
    pub request_text: String,
    pub command: String,
    #[serde(default = "default_json_object")]
    pub params: Value,
    pub response_text: String,
    pub scope: LaunchEvalScope,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    #[serde(default = "default_request_memory_source")]
    pub source: String,
    #[serde(default)]
    pub response_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalScope {
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub chat_type: Option<String>,
    #[serde(default)]
    pub sender: Option<String>,
}

impl LaunchEvalScope {
    pub(crate) fn memory_scope(&self) -> Option<String> {
        build_memory_scope(
            self.channel.as_deref(),
            self.chat_type.as_deref(),
            self.sender.as_deref(),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationGateStage {
    AutoQueue,
    Approval,
}

impl RecommendationGateStage {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::AutoQueue => "auto_queue",
            Self::Approval => "approval",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalBusinessPlan {
    pub intent: crate::nl_automation::IntentType,
    #[serde(default)]
    pub descriptions: Vec<String>,
    #[serde(default)]
    pub slots: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalAssertionExpectation {
    pub key: String,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestMemoryPolicyMode {
    Response,
    Intent,
}

impl RequestMemoryPolicyMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Response => "response",
            Self::Intent => "intent",
        }
    }
}

fn default_request_memory_policy_mode() -> RequestMemoryPolicyMode {
    RequestMemoryPolicyMode::Response
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalReport {
    pub generated_at: String,
    pub config_path: Option<String>,
    pub db_path: String,
    pub report_json_path: String,
    pub report_markdown_path: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<LaunchEvalCaseResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalCaseResult {
    pub id: String,
    pub kind: String,
    pub description: Option<String>,
    pub passed: bool,
    pub errors: Vec<String>,
    pub notes: Vec<String>,
    pub command: Option<String>,
    pub response_preview: Option<String>,
    pub readiness: Option<LaunchEvalReadinessReport>,
    pub admission: Option<LaunchEvalAdmissionReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalReadinessReport {
    pub ready: bool,
    pub reasons: Vec<String>,
    pub category: String,
    pub business_score: f64,
    pub confidence: f64,
    pub occurrences: Option<u32>,
    pub distinct_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalAdmissionReport {
    pub accepted: bool,
    pub reasons: Vec<String>,
    pub priority_score: f64,
    pub pending_same_category: usize,
    pub pending_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalCandidate {
    pub id: String,
    #[serde(default = "default_real_provenance")]
    pub provenance: String,
    pub source_kind: String,
    pub scenario_kind: String,
    pub title: String,
    pub score: f64,
    pub command: Option<String>,
    pub request_message: String,
    pub rationale: Vec<String>,
    pub yaml: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalCandidateSnapshot {
    pub generated_at: String,
    pub output_path: String,
    #[serde(default = "default_real_provenance")]
    pub provenance_filter: String,
    pub scenario_count: usize,
    pub candidate_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvalCandidateSnapshotInfo {
    pub output_path: String,
    pub exists: bool,
    #[serde(default = "default_real_provenance")]
    pub provenance_filter: String,
    pub scenario_count: usize,
    pub updated_at: Option<String>,
    pub scenario_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchEvalCandidateProvenanceFilter {
    Real,
    Synthetic,
    All,
}

impl LaunchEvalCandidateProvenanceFilter {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Real => "real",
            Self::Synthetic => "synthetic",
            Self::All => "all",
        }
    }
}

pub fn parse_launch_eval_candidate_provenance_filter(
    raw: Option<&str>,
) -> LaunchEvalCandidateProvenanceFilter {
    match raw.unwrap_or("real").trim().to_ascii_lowercase().as_str() {
        "synthetic" | "dogfood" => LaunchEvalCandidateProvenanceFilter::Synthetic,
        "all" | "mixed" => LaunchEvalCandidateProvenanceFilter::All,
        _ => LaunchEvalCandidateProvenanceFilter::Real,
    }
}

pub(crate) fn default_json_object() -> Value {
    json!({})
}

fn default_real_provenance() -> String {
    "real".to_string()
}

fn default_confidence() -> f64 {
    0.95
}

fn default_request_memory_source() -> String {
    "launch.eval.request_seed".to_string()
}

fn default_execution_memory_source() -> String {
    "launch.eval.execution_seed".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LaunchEvalScenarioSnippet {
    pub scenarios: Vec<LaunchEvalScenario>,
}
