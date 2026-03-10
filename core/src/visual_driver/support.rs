pub(crate) fn normalize_timeout_ms(value: Option<u64>, default: u64, max: u64) -> u64 {
    value.map(|v| v.max(500).min(max)).unwrap_or(default)
}
