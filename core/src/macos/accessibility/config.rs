fn env_flag_default(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn env_usize_bounded(key: &str, default: usize, min: usize, max: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default)
}

fn env_u64_bounded(key: &str, default: u64, min: u64, max: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default)
}

pub(super) fn snapshot_focus_retry_count() -> usize {
    env_usize_bounded("STEER_AX_SNAPSHOT_FOCUS_RETRIES", 2, 0, 8)
}

pub(super) fn snapshot_focus_retry_ms() -> u64 {
    env_u64_bounded("STEER_AX_SNAPSHOT_RETRY_MS", 120, 20, 2000)
}

pub(super) fn snapshot_window_retry_count() -> usize {
    env_usize_bounded("STEER_AX_SNAPSHOT_WINDOW_RETRIES", 3, 0, 12)
}

pub(super) fn snapshot_window_retry_ms() -> u64 {
    env_u64_bounded("STEER_AX_SNAPSHOT_WINDOW_RETRY_MS", 90, 20, 2000)
}

pub(super) fn snapshot_fallback_app_name() -> String {
    std::env::var("STEER_AX_SNAPSHOT_FALLBACK_APP")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "Finder".to_string())
}

pub(super) fn snapshot_strict_mode() -> bool {
    if let Ok(raw) = std::env::var("STEER_AX_SNAPSHOT_STRICT") {
        let normalized = raw.trim().to_ascii_lowercase();
        if matches!(normalized.as_str(), "0" | "false" | "no" | "off") {
            return false;
        }
        if matches!(normalized.as_str(), "1" | "true" | "yes" | "on") {
            return true;
        }
    }
    crate::env_flag("STEER_SCENARIO_MODE")
        || crate::env_flag("STEER_TEST_MODE")
        || env_flag_default("STEER_PREFLIGHT_FOCUS_HANDOFF", false)
}
