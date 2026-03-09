use super::super::support::truncate_preview;
use super::super::*;
use crate::launch_eval::seeding::{
    infer_execution_params_key, seed_execution_memory, seed_request_memory,
};
use serde_json::json;

pub(super) fn evaluate_request_memory_policy_case(
    id: &str,
    description: &Option<String>,
    seed: &RequestMemorySeed,
    request: &LaunchEvalChatInput,
    mode: &RequestMemoryPolicyMode,
    feedback: Option<&str>,
    suppress: bool,
    expect_cached: bool,
    expect_command: Option<&str>,
    expect_response_contains: &[String],
) -> LaunchEvalCaseResult {
    let memory_scope = request.memory_scope();
    let memory_scope_ref = memory_scope.as_deref();
    let mut errors = Vec::new();
    let mut notes = vec![format!(
        "scope={}",
        memory_scope.clone().unwrap_or_else(|| "global".to_string())
    )];

    if let Err(error) = seed_request_memory(seed, memory_scope_ref) {
        return LaunchEvalCaseResult {
            id: id.to_string(),
            kind: "request_memory_policy".to_string(),
            description: description.clone(),
            passed: false,
            errors: vec![error.to_string()],
            notes,
            command: None,
            response_preview: None,
            readiness: None,
            admission: None,
        };
    }

    if let Some(sentiment) = feedback {
        notes.push(format!("feedback={}", sentiment));
        if let Err(error) = crate::db::record_request_memory_feedback_scoped(
            memory_scope_ref,
            &seed.request_text,
            &seed.response_text,
            sentiment,
        ) {
            errors.push(format!(
                "failed to apply request memory feedback: {}",
                error
            ));
        }
    }
    if suppress {
        notes.push("suppressed=true".to_string());
        if let Err(error) = crate::db::suppress_request_memory_scoped(
            memory_scope_ref,
            &seed.request_text,
            Some("launch eval policy suppression"),
        ) {
            errors.push(format!("failed to suppress request memory: {}", error));
        }
    }

    let (command, response_preview) = match mode {
        RequestMemoryPolicyMode::Response => {
            let cached = crate::api_server::load_cached_request_response(
                memory_scope_ref,
                &request.message,
                &seed.command,
            );
            if cached.is_some() != expect_cached {
                errors.push(format!(
                    "expected cached response={}, got {}",
                    expect_cached,
                    cached.is_some()
                ));
            }
            if let Some(response) = cached.as_ref() {
                for needle in expect_response_contains {
                    if !response.contains(needle) {
                        errors.push(format!("cached response missing '{}'", needle));
                    }
                }
            }
            notes.push("mode=response".to_string());
            (
                Some(seed.command.clone()),
                cached.map(|value| truncate_preview(&value, 220)),
            )
        }
        RequestMemoryPolicyMode::Intent => {
            let cached =
                crate::api_server::load_cached_request_intent(memory_scope_ref, &request.message);
            if cached.is_some() != expect_cached {
                errors.push(format!(
                    "expected cached intent={}, got {}",
                    expect_cached,
                    cached.is_some()
                ));
            }
            if let Some((intent, confidence)) = cached.as_ref() {
                if let Some(expected) = expect_command {
                    if intent["command"].as_str() != Some(expected) {
                        errors.push(format!(
                            "expected cached intent command {:?}, got {:?}",
                            expected,
                            intent["command"].as_str()
                        ));
                    }
                }
                notes.push(format!("mode=intent confidence={:.2}", confidence));
                (
                    intent["command"].as_str().map(|value| value.to_string()),
                    Some(truncate_preview(&intent.to_string(), 220)),
                )
            } else {
                notes.push("mode=intent".to_string());
                (None, None)
            }
        }
    };

    LaunchEvalCaseResult {
        id: id.to_string(),
        kind: "request_memory_policy".to_string(),
        description: description.clone(),
        passed: errors.is_empty(),
        errors,
        notes,
        command,
        response_preview,
        readiness: None,
        admission: None,
    }
}

pub(super) fn evaluate_execution_memory_policy_case(
    id: &str,
    description: &Option<String>,
    seed: &ExecutionMemorySeed,
    request: &LaunchEvalChatInput,
    lookup_params: Option<&serde_json::Value>,
    feedback: Option<&str>,
    suppress: bool,
    expect_cached: bool,
    expect_response_contains: &[String],
) -> LaunchEvalCaseResult {
    let memory_scope = request.memory_scope();
    let memory_scope_ref = memory_scope.as_deref();
    let mut errors = Vec::new();
    let mut notes = vec![format!(
        "scope={}",
        memory_scope.clone().unwrap_or_else(|| "global".to_string())
    )];

    if let Err(error) = seed_execution_memory(seed, memory_scope_ref) {
        return LaunchEvalCaseResult {
            id: id.to_string(),
            kind: "execution_memory_policy".to_string(),
            description: description.clone(),
            passed: false,
            errors: vec![error.to_string()],
            notes,
            command: Some(seed.command.clone()),
            response_preview: None,
            readiness: None,
            admission: None,
        };
    }

    let params_key = infer_execution_params_key(&seed.command, &seed.params);
    if let Some(sentiment) = feedback {
        notes.push(format!("feedback={}", sentiment));
        match &params_key {
            Ok(params_key) => {
                if let Err(error) = crate::db::record_execution_memory_feedback_scoped(
                    memory_scope_ref,
                    &seed.command,
                    params_key,
                    &seed.response_text,
                    sentiment,
                ) {
                    errors.push(format!(
                        "failed to apply execution memory feedback: {}",
                        error
                    ));
                }
            }
            Err(error) => errors.push(error.to_string()),
        }
    }
    if suppress {
        notes.push("suppressed=true".to_string());
        match &params_key {
            Ok(params_key) => {
                if let Err(error) = crate::db::suppress_execution_memory_scoped(
                    memory_scope_ref,
                    &seed.command,
                    params_key,
                    Some("launch eval policy suppression"),
                ) {
                    errors.push(format!("failed to suppress execution memory: {}", error));
                }
            }
            Err(error) => errors.push(error.to_string()),
        }
    }

    let params = lookup_params
        .cloned()
        .unwrap_or_else(|| seed.params.clone());
    let intent = json!({
        "command": seed.command,
        "params": params,
        "confidence": seed.confidence,
    });
    let cached = crate::api_server::load_cached_execution_response(
        memory_scope_ref,
        &request.message,
        &seed.command,
        &intent,
    );
    if cached.is_some() != expect_cached {
        errors.push(format!(
            "expected cached execution response={}, got {}",
            expect_cached,
            cached.is_some()
        ));
    }
    if let Some(response) = cached.as_ref() {
        for needle in expect_response_contains {
            if !response.contains(needle) {
                errors.push(format!("cached execution response missing '{}'", needle));
            }
        }
    }

    notes.push(format!(
        "lookup_params={}",
        truncate_preview(&intent["params"].to_string(), 120)
    ));

    LaunchEvalCaseResult {
        id: id.to_string(),
        kind: "execution_memory_policy".to_string(),
        description: description.clone(),
        passed: errors.is_empty(),
        errors,
        notes,
        command: Some(seed.command.clone()),
        response_preview: cached.map(|value| truncate_preview(&value, 220)),
        readiness: None,
        admission: None,
    }
}
