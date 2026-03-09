use super::{
    build_scope_marker, normalize_request_text, AiDigestTriggerResult, DEFAULT_RUNBOOK_ROOT,
    DEFAULT_WEBHOOK_TIMEOUT_SECS,
};
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

fn env_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn runbook_root() -> String {
    env_non_empty("STEER_MASTER_RUNBOOK_ROOT").unwrap_or_else(|| DEFAULT_RUNBOOK_ROOT.to_string())
}

fn resolve_workflow_id_from_runbook() -> Option<String> {
    let latest = PathBuf::from(runbook_root()).join("latest_run_dir.txt");
    let run_dir = std::fs::read_to_string(latest).ok()?;
    let run_dir = run_dir.trim();
    if run_dir.is_empty() {
        return None;
    }

    let status_json_path = PathBuf::from(run_dir).join("status.json");
    let raw = std::fs::read_to_string(status_json_path).ok()?;
    let status: Value = serde_json::from_str(&raw).ok()?;
    status
        .get("n8n_workflow_id")
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub fn resolve_program_webhook_url() -> Result<String> {
    if let Some(url) = env_non_empty("STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL") {
        return Ok(url);
    }

    let workflow_id =
        env_non_empty("STEER_AI_DIGEST_WORKFLOW_ID").or_else(resolve_workflow_id_from_runbook);

    if let Some(workflow_id) = workflow_id {
        return Ok(format!(
            "http://localhost:5678/webhook/{}/programtrigger/ai-digest-program",
            workflow_id
        ));
    }

    Err(anyhow!(
        "AI digest webhook not configured. Set STEER_AI_DIGEST_PROGRAM_WEBHOOK_URL or \
STEER_AI_DIGEST_WORKFLOW_ID, or keep /tmp/steer_master_runbook status.json with n8n_workflow_id."
    ))
}

fn webhook_timeout_secs() -> u64 {
    std::env::var("STEER_AI_DIGEST_TIMEOUT_SEC")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|v| v.clamp(30, 900))
        .unwrap_or(DEFAULT_WEBHOOK_TIMEOUT_SECS)
}

fn maybe_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn dedupe_push(vec: &mut Vec<String>, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return;
    }
    if vec.iter().any(|v| v.eq_ignore_ascii_case(trimmed)) {
        return;
    }
    vec.push(trimmed.to_string());
}

fn collect_string_fields_by_keys(payload: &Value, keys: &[&str], max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![payload];
    while let Some(node) = stack.pop() {
        match node {
            Value::Object(map) => {
                for key in keys {
                    if let Some(value) = maybe_string(map.get(*key)) {
                        dedupe_push(&mut out, &value);
                        if out.len() >= max {
                            return out;
                        }
                    }
                }
                for value in map.values() {
                    stack.push(value);
                }
            }
            Value::Array(items) => {
                for item in items.iter().rev() {
                    stack.push(item);
                }
            }
            _ => {}
        }
    }
    out
}

fn extract_headline_lines(text: &str, out: &mut Vec<String>) {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized = trimmed
            .trim_start_matches("- ")
            .trim_start_matches("* ")
            .trim();
        let candidate = if let Some((num, rest)) = normalized.split_once('.') {
            if num.chars().all(|ch| ch.is_ascii_digit()) && !rest.trim().is_empty() {
                rest.trim()
            } else {
                normalized
            }
        } else {
            normalized
        };
        if candidate.is_empty() {
            continue;
        }
        if candidate.starts_with("http://") || candidate.starts_with("https://") {
            continue;
        }
        dedupe_push(out, candidate);
        if out.len() >= 5 {
            return;
        }
    }
}

fn extract_headlines(payload: &Value) -> Vec<String> {
    let mut out = Vec::new();

    for value in collect_string_fields_by_keys(payload, &["top_headlines_text"], 4) {
        extract_headline_lines(&value, &mut out);
        if out.len() >= 5 {
            return out;
        }
    }

    for value in collect_string_fields_by_keys(payload, &["telegram_summary"], 4) {
        extract_headline_lines(&value, &mut out);
        if out.len() >= 5 {
            return out;
        }
    }

    let mut stack = vec![payload];
    while let Some(node) = stack.pop() {
        match node {
            Value::Object(map) => {
                if let Some(Value::Array(items)) = map.get("top_headlines") {
                    for item in items {
                        if let Some(title) = item.as_str() {
                            dedupe_push(&mut out, title);
                            if out.len() >= 5 {
                                return out;
                            }
                        }
                    }
                }
                if let Some(title) = maybe_string(map.get("title")) {
                    let has_news_link = map.get("source_url").and_then(|v| v.as_str()).is_some()
                        || map.get("link").and_then(|v| v.as_str()).is_some();
                    if has_news_link {
                        dedupe_push(&mut out, &title);
                        if out.len() >= 5 {
                            return out;
                        }
                    }
                }
                for value in map.values() {
                    stack.push(value);
                }
            }
            Value::Array(items) => {
                for item in items.iter().rev() {
                    stack.push(item);
                }
            }
            _ => {}
        }
    }

    out
}

pub(crate) fn extract_notion_url(payload: &Value) -> Option<String> {
    let mut candidates = Vec::new();
    for key in ["notion_url", "notion_page_url", "page_url", "url"] {
        for value in collect_string_fields_by_keys(payload, &[key], 24) {
            dedupe_push(&mut candidates, &value);
        }
    }
    if let Some(value) = maybe_string(payload.pointer("/notion/url")) {
        dedupe_push(&mut candidates, &value);
    }
    if let Some(value) = maybe_string(payload.pointer("/result/notion_url")) {
        dedupe_push(&mut candidates, &value);
    }
    if let Some(value) = maybe_string(payload.pointer("/data/notion_url")) {
        dedupe_push(&mut candidates, &value);
    }

    for candidate in candidates {
        let trimmed = candidate
            .trim_matches(|c: char| c == '"' || c == '\'' || c == '`')
            .trim();
        if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
            continue;
        }
        if trimmed.to_ascii_lowercase().contains("notion.so") {
            return Some(trimmed.to_string());
        }
    }

    let raw = payload.to_string();
    let re = Regex::new(r#"https?://[^\s"\\]+notion\.so[^\s"\\]*"#).ok()?;
    let found = re.find(&raw)?.as_str().trim();
    Some(found.to_string())
}

fn extract_scope_marker(payload: &Value) -> Option<String> {
    collect_string_fields_by_keys(payload, &["scope_marker"], 4)
        .into_iter()
        .next()
        .or_else(|| maybe_string(payload.pointer("/result/scope_marker")))
        .or_else(|| maybe_string(payload.pointer("/data/scope_marker")))
}

fn extract_status(payload: &Value) -> Option<String> {
    collect_string_fields_by_keys(payload, &["status"], 4)
        .into_iter()
        .next()
        .or_else(|| maybe_string(payload.pointer("/result/status")))
        .or_else(|| maybe_string(payload.pointer("/data/status")))
        .map(|s| s.to_lowercase())
}

fn truncate_chars(raw: &str, max_chars: usize) -> String {
    if raw.chars().count() <= max_chars {
        return raw.to_string();
    }
    let mut out = String::new();
    for ch in raw.chars().take(max_chars) {
        out.push(ch);
    }
    out.push_str("...");
    out
}

pub async fn trigger_program_webhook(
    request_text: &str,
    scope_marker_override: Option<String>,
) -> Result<AiDigestTriggerResult> {
    let webhook_url = resolve_program_webhook_url()?;
    let request_text = normalize_request_text(Some(request_text));
    let scope_marker = scope_marker_override.unwrap_or_else(build_scope_marker);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(webhook_timeout_secs()))
        .build()
        .context("failed to build HTTP client")?;

    let payload = json!({
        "text": request_text,
        "scope_marker": scope_marker,
        "source": "local_os_agent.program",
    });

    let response = client
        .post(&webhook_url)
        .json(&payload)
        .send()
        .await
        .with_context(|| format!("failed to call webhook {}", webhook_url))?;
    let status_code = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    let parsed_json = serde_json::from_str::<Value>(&body).ok();

    if !(200..300).contains(&status_code) {
        return Err(anyhow!(
            "webhook returned HTTP {}: {}",
            status_code,
            truncate_chars(body.trim(), 600)
        ));
    }

    if let Some(status) = parsed_json.as_ref().and_then(extract_status) {
        if matches!(status.as_str(), "error" | "failed" | "failure") {
            return Err(anyhow!(
                "webhook returned failed status='{}': {}",
                status,
                truncate_chars(body.trim(), 600)
            ));
        }
    }

    let notion_url = parsed_json.as_ref().and_then(extract_notion_url);
    let scope_marker = parsed_json
        .as_ref()
        .and_then(extract_scope_marker)
        .unwrap_or(scope_marker);

    Ok(AiDigestTriggerResult {
        ok: true,
        status_code,
        webhook_url,
        scope_marker,
        notion_url,
        response_json: parsed_json,
        response_text: if body.trim().is_empty() {
            None
        } else {
            Some(truncate_chars(body.trim(), 1200))
        },
    })
}

pub async fn trigger_program_webhook_human_summary(
    request_text: &str,
    scope_marker_override: Option<String>,
) -> Result<String> {
    let result = trigger_program_webhook(request_text, scope_marker_override).await?;
    Ok(format_human_summary(&result))
}

pub fn format_human_summary(result: &AiDigestTriggerResult) -> String {
    let extracted_notion_url = result
        .response_json
        .as_ref()
        .and_then(extract_notion_url)
        .or_else(|| result.notion_url.clone());
    let highlights = result
        .response_json
        .as_ref()
        .map(extract_headlines)
        .unwrap_or_default();

    let mut lines = Vec::new();
    lines.push("상태: ✅ 완료".to_string());
    lines.push("결과: AI 트렌드 요약/노션 정리 실행됨".to_string());
    if let Some(url) = extracted_notion_url {
        lines.push(format!("노션 링크: {}", url));
    } else {
        lines.push("노션 링크: (응답에서 확인되지 않음)".to_string());
    }
    if !highlights.is_empty() {
        lines.push("핵심 뉴스:".to_string());
        for (idx, title) in highlights.iter().take(5).enumerate() {
            lines.push(format!("{}. {}", idx + 1, title));
        }
    }
    lines.push(format!("run marker: {}", result.scope_marker));
    lines.push(format!("webhook 응답: HTTP {}", result.status_code));
    lines.join("\n")
}
