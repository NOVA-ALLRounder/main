mod admin;
mod agent;
mod automation;
mod chat;
mod chat_execution;
mod chat_support;
mod control;
mod diagnostics;
mod operational;
mod recommendations;
mod routes;
mod runtime;
mod server;

pub(crate) use self::agent::evaluate_business_evidence_with_assertions;
pub(crate) use self::automation::run_analysis_internal;
pub(crate) use self::chat::handle_chat;
pub use self::chat::{
    ChatFeedbackRequest, ChatFeedbackResponse, ChatRequest, ChatResponse, ChatRouteMeta,
};
pub use self::chat_support::process_chat_request;
pub(crate) use self::chat_support::{
    load_cached_execution_response, load_cached_request_intent, load_cached_request_response,
};
pub(crate) use self::diagnostics::truncate_log_message;
use crate::llm_gateway;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub llm_client: Option<std::sync::Arc<dyn llm_gateway::LLMClient>>,
    pub current_goal: Arc<Mutex<Option<String>>>,
}

pub(crate) use self::runtime::{
    inflight_agent_executions, log_verification_run, telegram_listener_started_flag,
    API_SERVER_STARTED_AT,
};
pub use self::runtime::{try_spawn_telegram_listener, TelegramListenerStartOutcome};
pub use self::server::{build_api_router, start_api_server};

#[cfg(test)]
mod tests;
