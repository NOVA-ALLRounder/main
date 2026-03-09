use super::{
    notion::{build_gmail_digest_blocks, notion_divider_block},
    summarize_prompt,
};
use anyhow::anyhow;
use chrono::Utc;
use local_os_agent::{env_flag, integrations, llm_gateway};
use serde_json::json;
use std::collections::HashMap;
use tracing::warn;

#[derive(Debug, Clone)]
pub(super) struct DigestEmail {
    pub(super) id: String,
    pub(super) from: String,
    pub(super) subject: String,
    pub(super) date: String,
    pub(super) snippet: String,
}

#[derive(Debug, Clone)]
pub(super) struct DigestSummary {
    pub(super) overall_summary: String,
    pub(super) per_email_lines: Vec<String>,
}

fn extract_email_header(raw: &str, header: &str) -> Option<String> {
    let prefix = format!("{}:", header);
    raw.lines()
        .find_map(|line| line.strip_prefix(&prefix).map(|v| v.trim().to_string()))
}

fn extract_email_snippet(raw: &str) -> String {
    let snippet = raw
        .split("\n---\n\n")
        .nth(1)
        .unwrap_or("")
        .replace('\n', " ")
        .trim()
        .to_string();
    summarize_prompt(&snippet, 220)
}

fn fallback_digest_summary(emails: &[DigestEmail]) -> DigestSummary {
    let per_email_lines = emails
        .iter()
        .enumerate()
        .map(|(idx, email)| {
            format!(
                "{}. [MEDIUM] {} | {}",
                idx + 1,
                summarize_prompt(&email.subject, 72),
                summarize_prompt(&email.snippet, 120)
            )
        })
        .collect::<Vec<_>>();

    let overall_summary = format!(
        "최근 메일 {}건을 수집했습니다. 중요도 분류가 불완전하므로 제목/발신자 기준으로 우선 확인하세요.",
        emails.len()
    );
    DigestSummary {
        overall_summary,
        per_email_lines,
    }
}

fn parse_retry_after_seconds(err: &str) -> Option<u64> {
    let lower = err.to_lowercase();
    let marker = "try again in ";
    let start = lower.find(marker)? + marker.len();
    let remain = &lower[start..];
    let mut numeric = String::new();
    for ch in remain.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            numeric.push(ch);
            continue;
        }
        break;
    }
    if numeric.is_empty() {
        return None;
    }
    let secs = numeric.parse::<f64>().ok()?;
    Some(secs.ceil().max(1.0) as u64)
}

async fn summarize_digest_with_llm(
    llm: &dyn llm_gateway::LLMClient,
    emails: &[DigestEmail],
) -> anyhow::Result<DigestSummary> {
    let payload = emails
        .iter()
        .map(|email| {
            json!({
                "id": email.id,
                "from": summarize_prompt(&email.from, 80),
                "subject": summarize_prompt(&email.subject, 120),
                "date": summarize_prompt(&email.date, 80),
                "snippet": summarize_prompt(&email.snippet, 120),
            })
        })
        .collect::<Vec<_>>();

    let user_prompt = format!(
        "다음 이메일 목록을 한국어로 요약하세요.\n\
출력은 반드시 JSON만 반환하세요.\n\
스키마:\n\
{{\n\
  \"overall_summary\": \"문장 2~3개\",\n\
  \"items\": [\n\
    {{\"id\":\"원본 id\", \"summary\":\"핵심 요약 1문장\", \"importance\":\"high|medium|low\", \"action_item\":\"필요한 후속조치 1문장\"}}\n\
  ]\n\
}}\n\
규칙:\n\
- items 길이는 입력과 동일\n\
- importance는 high|medium|low 중 하나\n\
- summary/action_item은 80자 이내\n\
\n입력:\n{}",
        serde_json::to_string(&payload)?
    );

    let messages = vec![
        json!({
            "role": "system",
            "content": "You are a strict JSON generator. Never output markdown or extra text."
        }),
        json!({
            "role": "user",
            "content": user_prompt
        }),
    ];

    let mut raw = String::new();
    let max_attempts = 2usize;
    for attempt in 0..max_attempts {
        match llm.chat_completion(messages.clone()).await {
            Ok(v) => {
                raw = v;
                break;
            }
            Err(e) => {
                let msg = e.to_string();
                let rate_limited = msg.to_lowercase().contains("rate_limit") || msg.contains("429");
                if !rate_limited || attempt + 1 >= max_attempts {
                    return Err(anyhow!(msg));
                }
                let wait_sec = parse_retry_after_seconds(&msg).unwrap_or(5).min(45);
                warn!(
                    "LLM digest rate-limited; retrying in {}s (attempt {}/{})",
                    wait_sec,
                    attempt + 1,
                    max_attempts
                );
                tokio::time::sleep(std::time::Duration::from_secs(wait_sec)).await;
            }
        }
    }
    if raw.is_empty() {
        return Err(anyhow!("LLM digest returned empty response"));
    }

    let parsed = llm_gateway::recover_json(&raw)
        .ok_or_else(|| anyhow!("LLM digest output is not valid JSON"))?;

    let mut by_id: HashMap<String, (String, String, String)> = HashMap::new();
    if let Some(items) = parsed.get("items").and_then(|v| v.as_array()) {
        for item in items {
            let id = item
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() {
                continue;
            }
            let summary = item
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let importance = item
                .get("importance")
                .and_then(|v| v.as_str())
                .unwrap_or("medium")
                .trim()
                .to_lowercase();
            let action_item = item
                .get("action_item")
                .and_then(|v| v.as_str())
                .unwrap_or("없음")
                .trim()
                .to_string();
            by_id.insert(id, (summary, importance, action_item));
        }
    }

    let mut per_email_lines = Vec::new();
    for (idx, email) in emails.iter().enumerate() {
        let (summary, importance, action_item) =
            by_id.get(&email.id).cloned().unwrap_or_else(|| {
                (
                    summarize_prompt(&email.snippet, 80),
                    "medium".to_string(),
                    "추가 확인 필요".to_string(),
                )
            });
        let importance_tag = match importance.as_str() {
            "high" => "HIGH",
            "low" => "LOW",
            _ => "MEDIUM",
        };
        per_email_lines.push(format!(
            "{}. [{}] {} | {} | 조치: {}",
            idx + 1,
            importance_tag,
            summarize_prompt(&email.subject, 72),
            summarize_prompt(&summary, 100),
            summarize_prompt(&action_item, 72)
        ));
    }

    let overall_summary = if let Some(text) = parsed.get("overall_summary").and_then(|v| v.as_str())
    {
        text.trim().to_string()
    } else if let Some(arr) = parsed.get("overall_summary").and_then(|v| v.as_array()) {
        arr.iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    } else {
        String::new()
    };

    if overall_summary.is_empty() {
        return Err(anyhow!("LLM digest JSON missing overall_summary"));
    }

    Ok(DigestSummary {
        overall_summary,
        per_email_lines,
    })
}

pub(crate) async fn run_gmail_digest_pipeline(
    count: u32,
    llm: Option<&dyn llm_gateway::LLMClient>,
) -> anyhow::Result<()> {
    let count = count.clamp(1, 10);
    let gmail = integrations::gmail::GmailClient::new().await?;
    let listed = gmail.list_messages(count).await?;
    if listed.is_empty() {
        return Err(anyhow!("gmail inbox is empty"));
    }

    let mut emails = Vec::new();
    for (id, subject, from) in listed.into_iter().take(count as usize) {
        let raw = gmail.get_message(&id).await.unwrap_or_default();
        let date = extract_email_header(&raw, "Date").unwrap_or_else(|| "(unknown)".to_string());
        emails.push(DigestEmail {
            id,
            from,
            subject,
            date,
            snippet: extract_email_snippet(&raw),
        });
    }

    let digest = if let Some(client) = llm {
        match summarize_digest_with_llm(client, &emails).await {
            Ok(v) => v,
            Err(e) => {
                warn!("LLM digest failed; using fallback summarizer: {}", e);
                fallback_digest_summary(&emails)
            }
        }
    } else {
        fallback_digest_summary(&emails)
    };

    let now = Utc::now();
    let stamp = now.format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let notion_blocks = build_gmail_digest_blocks(&stamp, &emails, &digest);

    let notion_client = integrations::notion::NotionClient::from_env()?;
    let page_id = std::env::var("NOTION_PAGE_ID").unwrap_or_default();
    let database_id = std::env::var("NOTION_DATABASE_ID").unwrap_or_default();
    let append_to_page = env_flag("NOTION_APPEND_TO_PAGE");
    let notion_url;

    if !database_id.trim().is_empty() {
        let title = format!("Gmail Digest {}", now.format("%Y%m%d_%H%M%S"));
        let created_page_id = notion_client
            .create_database_page_with_children(&database_id, &title, &notion_blocks)
            .await?;
        notion_url = format!("https://www.notion.so/{}", created_page_id.replace('-', ""));
    } else if !page_id.trim().is_empty() {
        if append_to_page {
            let mut append_blocks = vec![notion_divider_block()];
            append_blocks.extend(notion_blocks);
            notion_client
                .append_blocks(&page_id, &append_blocks)
                .await?;
            notion_url = format!("https://www.notion.so/{}", page_id.replace('-', ""));
        } else {
            let title = format!("Gmail Digest {}", now.format("%Y%m%d_%H%M%S"));
            let created_page_id = notion_client
                .create_child_page_with_children(&page_id, &title, &notion_blocks)
                .await?;
            notion_url = format!("https://www.notion.so/{}", created_page_id.replace('-', ""));
        }
    } else {
        return Err(anyhow!("NOTION_PAGE_ID or NOTION_DATABASE_ID must be set"));
    }

    let mut telegram_message = format!(
        "Gmail 요약 완료\n시간: {}\n건수: {}\n전체: {}\n",
        stamp,
        emails.len(),
        digest.overall_summary
    );
    for line in digest.per_email_lines.iter().take(5) {
        telegram_message.push_str(line);
        telegram_message.push('\n');
    }
    telegram_message.push_str(&format!("Notion: {}", notion_url));
    telegram_message = summarize_prompt(&telegram_message, 3400);

    let bot = integrations::telegram::TelegramBot::from_env()?;
    bot.send_plain(&telegram_message).await?;

    println!("✅ Gmail digest pipeline completed.");
    println!("   - emails: {}", emails.len());
    println!("   - notion: {}", notion_url);
    println!("   - telegram: sent");

    Ok(())
}
