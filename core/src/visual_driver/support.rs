pub(crate) fn normalize_timeout_ms(value: Option<u64>, default: u64, max: u64) -> u64 {
    value.map(|v| v.max(500).min(max)).unwrap_or(default)
}

pub(crate) fn should_fallback_to_native_type(err_text: &str) -> bool {
    let lower = err_text.to_lowercase();
    lower.contains("1002")
        || lower.contains("keystroke")
        || lower.contains("허용되지 않습니다")
        || lower.contains("not allowed to send keystrokes")
}
