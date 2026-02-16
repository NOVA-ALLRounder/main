use anyhow::{anyhow, Result};
use chrono::Local;
use local_os_agent::integrations::{
    gmail::{GmailClient, GmailMessageSummary},
    notion::NotionClient,
    telegram::TelegramBot,
};

fn truncate_chars(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

fn score_message(msg: &GmailMessageSummary) -> i32 {
    let mut score = 0i32;
    let text = format!("{} {} {}", msg.subject.to_lowercase(), msg.from.to_lowercase(), msg.snippet.to_lowercase());
    let high = ["urgent", "asap", "action required", "required", "invoice", "payment", "deadline", "security", "alert", "escalation"];
    let mid = ["review", "meeting", "request", "follow up", "confirm", "approval", "contract", "schedule"];
    for k in high {
        if text.contains(k) {
            score += 5;
        }
    }
    for k in mid {
        if text.contains(k) {
            score += 2;
        }
    }
    if text.contains("noreply") {
        score -= 1;
    }
    score
}

fn extract_actions(messages: &[GmailMessageSummary]) -> Vec<String> {
    let mut out = Vec::new();
    for m in messages {
        let t = format!("{} {}", m.subject.to_lowercase(), m.snippet.to_lowercase());
        if t.contains("action required") || t.contains("required") || t.contains("asap") || t.contains("urgent") {
            out.push(format!("- Reply needed: {}", m.subject));
            continue;
        }
        if t.contains("meeting") || t.contains("calendar") || t.contains("schedule") {
            out.push(format!("- Check calendar item: {}", m.subject));
            continue;
        }
        if t.contains("invoice") || t.contains("payment") || t.contains("bill") {
            out.push(format!("- Finance check: {}", m.subject));
            continue;
        }
    }
    if out.is_empty() {
        out.push("- No obvious urgent action found. Review top item first.".to_string());
    }
    out
}

fn build_summary(now: chrono::DateTime<Local>, selected: &[GmailMessageSummary], query: &str) -> String {
    let mut summary = String::new();
    summary.push_str(&format!(
        "# Email Summary ({})\n\n",
        now.format("%Y-%m-%d %H:%M")
    ));
    summary.push_str(&format!("- Query: `{}`\n", query));
    summary.push_str(&format!("- Processed: {} message(s)\n\n", selected.len()));
    summary.push_str("## Top Messages\n");
    for (idx, m) in selected.iter().enumerate() {
        summary.push_str(&format!(
            "{}. **{}**\n- From: {}\n- Date: {}\n- Snippet: {}\n\n",
            idx + 1,
            m.subject,
            m.from,
            m.date,
            truncate_chars(m.snippet.trim(), 260)
        ));
    }
    summary.push_str("## Suggested Actions\n");
    for action in extract_actions(selected) {
        summary.push_str(&action);
        summary.push('\n');
    }
    summary
}

async fn write_to_notion(content: &str, title: &str) -> Result<String> {
    let client = NotionClient::from_env()?;
    if let Ok(db) = std::env::var("NOTION_DATABASE_ID") {
        if !db.trim().is_empty() {
            let page_id = client.create_page(&db, title, content).await?;
            return Ok(format!("https://www.notion.so/{}", page_id.replace('-', "")));
        }
    }
    if let Ok(page_id) = std::env::var("NOTION_PAGE_ID") {
        if !page_id.trim().is_empty() {
            client.append_to_page(&page_id, content).await?;
            return Ok(format!("https://www.notion.so/{}", page_id.replace('-', "")));
        }
    }
    Err(anyhow!("NOTION_DATABASE_ID or NOTION_PAGE_ID is required"))
}

#[tokio::main]
async fn main() -> Result<()> {
    local_os_agent::load_env();
    let now = Local::now();

    let query = std::env::var("EMAIL_WORKFLOW_GMAIL_QUERY")
        .unwrap_or_else(|_| "is:unread newer_than:7d category:primary".to_string());
    let fetch_limit = std::env::var("EMAIL_WORKFLOW_FETCH_LIMIT")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(12);
    let select_count = std::env::var("EMAIL_WORKFLOW_SELECT_COUNT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(3);

    println!("=== Email -> Notion -> Telegram ===");
    println!("Started: {}", now.format("%Y-%m-%d %H:%M:%S"));
    println!("Query: {}", query);

    let gmail = GmailClient::new().await?;
    let mut messages = gmail
        .list_messages_enriched(fetch_limit, Some(&query))
        .await?;
    if messages.is_empty() {
        return Err(anyhow!("No recent emails found with query: {}", query));
    }

    messages.sort_by(|a, b| score_message(b).cmp(&score_message(a)));
    let selected: Vec<GmailMessageSummary> = messages.into_iter().take(select_count).collect();
    let summary = build_summary(now, &selected, &query);

    let notion_title = format!("Email Summary {}", now.format("%Y-%m-%d %H:%M"));
    let page_url = write_to_notion(&summary, &notion_title).await?;

    let telegram = TelegramBot::from_env()?;
    let top = selected
        .first()
        .map(|m| truncate_chars(&m.subject, 80))
        .unwrap_or_else(|| "N/A".to_string());
    let telegram_text = format!(
        "Email workflow complete\n- Selected: {} / fetched {}\n- Top: {}\n- Notion: {}",
        selected.len(),
        fetch_limit,
        top,
        page_url
    );
    telegram.send(&telegram_text).await?;

    println!("Selected emails: {}", selected.len());
    println!("Notion: {}", page_url);
    println!("Telegram notify: sent");
    println!("RESULT: PASS");
    Ok(())
}
