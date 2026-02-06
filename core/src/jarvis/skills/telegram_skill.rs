// TelegramSkill - Telegram messaging
// Send messages, manage chats

use super::{Skill, SkillContext, SkillResult};
use crate::jarvis::skills::metadata::*;
use async_trait::async_trait;
use serde_json::json;
use teloxide::Bot;
use teloxide::prelude::*;

pub struct TelegramSkill {
    bot_token: String,
}

impl TelegramSkill {
    pub fn new() -> Self {
        let bot_token = std::env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default();

        if bot_token.is_empty() {
            log::warn!("TELEGRAM_BOT_TOKEN not set - TelegramSkill will not be available");
        }

        Self { bot_token }
    }

    fn validate_token(&self) -> Result<(), String> {
        if self.bot_token.is_empty() {
            return Err("TELEGRAM_BOT_TOKEN not set".to_string());
        }
        Ok(())
    }
}

impl Default for TelegramSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for TelegramSkill {
    fn metadata(&self) -> SkillMetadata {
        SkillMetadata {
            name: "telegram".to_string(),
            description: "Send Telegram messages".to_string(),
            version: "1.0.0".to_string(),
            actions: vec![
                "send".to_string(),
                "get_updates".to_string(),
            ],
            requirements: SkillRequirements {
                env_vars: vec!["TELEGRAM_BOT_TOKEN".to_string()],
                required_bins: vec![],
                any_bins: vec![],
                platform: None, // Cross-platform
                config_keys: vec![],
            },
            tags: vec!["messaging".to_string(), "communication".to_string()],
        }
    }

    fn check_eligibility(&self) -> EligibilityResult {
        let requirements = self.metadata().requirements;
        check_requirements(&requirements)
    }

    async fn execute(&self, ctx: SkillContext) -> SkillResult {
        log::info!(
            "TelegramSkill executing action: {} (session: {})",
            ctx.action,
            ctx.session_key
        );

        match ctx.action.as_str() {
            "send" => self.send_message(ctx).await,
            "get_updates" => self.get_updates(ctx).await,
            _ => SkillResult::error(format!("Unknown action: {}", ctx.action)),
        }
    }

    fn requires_approval(&self, _action: &str) -> bool {
        // No actions require approval for Telegram
        false
    }
}

impl TelegramSkill {
    async fn send_message(&self, ctx: SkillContext) -> SkillResult {
        // Validate token
        if let Err(e) = self.validate_token() {
            return SkillResult::error(format!("Token validation failed: {}", e));
        }

        // Parse chat_id - can be string or number
        let chat_id_str = match ctx.params.get("chat_id") {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Number(n)) => n.to_string(),
            _ => {
                return SkillResult::error("Missing 'chat_id' parameter");
            }
        };

        // Parse as i64 (Telegram chat IDs are i64)
        let chat_id: i64 = match chat_id_str.parse() {
            Ok(id) => id,
            Err(e) => {
                return SkillResult::error(format!("Invalid chat_id format: {}", e));
            }
        };

        // Extract message text
        let text = match ctx.params.get("text").or_else(|| ctx.params.get("message")) {
            Some(serde_json::Value::String(s)) => s.clone(),
            _ => {
                return SkillResult::error("Missing 'text' or 'message' parameter");
            }
        };

        log::info!("Sending Telegram message to chat_id: {}", chat_id);

        // Create bot instance
        let bot = Bot::new(&self.bot_token);

        // Send message using teloxide
        match bot.send_message(ChatId(chat_id), text.clone()).await {
            Ok(sent_msg) => {
                log::info!("Message sent successfully to chat_id: {}", chat_id);
                SkillResult::success_with_data(
                    format!("Message sent to chat {}", chat_id),
                    json!({
                        "chat_id": chat_id,
                        "status": "sent",
                        "message_id": sent_msg.id.0,
                    }),
                )
            }
            Err(e) => {
                log::error!("Failed to send Telegram message: {}", e);
                SkillResult::error(format!("Failed to send message: {}", e))
            }
        }
    }

    async fn get_updates(&self, ctx: SkillContext) -> SkillResult {
        // Validate token
        if let Err(e) = self.validate_token() {
            return SkillResult::error(format!("Token validation failed: {}", e));
        }

        // Optional offset parameter
        let offset = ctx
            .params
            .get("offset")
            .and_then(|v| v.as_i64())
            .map(|o| o as i32);

        log::info!("Fetching Telegram updates (offset: {:?})", offset);

        // Create bot instance
        let bot = Bot::new(&self.bot_token);

        // Get updates
        let mut request = bot.get_updates();
        if let Some(offset_val) = offset {
            request = request.offset(offset_val);
        }

        match request.await {
            Ok(updates) => {
                let updates_json: Vec<serde_json::Value> = updates
                    .iter()
                    .map(|update| {
                        let msg_data = if let teloxide::types::UpdateKind::Message(ref msg) = update.kind {
                            Some(json!({
                                "message_id": msg.id.0,
                                "chat_id": msg.chat.id.0,
                                "text": msg.text(),
                                "date": msg.date.timestamp(),
                            }))
                        } else {
                            None
                        };

                        json!({
                            "update_id": update.id,
                            "message": msg_data,
                        })
                    })
                    .collect();

                log::info!("Retrieved {} updates", updates_json.len());

                SkillResult::success_with_data(
                    format!("Retrieved {} updates", updates_json.len()),
                    json!({
                        "count": updates_json.len(),
                        "updates": updates_json,
                    }),
                )
            }
            Err(e) => {
                log::error!("Failed to get Telegram updates: {}", e);
                SkillResult::error(format!("Failed to get updates: {}", e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::jarvis::models::context::UserContext;
    use std::time::Instant;

    fn test_user_context() -> UserContext {
        UserContext {
            active_app: None,
            active_window_title: None,
            last_activity: Instant::now(),
            activity_count: 0,
        }
    }

    #[tokio::test]
    async fn test_metadata() {
        let skill = TelegramSkill::new();
        let meta = skill.metadata();

        assert_eq!(meta.name, "telegram");
        assert!(meta.actions.contains(&"send".to_string()));
        assert!(meta.actions.contains(&"get_updates".to_string()));
        assert!(meta.requirements.env_vars.contains(&"TELEGRAM_BOT_TOKEN".to_string()));
    }

    #[test]
    fn test_requires_approval() {
        let skill = TelegramSkill::new();

        // No actions require approval as per specification
        assert!(!skill.requires_approval("send"));
        assert!(!skill.requires_approval("get_updates"));
    }

    #[test]
    fn test_token_validation() {
        // Test with empty token
        let skill = TelegramSkill {
            bot_token: String::new(),
        };
        assert!(skill.validate_token().is_err());

        // Test with valid token
        let skill = TelegramSkill {
            bot_token: "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11".to_string(),
        };
        assert!(skill.validate_token().is_ok());
    }

    #[tokio::test]
    async fn test_send_message_missing_token() {
        let skill = TelegramSkill {
            bot_token: String::new(),
        };

        let mut params = HashMap::new();
        params.insert("chat_id".to_string(), json!("12345"));
        params.insert("text".to_string(), json!("Test message"));

        let ctx = SkillContext {
            action: "send".to_string(),
            params,
            session_key: "test".to_string(),
            user_context: test_user_context(),
        };

        let result = skill.send_message(ctx).await;
        assert!(!result.success);
        assert!(result.message.contains("Token validation failed"));
    }

    #[tokio::test]
    async fn test_send_message_missing_chat_id() {
        let skill = TelegramSkill {
            bot_token: "test_token".to_string(),
        };

        let mut params = HashMap::new();
        params.insert("text".to_string(), json!("Test message"));

        let ctx = SkillContext {
            action: "send".to_string(),
            params,
            session_key: "test".to_string(),
            user_context: test_user_context(),
        };

        let result = skill.send_message(ctx).await;
        assert!(!result.success);
        assert!(result.message.contains("Missing 'chat_id' parameter"));
    }

    #[tokio::test]
    async fn test_send_message_missing_text() {
        let skill = TelegramSkill {
            bot_token: "test_token".to_string(),
        };

        let mut params = HashMap::new();
        params.insert("chat_id".to_string(), json!("12345"));

        let ctx = SkillContext {
            action: "send".to_string(),
            params,
            session_key: "test".to_string(),
            user_context: test_user_context(),
        };

        let result = skill.send_message(ctx).await;
        assert!(!result.success);
        assert!(result.message.contains("Missing 'text' or 'message' parameter"));
    }

    #[tokio::test]
    async fn test_send_message_invalid_chat_id() {
        let skill = TelegramSkill {
            bot_token: "test_token".to_string(),
        };

        let mut params = HashMap::new();
        params.insert("chat_id".to_string(), json!("not_a_number"));
        params.insert("text".to_string(), json!("Test message"));

        let ctx = SkillContext {
            action: "send".to_string(),
            params,
            session_key: "test".to_string(),
            user_context: test_user_context(),
        };

        let result = skill.send_message(ctx).await;
        assert!(!result.success);
        assert!(result.message.contains("Invalid chat_id format"));
    }

    #[tokio::test]
    async fn test_send_message_chat_id_as_number() {
        let skill = TelegramSkill {
            bot_token: "test_token".to_string(),
        };

        let mut params = HashMap::new();
        params.insert("chat_id".to_string(), json!(12345));
        params.insert("text".to_string(), json!("Test message"));

        let ctx = SkillContext {
            action: "send".to_string(),
            params,
            session_key: "test".to_string(),
            user_context: test_user_context(),
        };

        // This will fail with authentication error since we're using a fake token
        // but it validates parameter parsing
        let result = skill.send_message(ctx).await;
        assert!(!result.success);
        // Should not fail on parameter parsing
        assert!(!result.message.contains("Missing"));
        assert!(!result.message.contains("Invalid chat_id format"));
    }

    #[tokio::test]
    async fn test_get_updates_missing_token() {
        let skill = TelegramSkill {
            bot_token: String::new(),
        };

        let params = HashMap::new();

        let ctx = SkillContext {
            action: "get_updates".to_string(),
            params,
            session_key: "test".to_string(),
            user_context: test_user_context(),
        };

        let result = skill.get_updates(ctx).await;
        assert!(!result.success);
        assert!(result.message.contains("Token validation failed"));
    }

    #[tokio::test]
    async fn test_execute_routing() {
        let skill = TelegramSkill {
            bot_token: String::new(),
        };

        let params = HashMap::new();

        // Test unknown action
        let ctx = SkillContext {
            action: "unknown_action".to_string(),
            params: params.clone(),
            session_key: "test".to_string(),
            user_context: test_user_context(),
        };

        let result = skill.execute(ctx).await;
        assert!(!result.success);
        assert!(result.message.contains("Unknown action"));
    }
}
