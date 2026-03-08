use crate::request_memory::{build_request_signature, normalize_request_text};
use serde_json::{json, Value};
use std::collections::HashSet;

const WORKFLOW_KEYWORDS: &[&str] = &[
    "workflow",
    "workflows",
    "automation",
    "automate",
    "n8n",
    "워크플로우",
    "자동화",
];

const CREATE_SIGNALS: &[&str] = &[
    "create", "build", "make", "generate", "setup", "set up", "만들", "생성", "구성", "설계",
    "작성", "짜", "등록",
];

const BROWSE_SIGNALS: &[&str] = &[
    "show", "list", "browse", "view", "what", "추천", "목록", "보여", "조회",
];

const MUTATING_CALENDAR_SIGNALS: &[&str] = &[
    "add", "create", "schedule", "invite", "register", "추가", "등록", "생성", "잡아", "만들",
];

const EMAIL_SEND_SIGNALS: &[&str] = &[
    "send", "draft", "compose", "reply", "forward", "subject", "body", "to ", "mailto", "보내",
    "작성", "답장", "전달", "제목", "본문",
];

const EMAIL_DETAIL_SIGNALS: &[&str] = &["read ", "읽어", "본문", "내용"];

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn signature_tokens(text: &str) -> HashSet<String> {
    build_request_signature(text)
        .split_whitespace()
        .map(|token| token.to_string())
        .collect()
}

fn extract_count(tokens: &HashSet<String>, default_value: u32) -> u32 {
    tokens
        .iter()
        .find_map(|token| token.parse::<u32>().ok())
        .map(|count| count.clamp(1, 20))
        .unwrap_or(default_value)
}

fn parse_analyze_patterns(normalized: &str) -> Option<Value> {
    let has_pattern = normalized.contains("패턴") || normalized.contains("pattern");
    let has_analysis = contains_any(
        normalized,
        &["분석", "analyze", "analysis", "detect", "찾아"],
    );
    if has_pattern && has_analysis {
        Some(json!({
            "command": "analyze_patterns",
            "params": {},
            "confidence": 0.97,
            "source": "deterministic"
        }))
    } else {
        None
    }
}

fn parse_calendar(normalized: &str, tokens: &HashSet<String>) -> Option<Value> {
    if tokens.contains("summarize") || contains_any(normalized, MUTATING_CALENDAR_SIGNALS) {
        return None;
    }
    if !tokens.contains("calendar") {
        return None;
    }

    if tokens.contains("today") {
        let confidence = if tokens.contains("read") { 0.97 } else { 0.93 };
        return Some(json!({
            "command": "calendar_today",
            "params": {},
            "confidence": confidence,
            "source": "deterministic"
        }));
    }

    if tokens.contains("this_week") {
        let confidence = if tokens.contains("read") { 0.97 } else { 0.93 };
        return Some(json!({
            "command": "calendar_week",
            "params": {},
            "confidence": confidence,
            "source": "deterministic"
        }));
    }

    None
}

fn parse_gmail_list(normalized: &str, tokens: &HashSet<String>) -> Option<Value> {
    if !tokens.contains("email") {
        return None;
    }
    if tokens.contains("summarize") {
        return None;
    }
    if normalized.contains('@') {
        return None;
    }
    if contains_any(normalized, EMAIL_SEND_SIGNALS)
        || contains_any(normalized, EMAIL_DETAIL_SIGNALS)
    {
        return None;
    }
    if !tokens.contains("read")
        && !tokens.contains("recent")
        && !tokens.iter().any(|token| token.parse::<u32>().is_ok())
    {
        return None;
    }

    let count = extract_count(tokens, 5);
    let confidence = if tokens.contains("recent") || count != 5 {
        0.96
    } else {
        0.92
    };
    Some(json!({
        "command": "gmail_list",
        "params": { "count": count },
        "confidence": confidence,
        "source": "deterministic"
    }))
}

fn parse_build_workflow(normalized: &str, original: &str) -> Option<Value> {
    if !contains_any(normalized, WORKFLOW_KEYWORDS) {
        return None;
    }
    if !contains_any(normalized, CREATE_SIGNALS) {
        return None;
    }
    if contains_any(normalized, BROWSE_SIGNALS) {
        return None;
    }

    let prompt = original.trim();
    if prompt.is_empty() {
        return None;
    }

    Some(json!({
        "command": "build_workflow",
        "params": { "prompt": prompt },
        "confidence": 0.90,
        "source": "deterministic"
    }))
}

pub fn classify_chat_intent(text: &str) -> Option<Value> {
    let normalized = normalize_request_text(text);
    if normalized.is_empty() {
        return None;
    }
    if let Some(intent) = parse_analyze_patterns(&normalized) {
        return Some(intent);
    }

    let tokens = signature_tokens(text);
    if let Some(intent) = parse_calendar(&normalized, &tokens) {
        return Some(intent);
    }
    if let Some(intent) = parse_gmail_list(&normalized, &tokens) {
        return Some(intent);
    }
    if let Some(intent) = parse_build_workflow(&normalized, text) {
        return Some(intent);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_pattern_analysis_request() {
        let intent = classify_chat_intent("패턴 분석해줘").expect("intent");
        assert_eq!(intent["command"].as_str(), Some("analyze_patterns"));
    }

    #[test]
    fn classifies_today_calendar_request() {
        let intent = classify_chat_intent("오늘 캘린더 보여줘").expect("intent");
        assert_eq!(intent["command"].as_str(), Some("calendar_today"));
    }

    #[test]
    fn classifies_week_calendar_request() {
        let intent = classify_chat_intent("이번 주 일정 확인해줘").expect("intent");
        assert_eq!(intent["command"].as_str(), Some("calendar_week"));
    }

    #[test]
    fn classifies_recent_email_count_request() {
        let intent = classify_chat_intent("최근 이메일 10개 보여줘").expect("intent");
        assert_eq!(intent["command"].as_str(), Some("gmail_list"));
        assert_eq!(intent["params"]["count"].as_u64(), Some(10));
    }

    #[test]
    fn skips_email_summary_request() {
        assert!(classify_chat_intent("최근 이메일 5개 요약해줘").is_none());
    }

    #[test]
    fn skips_email_send_request() {
        assert!(classify_chat_intent("foo@example.com 으로 이메일 보내줘").is_none());
    }

    #[test]
    fn classifies_explicit_workflow_creation_request() {
        let intent =
            classify_chat_intent("노션에 회의록 정리하는 워크플로우 만들어줘").expect("intent");
        assert_eq!(intent["command"].as_str(), Some("build_workflow"));
    }

    #[test]
    fn skips_workflow_browse_request() {
        assert!(classify_chat_intent("워크플로우 추천 목록 보여줘").is_none());
    }
}
