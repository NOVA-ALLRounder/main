pub(super) fn normalize_contract_token(input: &str) -> String {
    input
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || ['-', '_', '@', '.'].contains(c))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn summary_contains_token(summary: &str, token: &str) -> bool {
    let token_norm = normalize_contract_token(token);
    if token_norm.is_empty() {
        return true;
    }
    let summary_norm = normalize_contract_token(summary);
    summary_norm.contains(&token_norm)
}

pub(super) fn slot_value(plan: &crate::nl_automation::Plan, key: &str) -> Option<String> {
    plan.slots
        .get(key)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub(super) fn is_meaningful_summary(plan: &crate::nl_automation::Plan, summary: &str) -> bool {
    let normalized = summary.trim().to_lowercase();
    if normalized.is_empty() {
        return false;
    }
    if matches!(normalized.as_str(), "need more details" | "n/a" | "unknown") {
        return false;
    }
    if normalized.contains("no summary extracted") {
        return false;
    }
    if matches!(
        plan.intent,
        crate::nl_automation::IntentType::FlightSearch
            | crate::nl_automation::IntentType::ShoppingCompare
            | crate::nl_automation::IntentType::FormFill
    ) && normalized.contains("unknown")
    {
        return false;
    }
    true
}

pub(crate) fn extract_summary(logs: &[String]) -> Option<String> {
    logs.iter()
        .find_map(|line| line.strip_prefix("Summary: ").map(|s| s.to_string()))
}
