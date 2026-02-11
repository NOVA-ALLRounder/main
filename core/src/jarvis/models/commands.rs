// Command and Response models

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Channel-agnostic message structure
/// Used by ChannelRouter to normalize messages from different sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Session key in format "channel:identifier" (e.g., "telegram:123", "web:session_abc")
    pub session_key: String,

    /// Message text content
    pub text: String,

    /// Channel-specific metadata (attachments, reply_to, etc.)
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub text: String,
    pub source: CommandSource,
    pub params: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandSource {
    Telegram,
    TauriUI,
    API,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl Response {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: None,
        }
    }

    pub fn success_with_data(message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: Some(data),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
        }
    }
}
