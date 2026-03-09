mod response;
mod telemetry;

#[derive(Default, Clone, Copy)]
pub(crate) struct ChatOpsFlags {
    pub freshness_bypassed: bool,
    pub intent_memory_hit: bool,
    pub request_memory_hit: bool,
    pub execution_memory_hit: bool,
    pub deterministic_used: bool,
    pub llm_used: bool,
    pub ai_digest_used: bool,
    pub local_only: bool,
}

pub use self::response::process_chat_request;
pub(crate) use self::response::{respond_chat, run_ai_digest_chat_response};
pub(crate) use self::telemetry::chat_ops_outcome_from_response;
