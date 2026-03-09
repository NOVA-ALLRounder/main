use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone)]
pub struct ChatRequest {
    pub message: String,
    pub channel: Option<String>,
    pub chat_type: Option<String>,
    pub sender: Option<String>,
    pub mentioned: Option<bool>,
}

#[derive(Deserialize)]
pub struct ChatFeedbackRequest {
    pub request_text: String,
    pub response_text: String,
    pub command: Option<String>,
    pub sentiment: String,
    pub channel: Option<String>,
    pub chat_type: Option<String>,
    pub sender: Option<String>,
}

#[derive(Serialize)]
pub struct ChatFeedbackResponse {
    pub ok: bool,
    pub request_memory_updated: bool,
    pub execution_memory_updated: bool,
    pub reuse_suppressed: bool,
}

#[derive(Serialize)]
pub struct ChatRouteMeta {
    pub route_kind: String,
    pub outcome: String,
    pub confidence: Option<f64>,
    pub note: Option<String>,
    pub memory_scope: Option<String>,
    pub freshness_bypassed: bool,
    pub intent_memory_hit: bool,
    pub request_memory_hit: bool,
    pub execution_memory_hit: bool,
    pub deterministic_used: bool,
    pub llm_used: bool,
    pub ai_digest_used: bool,
    pub local_only: bool,
}

#[derive(Serialize)]
pub struct ChatResponse {
    pub response: String,
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_meta: Option<ChatRouteMeta>,
}
