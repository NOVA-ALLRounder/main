use crate::launch_eval::*;

pub(super) fn request_memory_candidate(
    record: &crate::db::RequestMemoryRecord,
) -> Option<LaunchEvalCandidate> {
    let command = record.intent_command.as_deref()?.trim();
    if !matches!(command, "calendar_today" | "calendar_week" | "gmail_list") {
        return None;
    }
    if is_synthetic_source(&record.source) {
        return None;
    }

    let params = request_memory_params(record.intent_json.as_deref());
    let scope = candidate_scope(&record.memory_scope, command);
    let synthetic_marker = format!(
        "LAUNCH_EVAL_REQUEST_CACHE_{}",
        slug_id(&record.original_request)
    );
    let scenario = LaunchEvalScenario::RequestMemoryReuse {
        id: candidate_id("request-memory", command, &record.original_request),
        description: Some(format!(
            "Generated from request memory (use_count={}, confidence={:.2}).",
            record.use_count, record.confidence
        )),
        seed: RequestMemorySeed {
            request_text: record.original_request.clone(),
            command: command.to_string(),
            params,
            response_text: synthetic_marker.clone(),
            confidence: record.confidence.max(0.8),
            source: "launch.eval.candidate.request_memory".to_string(),
            response_mode: Some(record.response_mode.clone()),
        },
        request: LaunchEvalChatInput {
            message: record.original_request.clone(),
            channel: scope.channel.clone(),
            chat_type: scope.chat_type.clone(),
            sender: scope.sender.clone(),
            mentioned: Some(false),
        },
        expect: LaunchEvalChatExpectation {
            command: Some(command.to_string()),
            response_contains: vec![synthetic_marker.clone()],
            response_contains_any: vec![],
            response_not_contains: vec![],
        },
    };

    Some(LaunchEvalCandidate {
        id: scenario_id(&scenario),
        provenance: "real".to_string(),
        source_kind: "request_memory".to_string(),
        scenario_kind: "request_memory_reuse".to_string(),
        title: format!(
            "Request memory: {}",
            summarize_message(&record.original_request)
        ),
        score: request_memory_score(record),
        command: Some(command.to_string()),
        request_message: record.original_request.clone(),
        rationale: vec![
            format!("use_count={}", record.use_count),
            format!("confidence={:.2}", record.confidence),
            format!(
                "feedback=+{} -{}",
                record.positive_feedback_count, record.negative_feedback_count
            ),
        ],
        yaml: render_scenario_yaml(&scenario),
    })
}

pub(super) fn execution_memory_candidate(
    record: &crate::db::ExecutionMemoryRecord,
) -> Option<LaunchEvalCandidate> {
    if !record.success || record.suppressed {
        return None;
    }
    if !matches!(
        record.intent_command.as_str(),
        "calendar_today" | "calendar_week" | "gmail_list"
    ) {
        return None;
    }
    if is_synthetic_source(&record.source) {
        return None;
    }

    let request_message = execution_request_message(record)?;
    let params = parse_json_object(record.params_json.as_deref());
    let scope = candidate_scope(&record.memory_scope, &record.intent_command);
    let synthetic_marker = format!(
        "LAUNCH_EVAL_EXECUTION_CACHE_{}",
        slug_id(&format!("{}-{}", record.intent_command, request_message))
    );
    let scenario = LaunchEvalScenario::ExecutionMemoryReuse {
        id: candidate_id("execution-memory", &record.intent_command, &request_message),
        description: Some(format!(
            "Generated from execution memory (use_count={}, ttl={}s).",
            record.use_count, record.freshness_ttl_seconds
        )),
        seed: ExecutionMemorySeed {
            original_request: request_message.clone(),
            command: record.intent_command.clone(),
            params,
            response_text: synthetic_marker.clone(),
            confidence: execution_memory_confidence(record),
            source: "launch.eval.candidate.execution_memory".to_string(),
            ttl_seconds: Some(record.freshness_ttl_seconds.max(1)),
            tool_path: Some(record.tool_path.clone()),
        },
        request: LaunchEvalChatInput {
            message: request_message.clone(),
            channel: scope.channel.clone(),
            chat_type: scope.chat_type.clone(),
            sender: scope.sender.clone(),
            mentioned: Some(false),
        },
        expect: LaunchEvalChatExpectation {
            command: Some(record.intent_command.clone()),
            response_contains: vec![synthetic_marker.clone()],
            response_contains_any: vec![],
            response_not_contains: vec![],
        },
    };

    Some(LaunchEvalCandidate {
        id: scenario_id(&scenario),
        provenance: "real".to_string(),
        source_kind: "execution_memory".to_string(),
        scenario_kind: "execution_memory_reuse".to_string(),
        title: format!("Execution memory: {}", summarize_message(&request_message)),
        score: execution_memory_score(record),
        command: Some(record.intent_command.clone()),
        request_message,
        rationale: vec![
            format!("use_count={}", record.use_count),
            format!("ttl={}s", record.freshness_ttl_seconds),
            format!(
                "feedback=+{} -{}",
                record.positive_feedback_count, record.negative_feedback_count
            ),
        ],
        yaml: render_scenario_yaml(&scenario),
    })
}

fn is_synthetic_source(source: &str) -> bool {
    let normalized = source.trim().to_ascii_lowercase();
    normalized.starts_with("unit.test")
        || normalized.starts_with("launch.eval")
        || normalized.starts_with("launch.dogfood.synthetic")
}
