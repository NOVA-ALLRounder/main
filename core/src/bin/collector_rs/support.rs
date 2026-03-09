use super::AppState;
use axum::http::HeaderMap;
use chrono::{DateTime, Timelike, Utc};
use local_os_agent::schema::EventEnvelope;
use serde_json::Value;
use std::path::PathBuf;

pub(super) fn request_is_authorized(state: &AppState, headers: &HeaderMap) -> bool {
    let Some(expected) = state.ingest_token.as_ref() else {
        return true;
    };
    if let Some(value) = headers
        .get("x-collector-token")
        .and_then(|v| v.to_str().ok())
    {
        if value == expected {
            return true;
        }
    }
    if let Some(value) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(token) = value.strip_prefix("Bearer ") {
            if token.trim() == expected {
                return true;
            }
        }
    }
    false
}

pub(super) fn parse_events(payload: Value) -> Result<Vec<EventEnvelope>, String> {
    if let Some(arr) = payload.as_array() {
        let mut events = Vec::with_capacity(arr.len());
        for v in arr {
            let event: EventEnvelope = serde_json::from_value(v.clone())
                .map_err(|e| format!("invalid event in array: {e}"))?;
            events.push(event);
        }
        Ok(events)
    } else {
        let single: EventEnvelope =
            serde_json::from_value(payload).map_err(|e| format!("invalid event object: {e}"))?;
        Ok(vec![single])
    }
}

pub(super) fn resolve_db_path() -> PathBuf {
    let config_override = std::env::var("STEER_COLLECTOR_CONFIG")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    local_os_agent::collector_pipeline::resolve_db_path(config_override.as_deref())
}

pub(super) fn floor_to_five_minute_bucket(ts: DateTime<Utc>) -> DateTime<Utc> {
    let minute_bucket = ts.minute() - (ts.minute() % 5);
    ts.with_second(0)
        .and_then(|v| v.with_nanosecond(0))
        .and_then(|v| v.with_minute(minute_bucket))
        .unwrap_or(ts)
}

pub(super) fn format_iso_z(ts: DateTime<Utc>) -> String {
    ts.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub(super) fn normalize_app_for_pattern(app: &str) -> String {
    let source = if let Some((_, suffix)) = app.rsplit_once(" - ") {
        suffix
    } else {
        app
    };

    source
        .split('.')
        .next()
        .unwrap_or(source)
        .trim()
        .to_string()
}

pub(super) fn normalize_app_for_display(app: &str) -> String {
    if let Some((_, suffix)) = app.rsplit_once(" - ") {
        suffix.to_string()
    } else {
        app.to_string()
    }
}

pub(super) fn infer_action_type(control_type: &str) -> &'static str {
    match control_type {
        "Button" | "MenuItem" | "TabItem" | "Hyperlink" => "click",
        "Edit" => "type",
        "Text" => "read",
        "ListItem" | "TreeItem" | "RadioButton" => "select",
        "CheckBox" => "toggle",
        _ => "interact",
    }
}

pub(super) fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        value.chars().take(max_chars).collect()
    }
}
