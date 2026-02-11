// TelegramChannel - Telegram Bot API integration

use super::{ChannelHandler, RateLimit};
use crate::jarvis::models::*;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Simplified Telegram Update structure
/// (Full structure is complex, we only parse what we need)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramUpdate {
    pub update_id: i64,
    pub message: Option<TelegramMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramMessage {
    pub message_id: i64,
    pub from: Option<TelegramUser>,
    pub chat: TelegramChat,
    pub text: Option<String>,
    pub voice: Option<TelegramVoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramUser {
    pub id: i64,
    pub first_name: String,
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramChat {
    pub id: i64,
    #[serde(rename = "type")]
    pub chat_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramVoice {
    pub file_id: String,
    pub duration: i32,
}

pub struct TelegramChannel {
    bot_token: Option<String>,
    allowed_users: Vec<String>,
    rate_limit: RateLimit,
}

impl TelegramChannel {
    pub fn new() -> Self {
        // Try to get bot token from env
        let bot_token = std::env::var("TELEGRAM_BOT_TOKEN").ok();

        if bot_token.is_none() {
            log::warn!("TELEGRAM_BOT_TOKEN not set. Telegram channel will reject all messages.");
        }

        // Try to get allowed users from env (comma-separated chat IDs)
        let allowed_users = std::env::var("TELEGRAM_ALLOWED_USERS")
            .ok()
            .map(|s| s.split(',').map(|id| id.trim().to_string()).collect())
            .unwrap_or_default();

        Self {
            bot_token,
            allowed_users,
            rate_limit: RateLimit {
                max_per_minute: 10,  // Conservative limit for Telegram
                burst_size: 3,
            },
        }
    }

    pub fn with_config(
        bot_token: Option<String>,
        allowed_users: Vec<String>,
        rate_limit: RateLimit,
    ) -> Self {
        Self {
            bot_token,
            allowed_users,
            rate_limit,
        }
    }

    /// Send message to Telegram user
    pub async fn send_telegram_message(&self, chat_id: i64, text: &str) -> Result<()> {
        let bot_token = self
            .bot_token
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Telegram bot token not configured"))?;

        let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);

        let body = serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "Markdown",
        });

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Telegram API error: {}", error_text);
        }

        Ok(())
    }
}

impl Default for TelegramChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChannelHandler for TelegramChannel {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn receive(&self, raw: Vec<u8>) -> Result<Message> {
        // Parse Telegram webhook update
        let update: TelegramUpdate = serde_json::from_slice(&raw)?;

        let message = update
            .message
            .ok_or_else(|| anyhow::anyhow!("No message in update"))?;

        // Extract text (or placeholder for voice)
        let text = if let Some(text) = message.text {
            text
        } else if message.voice.is_some() {
            // Voice message - will be handled in Phase 5
            "[Voice message - transcription not yet implemented]".to_string()
        } else {
            anyhow::bail!("Unsupported message type (only text and voice supported)");
        };

        // Generate session key
        let chat_id = message.chat.id;
        let session_key = format!("telegram:{}", chat_id);

        // Build metadata
        let mut metadata = HashMap::new();
        metadata.insert("chat_id".to_string(), chat_id.to_string());
        metadata.insert("message_id".to_string(), message.message_id.to_string());
        metadata.insert("chat_type".to_string(), message.chat.chat_type.clone());

        if let Some(user) = message.from {
            metadata.insert("user_id".to_string(), user.id.to_string());
            metadata.insert("user_name".to_string(), user.first_name);
            if let Some(username) = user.username {
                metadata.insert("username".to_string(), username);
            }
        }

        if message.voice.is_some() {
            metadata.insert("has_voice".to_string(), "true".to_string());
        }

        Ok(Message {
            session_key,
            text,
            metadata,
        })
    }

    async fn send(&self, session_key: &str, response: Response) -> Result<()> {
        // Extract chat_id from session_key (format: "telegram:123456")
        let chat_id: i64 = session_key
            .strip_prefix("telegram:")
            .ok_or_else(|| anyhow::anyhow!("Invalid session key format"))?
            .parse()?;

        // Format response message
        let text = if response.success {
            format!("??{}", response.message)
        } else {
            format!("??{}", response.message)
        };

        // Send to Telegram
        self.send_telegram_message(chat_id, &text).await?;

        log::info!("Sent Telegram message to chat {}", chat_id);

        Ok(())
    }

    fn rate_limit(&self) -> RateLimit {
        self.rate_limit.clone()
    }

    fn is_allowed(&self, user_id: &str) -> bool {
        // If allowed_users is empty, allow all
        if self.allowed_users.is_empty() {
            return true;
        }

        // Check if user_id is in allowed list
        self.allowed_users.contains(&user_id.to_string())
    }
}
