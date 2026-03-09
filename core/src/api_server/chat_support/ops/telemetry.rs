use serde_json::json;

use crate::api_server::{ChatRequest, ChatResponse};
use crate::db;

use super::ChatOpsFlags;

fn safe_text_preview(value: &str, max_chars: usize) -> String {
    let normalized = value.trim().replace('\n', " ");
    let mut chars = normalized.chars();
    let preview: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}...", preview)
    } else {
        preview
    }
}

fn chat_ops_message_preview(message: &str) -> String {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return "<empty>".to_string();
    }
    safe_text_preview(trimmed, 160)
}

pub(crate) fn chat_ops_outcome_from_response(response: &str) -> &'static str {
    if request_memory_response_is_successful(response) {
        "success"
    } else {
        "error"
    }
}

pub(super) fn record_chat_response_effects(
    req: &ChatRequest,
    memory_scope: Option<&str>,
    message: &str,
    response: &ChatResponse,
    route_kind: &str,
    outcome: &str,
    confidence: Option<f64>,
    flags: ChatOpsFlags,
    note: Option<&str>,
) {
    record_chat_ops_event(
        req,
        memory_scope,
        message,
        route_kind,
        response.command.as_deref(),
        outcome,
        confidence,
        flags,
        note,
    );
    record_chat_nl_run(
        req,
        memory_scope,
        message,
        response,
        route_kind,
        outcome,
        confidence,
        flags,
        note,
    );
}

fn record_chat_ops_event(
    req: &ChatRequest,
    memory_scope: Option<&str>,
    message: &str,
    route_kind: &str,
    command: Option<&str>,
    outcome: &str,
    confidence: Option<f64>,
    flags: ChatOpsFlags,
    note: Option<&str>,
) {
    let preview = chat_ops_message_preview(message);
    if let Err(err) = db::record_launch_ops_event(
        req.channel.as_deref(),
        memory_scope,
        &preview,
        route_kind,
        command,
        outcome,
        confidence,
        flags.freshness_bypassed,
        flags.intent_memory_hit,
        flags.request_memory_hit,
        flags.execution_memory_hit,
        flags.deterministic_used,
        flags.llm_used,
        flags.ai_digest_used,
        flags.local_only,
        note,
    ) {
        eprintln!("Failed to record launch ops event: {}", err);
    }
}

fn chat_nl_run_should_record(route_kind: &str, command: Option<&str>, message: &str) -> bool {
    if crate::request_memory::normalize_request_text(message).is_empty() {
        return false;
    }

    if matches!(
        route_kind,
        "empty_message" | "gate_blocked" | "system_command" | "local_command" | "vision_demo"
    ) {
        return false;
    }

    !matches!(
        command.unwrap_or(""),
        "help_local"
            | "greeting_local"
            | "system_status"
            | "telegram_listener_start"
            | "telegram_listener_status"
            | "n8n_restart"
    )
}

fn chat_nl_run_status(route_kind: &str, response: &ChatResponse, outcome: &str) -> &'static str {
    if route_kind == "gate_blocked" || outcome == "blocked" {
        return "blocked";
    }
    if response.command.as_deref() == Some("build_workflow")
        || response.response.contains("승인 후 생성")
        || response.response.contains("approval")
    {
        return "approval_required";
    }
    if response.response.contains("수동으로")
        || response.response.contains("직접 확인")
        || response.response.contains("manual")
    {
        return "manual_required";
    }
    if outcome == "success" {
        "completed"
    } else {
        "error"
    }
}

fn chat_nl_run_summary(response: &str) -> Option<String> {
    let trimmed = response.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(safe_text_preview(trimmed, 160))
}

fn record_chat_nl_run(
    req: &ChatRequest,
    memory_scope: Option<&str>,
    message: &str,
    response: &ChatResponse,
    route_kind: &str,
    outcome: &str,
    confidence: Option<f64>,
    flags: ChatOpsFlags,
    note: Option<&str>,
) {
    if !chat_nl_run_should_record(route_kind, response.command.as_deref(), message) {
        return;
    }

    let status = chat_nl_run_status(route_kind, response, outcome);
    let summary = chat_nl_run_summary(&response.response);
    let details = json!({
        "source": "api.chat",
        "route_kind": route_kind,
        "command": response.command,
        "channel": req.channel,
        "chat_type": req.chat_type,
        "sender": req.sender,
        "memory_scope": memory_scope,
        "outcome": outcome,
        "confidence": confidence,
        "note": note,
        "flags": {
            "freshness_bypassed": flags.freshness_bypassed,
            "intent_memory_hit": flags.intent_memory_hit,
            "request_memory_hit": flags.request_memory_hit,
            "execution_memory_hit": flags.execution_memory_hit,
            "deterministic_used": flags.deterministic_used,
            "llm_used": flags.llm_used,
            "ai_digest_used": flags.ai_digest_used,
            "local_only": flags.local_only
        }
    });
    let details_json = serde_json::to_string(&details).ok();
    let intent = response.command.as_deref().unwrap_or(route_kind);
    if let Err(err) = db::insert_nl_run(
        intent,
        message,
        status,
        summary.as_deref(),
        details_json.as_deref(),
    ) {
        eprintln!("Failed to record chat nl_run: {}", err);
    }
}

pub(super) fn request_memory_response_is_successful(response: &str) -> bool {
    super::super::memory::request_memory_response_is_successful(response)
}
