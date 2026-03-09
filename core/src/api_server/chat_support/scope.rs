use crate::api_server::chat::{ChatFeedbackRequest, ChatRequest};

fn normalize_chat_scope_part(value: Option<&str>) -> Option<String> {
    let normalized = value
        .unwrap_or_default()
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn build_chat_memory_scope(
    channel: Option<&str>,
    chat_type: Option<&str>,
    sender: Option<&str>,
) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(channel) = normalize_chat_scope_part(channel) {
        parts.push(format!("channel_{}", channel));
    }
    if let Some(chat_type) = normalize_chat_scope_part(chat_type) {
        parts.push(format!("type_{}", chat_type));
    }
    if let Some(sender) = normalize_chat_scope_part(sender) {
        parts.push(format!("sender_{}", sender));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("__"))
    }
}

pub(crate) fn chat_request_memory_scope(req: &ChatRequest) -> Option<String> {
    build_chat_memory_scope(
        req.channel.as_deref(),
        req.chat_type.as_deref(),
        req.sender.as_deref(),
    )
}

pub(crate) fn chat_feedback_memory_scope(req: &ChatFeedbackRequest) -> Option<String> {
    build_chat_memory_scope(
        req.channel.as_deref(),
        req.chat_type.as_deref(),
        req.sender.as_deref(),
    )
}
