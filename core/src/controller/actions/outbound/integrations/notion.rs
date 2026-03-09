use serde_json::json;

use crate::controller::actions::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) async fn handle_notion_write(
        plan: &serde_json::Value,
        goal: &str,
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
        action_data: &mut Option<serde_json::Value>,
    ) {
        let mut title = plan["title"]
            .as_str()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "Steer Note".to_string());
        let mut content = plan["content"]
            .as_str()
            .or_else(|| plan["text"].as_str())
            .map(|v| v.trim().to_string())
            .unwrap_or_default();
        if content.is_empty() {
            *description = "notion_write failed: missing content".to_string();
            *action_status_override = Some("failed");
            return;
        }

        let marker = Self::preferred_run_scope_marker(Some(goal)).unwrap_or_default();
        if Self::goal_targets_ai_news_to_notion(goal) {
            let topic = Self::infer_news_topic_from_goal(goal);
            let item_count = Self::infer_news_item_count(goal);
            if let Ok(items) = Self::fetch_google_ai_news(&topic, item_count).await {
                if items.len() >= 3 {
                    content = Self::build_ai_news_digest(
                        &topic,
                        &items,
                        if marker.is_empty() {
                            None
                        } else {
                            Some(marker.as_str())
                        },
                    );
                    if title.trim() == "Steer Note" {
                        title = format!(
                            "{} 뉴스 요약 {}",
                            topic,
                            chrono::Local::now().format("%Y-%m-%d %H:%M")
                        );
                    }
                }
            }
        }
        if !marker.is_empty() && !content.contains(&marker) {
            content = format!("{}\n{}", content.trim_end(), marker);
        }
        if title.trim().is_empty() {
            title = if marker.is_empty() {
                "Steer Note".to_string()
            } else {
                format!("Steer Note {}", marker)
            };
        }

        match Self::notion_write_text(&title, &content).await {
            Ok(result) => {
                *description = format!("Notion page created: {}", result.page_url);
                *action_status_override = Some("success");
                *action_data = Some(json!({
                    "proof": "notion_write_confirmed",
                    "page_id": result.page_id,
                    "page_url": result.page_url,
                    "title": result.title,
                    "content_len": content.chars().count(),
                    "content_preview": content.chars().take(1600).collect::<String>()
                }));
                if let Some(data) = action_data.as_ref() {
                    Self::log_evidence(
                        "notion",
                        "write",
                        &[
                            ("status", "confirmed".to_string()),
                            (
                                "page_id",
                                data.get("page_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            ),
                            (
                                "page_url",
                                data.get("page_url")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            ),
                            (
                                "title",
                                data.get("title")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            ),
                        ],
                    );
                }
            }
            Err(e) => {
                *description = format!("notion_write failed: {}", e);
                *action_status_override = Some("failed");
            }
        }
    }
}
