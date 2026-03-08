use unicode_normalization::UnicodeNormalization;

const TOKEN_STOPWORDS: &[&str] = &[
    "a",
    "an",
    "the",
    "me",
    "my",
    "please",
    "pls",
    "plz",
    "좀",
    "조금",
    "한번",
    "부탁",
    "부탁해",
    "부탁해요",
    "부탁드립니다",
];

fn preprocess_phrases(input: &str) -> String {
    input
        .nfkc()
        .collect::<String>()
        .to_lowercase()
        .replace("this week", "thisweek")
        .replace("next week", "nextweek")
        .replace("이번 주", "이번주")
        .replace("다음 주", "다음주")
}

pub fn normalize_request_text(input: &str) -> String {
    preprocess_phrases(input)
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch.is_whitespace() {
                ch
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn canonical_token(token: &str) -> Option<&'static str> {
    match token {
        "오늘" | "금일" | "today" => Some("today"),
        "내일" | "tomorrow" => Some("tomorrow"),
        "이번주" | "금주" | "thisweek" => Some("this_week"),
        "다음주" | "nextweek" => Some("next_week"),
        "일정" | "스케줄" | "캘린더" | "calendar" | "schedule" | "agenda" => {
            Some("calendar")
        }
        "메일" | "이메일" | "mail" | "email" | "emails" | "gmail" | "inbox" => Some("email"),
        "최근" | "latest" | "recent" => Some("recent"),
        "보여줘" | "보여" | "알려줘" | "알려" | "확인해줘" | "확인" | "조회" | "list" | "show"
        | "tell" | "fetch" | "check" | "get" | "view" => Some("read"),
        "요약" | "요약해줘" | "정리" | "정리해줘" | "summary" | "summarize" | "digest" => {
            Some("summarize")
        }
        "상태" | "status" => Some("status"),
        "시스템" | "system" | "코어" | "core" => Some("system"),
        "도움말" | "help" | "명령어" | "commands" => Some("help"),
        _ => None,
    }
}

fn should_keep_numeric(token: &str) -> bool {
    token.chars().all(|ch| ch.is_ascii_digit())
}

fn supported_numeric_suffix(suffix: &str) -> bool {
    matches!(
        suffix,
        "개" | "건" | "명" | "번" | "통" | "개만" | "건만" | "명만" | "번만"
    )
}

fn extract_numeric_token(token: &str) -> Option<String> {
    if should_keep_numeric(token) {
        return Some(token.to_string());
    }

    let mut digits = String::new();
    let mut suffix = String::new();
    let mut seen_non_digit = false;

    for ch in token.chars() {
        if ch.is_ascii_digit() && !seen_non_digit {
            digits.push(ch);
        } else {
            seen_non_digit = true;
            suffix.push(ch);
        }
    }

    if digits.is_empty()
        || suffix.is_empty()
        || suffix.chars().any(|ch| ch.is_ascii_digit())
        || !supported_numeric_suffix(&suffix)
    {
        None
    } else {
        Some(digits)
    }
}

pub fn build_request_signature(input: &str) -> String {
    let normalized = normalize_request_text(input);
    let mut out = Vec::new();

    for token in normalized.split_whitespace() {
        if TOKEN_STOPWORDS.contains(&token) {
            continue;
        }

        let canonical = canonical_token(token)
            .map(str::to_string)
            .or_else(|| extract_numeric_token(token));

        let Some(token) = canonical else {
            continue;
        };

        if !out.iter().any(|existing| existing == &token) {
            out.push(token);
        }
    }

    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_request_text_strips_punctuation_and_case() {
        assert_eq!(
            normalize_request_text("오늘 일정 보여줘?!"),
            "오늘 일정 보여줘"
        );
        assert_eq!(normalize_request_text("SHOW   ME   EMAIL"), "show me email");
    }

    #[test]
    fn build_request_signature_collapses_safe_aliases() {
        assert_eq!(
            build_request_signature("오늘 일정 보여줘"),
            build_request_signature("오늘 캘린더 알려줘")
        );
        assert_eq!(
            build_request_signature("최근 이메일 5개 보여줘"),
            "recent email 5 read"
        );
    }

    #[test]
    fn build_request_signature_keeps_time_scope_distinct() {
        assert_ne!(
            build_request_signature("오늘 일정 보여줘"),
            build_request_signature("이번 주 일정 보여줘")
        );
    }

    #[test]
    fn build_request_signature_ignores_mixed_identifier_digits() {
        assert_eq!(
            build_request_signature("allvia-positive-123e4567 오늘 일정 보여줘"),
            "today calendar read"
        );
        assert_eq!(
            build_request_signature("build abc123xyz 최근 이메일 5개 보여줘"),
            "recent email 5 read"
        );
    }
}
