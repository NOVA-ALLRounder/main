use super::super::support::truncate_preview;
use super::super::*;
use crate::api_server::{AppState, ChatResponse};
use std::sync::{Arc, Mutex};

pub(super) async fn evaluate_chat_case(
    id: &str,
    kind: &str,
    description: &Option<String>,
    request: &LaunchEvalChatInput,
    expect: &LaunchEvalChatExpectation,
) -> LaunchEvalCaseResult {
    let response =
        crate::api_server::process_chat_request(default_state(), request.as_request()).await;
    let errors = evaluate_chat_expectation(expect, &response);
    LaunchEvalCaseResult {
        id: id.to_string(),
        kind: kind.to_string(),
        description: description.clone(),
        passed: errors.is_empty(),
        errors,
        notes: vec![format!(
            "scope={}",
            request
                .memory_scope()
                .unwrap_or_else(|| "global".to_string())
        )],
        command: response.command.clone(),
        response_preview: Some(truncate_preview(&response.response, 220)),
        readiness: None,
        admission: None,
    }
}

pub(super) fn default_state() -> AppState {
    AppState {
        llm_client: None,
        current_goal: Arc::new(Mutex::new(None)),
    }
}

pub(super) fn evaluate_chat_expectation(
    expect: &LaunchEvalChatExpectation,
    response: &ChatResponse,
) -> Vec<String> {
    let mut errors = Vec::new();
    if response.command != expect.command {
        errors.push(format!(
            "expected command {:?}, got {:?}",
            expect.command, response.command
        ));
    }
    for needle in &expect.response_contains {
        if !response.response.contains(needle) {
            errors.push(format!("response missing '{}'", needle));
        }
    }
    if !expect.response_contains_any.is_empty()
        && !expect
            .response_contains_any
            .iter()
            .any(|needle| response.response.contains(needle))
    {
        errors.push(format!(
            "response missing any of {:?}",
            expect.response_contains_any
        ));
    }
    for needle in &expect.response_not_contains {
        if response.response.contains(needle) {
            errors.push(format!("response unexpectedly contains '{}'", needle));
        }
    }
    errors
}
