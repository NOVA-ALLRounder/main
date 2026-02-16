pub fn scenario_mode_enabled() -> bool {
    matches!(
        std::env::var("STEER_SCENARIO_MODE").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

pub fn history_contains_case_insensitive(history: &[String], needle: &str) -> bool {
    let needle_lower = needle.to_lowercase();
    history
        .iter()
        .any(|h| h.to_lowercase().contains(&needle_lower))
}

pub fn last_opened_app_from_history(history: &[String]) -> Option<String> {
    for entry in history.iter().rev() {
        if let Some(rest) = entry.strip_prefix("Opened app: ") {
            let app = rest.trim();
            if !app.is_empty() {
                return Some(app.to_string());
            }
        }
    }
    None
}

pub fn goal_contains_any(goal_lower: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| goal_lower.contains(needle))
}

pub fn is_textual_app(app: &str) -> bool {
    app.eq_ignore_ascii_case("Notes")
        || app.eq_ignore_ascii_case("TextEdit")
        || app.eq_ignore_ascii_case("Mail")
}

