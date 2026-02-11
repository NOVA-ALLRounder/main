// Telegram Tool - Send messages and notifications via Telegram

use super::tool_trait::{Tool, ToolParams, ToolResult};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;

pub struct TelegramTool {
    token: Option<String>,
    default_chat_id: Option<i64>,
    client: reqwest::Client,
}

impl TelegramTool {
    pub fn new() -> Self {
        // Load from env
        let token = std::env::var("TELEGRAM_BOT_TOKEN").ok();
        let default_chat_id = std::env::var("TELEGRAM_USER_ID")
            .ok()
            .and_then(|id| id.parse().ok());

        Self {
            token,
            default_chat_id,
            client: reqwest::Client::new(),
        }
    }

    async fn send_message(&self, chat_id: i64, text: &str) -> Result<()> {
        let token = self
            .token
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("TELEGRAM_BOT_TOKEN not set"))?;

        let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
        let body = json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "Markdown"
        });

        self.client
            .post(&url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn send_message_with_buttons(
        &self,
        chat_id: i64,
        text: &str,
        buttons: Vec<String>,
    ) -> Result<()> {
        let token = self
            .token
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("TELEGRAM_BOT_TOKEN not set"))?;

        let url = format!("https://api.telegram.org/bot{}/sendMessage", token);

        // Create inline keyboard
        let keyboard = buttons
            .iter()
            .map(|btn| {
                vec![json!({
                    "text": btn,
                    "callback_data": btn
                })]
            })
            .collect::<Vec<_>>();

        let body = json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "Markdown",
            "reply_markup": {
                "inline_keyboard": keyboard
            }
        });

        self.client
            .post(&url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}

#[async_trait]
impl Tool for TelegramTool {
    fn name(&self) -> &str {
        "telegram"
    }

    fn description(&self) -> &str {
        "Send messages and notifications via Telegram"
    }

    async fn execute(&self, params: ToolParams) -> Result<ToolResult> {
        log::debug!("TelegramTool executing action: {}", params.action);

        match params.action.as_str() {
            "send_message" => {
                let text = params.get_string("text")?;
                let chat_id = if let Ok(id) = params.get_string("chat_id") {
                    id.parse::<i64>()?
                } else {
                    self.default_chat_id
                        .ok_or_else(|| anyhow::anyhow!("No chat_id provided and TELEGRAM_USER_ID not set"))?
                };

                self.send_message(chat_id, &text).await?;
                Ok(ToolResult::success("Message sent"))
            }
            "ask_approval" => {
                let question = params.get_string("question")?;
                let options = params.get_json("options")?;
                let chat_id = if let Ok(id) = params.get_string("chat_id") {
                    id.parse::<i64>()?
                } else {
                    self.default_chat_id
                        .ok_or_else(|| anyhow::anyhow!("No chat_id provided and TELEGRAM_USER_ID not set"))?
                };

                // Parse options array
                let buttons: Vec<String> = if let Some(arr) = options.as_array() {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                } else {
                    return Err(anyhow::anyhow!("options must be an array"));
                };

                self.send_message_with_buttons(chat_id, &question, buttons)
                    .await?;
                Ok(ToolResult::success("Approval request sent"))
            }
            _ => Err(anyhow::anyhow!("Unknown action: {}", params.action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_telegram_tool_creation() {
        let tool = TelegramTool::new();
        assert_eq!(tool.name(), "telegram");
    }

    #[tokio::test]
    async fn test_send_message_without_token() {
        // Clear env vars for this test
        std::env::remove_var("TELEGRAM_BOT_TOKEN");

        let tool = TelegramTool::new();
        let params = ToolParams::new("send_message")
            .with_param("text", "test")
            .with_param("chat_id", "123456");

        let result = tool.execute(params).await;
        // Should fail without token
        assert!(result.is_err());
    }
}
