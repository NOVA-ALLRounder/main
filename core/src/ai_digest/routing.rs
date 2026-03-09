use super::{
    resolve_program_webhook_url, AiDigestProgramRoute, AiDigestProgramRouteKind,
    DEFAULT_REQUEST_TEXT,
};
use chrono::Utc;
use regex::Regex;

pub fn default_request_text() -> &'static str {
    DEFAULT_REQUEST_TEXT
}

pub fn normalize_request_text(raw: Option<&str>) -> String {
    let trimmed = raw.unwrap_or_default().trim();
    if trimmed.is_empty() {
        return DEFAULT_REQUEST_TEXT.to_string();
    }
    trimmed.to_string()
}

pub fn build_scope_marker() -> String {
    format!(
        "RUN_SCOPE_TELEGRAM_AI_DIGEST_{}",
        Utc::now().format("%Y%m%d_%H%M%S")
    )
}

pub fn looks_like_news_digest_request(message: &str) -> bool {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = message.to_lowercase();
    let has_news = lower.contains("news")
        || lower.contains("headline")
        || lower.contains("trend")
        || lower.contains("digest")
        || message.contains("뉴스")
        || message.contains("기사")
        || message.contains("헤드라인")
        || message.contains("트렌드")
        || message.contains("브리핑");
    let has_notion = lower.contains("notion") || message.contains("노션");
    let has_digest = lower.contains("digest")
        || lower.contains("summary")
        || lower.contains("brief")
        || lower.contains("top")
        || lower.contains("latest")
        || message.contains("요약")
        || message.contains("정리")
        || message.contains("브리핑")
        || message.contains("선정")
        || message.contains("모아")
        || message.contains("저장");
    let has_youtube = lower.contains("youtube") || message.contains("유튜브");
    let has_count = Regex::new(r"(?i)(\d{1,2})\s*개|(?:top|latest)\s*(\d{1,2})")
        .ok()
        .is_some_and(|re| re.is_match(trimmed));

    has_news && has_notion && (has_digest || has_youtube || has_count)
}

pub fn looks_like_ai_digest_request(message: &str) -> bool {
    looks_like_news_digest_request(message)
}

pub fn extract_explicit_n8n_request(message: &str) -> Option<String> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return None;
    }

    let exact_markers = ["/n8n", "n8n", "/workflow", "workflow", "/digest", "digest"];
    if exact_markers
        .iter()
        .any(|marker| trimmed.eq_ignore_ascii_case(marker))
    {
        return Some(DEFAULT_REQUEST_TEXT.to_string());
    }

    let prefix_re =
        Regex::new(r"(?i)^(?:/n8n|n8n|/workflow|workflow|/digest|digest)(?:\s+|[:\-\|]\s*)(.+)$")
            .ok();
    if let Some(re) = prefix_re {
        if let Some(captures) = re.captures(trimmed) {
            if let Some(rest) = captures.get(1) {
                let cleaned = rest.as_str().trim();
                if !cleaned.is_empty() {
                    return Some(cleaned.to_string());
                }
            }
            return Some(DEFAULT_REQUEST_TEXT.to_string());
        }
    }

    let lower = trimmed.to_lowercase();
    let explicit_n8n_hint = lower.contains("n8n으로")
        || lower.contains("workflow로")
        || lower.contains("#n8n")
        || lower.contains("use n8n")
        || lower.contains("via n8n");
    if explicit_n8n_hint && looks_like_news_digest_request(trimmed) {
        return Some(trimmed.to_string());
    }

    None
}

pub fn strip_local_execution_prefix(message: &str) -> String {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let prefix_re =
        Regex::new(r"(?i)^(?:/local|local|/llm|llm|/surf|surf)(?:\s+|[:\-\|]\s*)(.+)$").ok();
    if let Some(re) = prefix_re {
        if let Some(captures) = re.captures(trimmed) {
            if let Some(rest) = captures.get(1) {
                let cleaned = rest.as_str().trim();
                if !cleaned.is_empty() {
                    return cleaned.to_string();
                }
            }
        }
    }

    trimmed.to_string()
}

fn normalize_channel_name(channel: Option<&str>) -> Option<String> {
    let normalized = channel
        .unwrap_or_default()
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn env_truthy(key: &str) -> Option<bool> {
    std::env::var(key).ok().map(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

pub fn ai_digest_auto_route_enabled(channel: Option<&str>) -> bool {
    let normalized = normalize_channel_name(channel);

    if let Some(configured) = std::env::var("ALLVIA_AI_DIGEST_AUTO_ROUTE_CHANNELS")
        .ok()
        .map(|value| {
            value
                .split(|ch: char| ch == ',' || ch == ';' || ch.is_whitespace())
                .map(|part| part.trim().to_lowercase())
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
        })
    {
        return normalized
            .as_ref()
            .is_some_and(|channel| configured.iter().any(|value| value == channel));
    }

    if normalized.as_deref() == Some("telegram") {
        return env_truthy("STEER_TELEGRAM_AUTO_ROUTE_AI_DIGEST").unwrap_or(true);
    }

    matches!(normalized.as_deref(), Some("web"))
}

pub fn infer_program_route(message: &str, channel: Option<&str>) -> Option<AiDigestProgramRoute> {
    let stripped = strip_local_execution_prefix(message);
    if stripped != message.trim() {
        return None;
    }

    if let Some(explicit) = extract_explicit_n8n_request(message) {
        return Some(AiDigestProgramRoute {
            request_text: explicit,
            kind: AiDigestProgramRouteKind::Explicit,
        });
    }

    if !ai_digest_auto_route_enabled(channel) {
        return None;
    }
    if !looks_like_ai_digest_request(message) {
        return None;
    }
    if resolve_program_webhook_url().is_err() {
        return None;
    }

    let request_text = if stripped.trim().is_empty() {
        DEFAULT_REQUEST_TEXT.to_string()
    } else {
        stripped
    };
    Some(AiDigestProgramRoute {
        request_text,
        kind: AiDigestProgramRouteKind::Auto,
    })
}
