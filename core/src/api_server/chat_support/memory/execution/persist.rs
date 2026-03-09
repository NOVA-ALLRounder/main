use serde_json::Value;

use crate::{db, request_memory};

use super::policy::{
    execution_memory_params, execution_memory_should_store, execution_memory_tool_path,
    execution_memory_ttl_secs,
};

pub(crate) fn persist_execution_memory(
    memory_scope: Option<&str>,
    message: &str,
    command: &str,
    intent: &Value,
    response: &str,
    source: &str,
) {
    let confidence = intent["confidence"].as_f64().unwrap_or(0.0);
    if !execution_memory_should_store(command, response, confidence) {
        return;
    }

    let Some((params_key, params_json)) = execution_memory_params(command, intent) else {
        return;
    };
    let Some(ttl_secs) = execution_memory_ttl_secs(command) else {
        return;
    };
    let request_signature = request_memory::build_request_signature(message);
    let signature = if request_signature.trim().is_empty() {
        None
    } else {
        Some(request_signature.as_str())
    };
    let _ = db::upsert_execution_memory_scoped(
        memory_scope,
        command,
        &params_key,
        Some(&params_json),
        signature,
        response,
        ttl_secs,
        source,
        execution_memory_tool_path(command),
    );
}
