mod routing;
#[cfg(test)]
mod tests;
mod webhook;

use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_REQUEST_TEXT: &str = "뉴스 5개 요약해서 노션에 정리해줘. 유튜브 링크 포함.";
const DEFAULT_RUNBOOK_ROOT: &str = "/tmp/steer_master_runbook";
const DEFAULT_WEBHOOK_TIMEOUT_SECS: u64 = 180;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDigestTriggerResult {
    pub ok: bool,
    pub status_code: u16,
    pub webhook_url: String,
    pub scope_marker: String,
    pub notion_url: Option<String>,
    pub response_json: Option<Value>,
    pub response_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiDigestProgramRouteKind {
    Explicit,
    Auto,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiDigestProgramRoute {
    pub request_text: String,
    pub kind: AiDigestProgramRouteKind,
}

pub use routing::{
    ai_digest_auto_route_enabled, build_scope_marker, default_request_text,
    extract_explicit_n8n_request, infer_program_route, looks_like_ai_digest_request,
    looks_like_news_digest_request, normalize_request_text, strip_local_execution_prefix,
};
#[cfg(test)]
pub(crate) use webhook::extract_notion_url;
pub use webhook::{
    format_human_summary, resolve_program_webhook_url, trigger_program_webhook,
    trigger_program_webhook_human_summary,
};
