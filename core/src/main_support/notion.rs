use super::summarize_prompt;
use serde_json::json;

fn notion_text(text: &str, max_chars: usize) -> String {
    summarize_prompt(text, max_chars).replace('\n', " ")
}

fn notion_heading_block(text: &str, level: u8) -> serde_json::Value {
    let rich = json!([{
        "type": "text",
        "text": { "content": notion_text(text, 180) }
    }]);
    match level {
        1 => json!({
            "object": "block",
            "type": "heading_1",
            "heading_1": { "rich_text": rich }
        }),
        3 => json!({
            "object": "block",
            "type": "heading_3",
            "heading_3": { "rich_text": rich }
        }),
        _ => json!({
            "object": "block",
            "type": "heading_2",
            "heading_2": { "rich_text": rich }
        }),
    }
}

fn notion_paragraph_block(text: &str) -> serde_json::Value {
    json!({
        "object": "block",
        "type": "paragraph",
        "paragraph": {
            "rich_text": [{
                "type": "text",
                "text": { "content": notion_text(text, 1800) }
            }]
        }
    })
}

fn notion_bulleted_item_block(text: &str) -> serde_json::Value {
    json!({
        "object": "block",
        "type": "bulleted_list_item",
        "bulleted_list_item": {
            "rich_text": [{
                "type": "text",
                "text": { "content": notion_text(text, 1800) }
            }]
        }
    })
}

fn notion_numbered_item_block(text: &str) -> serde_json::Value {
    json!({
        "object": "block",
        "type": "numbered_list_item",
        "numbered_list_item": {
            "rich_text": [{
                "type": "text",
                "text": { "content": notion_text(text, 1800) }
            }]
        }
    })
}

pub(super) fn notion_divider_block() -> serde_json::Value {
    json!({
        "object": "block",
        "type": "divider",
        "divider": {}
    })
}

pub(super) fn build_gmail_digest_blocks(
    stamp: &str,
    emails: &[super::digest::DigestEmail],
    digest: &super::digest::DigestSummary,
) -> Vec<serde_json::Value> {
    let mut blocks = vec![
        notion_heading_block("Gmail Digest", 2),
        notion_paragraph_block(&format!("생성 시각: {}", stamp)),
        notion_paragraph_block(&format!("수집 건수: {}", emails.len())),
        notion_heading_block("전체 요약", 3),
        notion_paragraph_block(&digest.overall_summary),
        notion_heading_block("메일별 요약", 3),
    ];

    for line in &digest.per_email_lines {
        let normalized = line
            .split_once(". ")
            .map(|(_, rest)| rest)
            .unwrap_or(line.as_str());
        blocks.push(notion_numbered_item_block(normalized));
    }

    blocks.push(notion_heading_block("원문 인덱스", 3));
    for email in emails {
        let line = format!(
            "{} | {} | {}",
            summarize_prompt(&email.from, 80),
            summarize_prompt(&email.subject, 100),
            summarize_prompt(&email.date, 80)
        );
        blocks.push(notion_bulleted_item_block(&line));
    }

    blocks
}
