use std::env;

pub(super) fn request_memory_min_confidence() -> f64 {
    env::var("ALLVIA_REQUEST_MEMORY_MIN_CONFIDENCE")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(|v| v.clamp(0.0, 1.0))
        .unwrap_or(0.8)
}

pub(super) fn request_memory_signature_cache_supported(command: &str) -> bool {
    matches!(command, "calendar_today" | "calendar_week" | "gmail_list")
}

pub(crate) fn feedback_allows_response_reuse(
    positive_feedback_count: i64,
    negative_feedback_count: i64,
) -> bool {
    negative_feedback_count == 0 || positive_feedback_count > negative_feedback_count
}

pub(super) fn request_memory_response_cache_ttl_secs(
    command: &str,
    response_mode: &str,
) -> Option<i64> {
    let env_key = match command {
        "gmail_list" => Some("ALLVIA_RESPONSE_CACHE_TTL_GMAIL_LIST"),
        "calendar_today" => Some("ALLVIA_RESPONSE_CACHE_TTL_CALENDAR_TODAY"),
        "calendar_week" => Some("ALLVIA_RESPONSE_CACHE_TTL_CALENDAR_WEEK"),
        "system_status" => Some("ALLVIA_RESPONSE_CACHE_TTL_SYSTEM_STATUS"),
        _ => None,
    };

    if response_mode == "reusable_response" {
        return env_key
            .and_then(|key| env::var(key).ok())
            .and_then(|value| value.trim().parse::<i64>().ok())
            .or(Some(24 * 60 * 60));
    }

    if !matches!(
        response_mode,
        "ttl_response_signature" | "ttl_response_exact"
    ) {
        return None;
    }

    env_key
        .and_then(|key| env::var(key).ok())
        .and_then(|value| value.trim().parse::<i64>().ok())
        .or_else(|| match command {
            "gmail_list" => Some(20),
            "calendar_today" => Some(30),
            "calendar_week" => Some(120),
            "system_status" => Some(10),
            _ => None,
        })
        .filter(|ttl| *ttl > 0)
}

pub(super) fn request_memory_response_mode_supports_signature(response_mode: &str) -> bool {
    matches!(response_mode, "ttl_response_signature")
}

pub(super) fn command_supports_live_refresh(command: &str) -> bool {
    matches!(
        command,
        "gmail_list" | "calendar_today" | "calendar_week" | "system_status"
    )
}
