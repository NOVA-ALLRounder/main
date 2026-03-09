pub(super) fn slugify_for_path(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars() {
        let normalized = ch.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            out.push(normalized);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
        if out.len() >= 48 {
            break;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "steer-workflow".to_string()
    } else {
        trimmed.to_string()
    }
}

pub(super) fn compact_prompt_seed(prompt: Option<&str>) -> String {
    prompt
        .unwrap_or_default()
        .trim()
        .replace('\n', " ")
        .chars()
        .take(240)
        .collect()
}
