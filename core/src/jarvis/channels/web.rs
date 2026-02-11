// WebChannel - Web UI messages via HTTP POST

use super::{ChannelHandler, RateLimit};
use crate::jarvis::models::*;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebMessageRequest {
    pub text: String,
    pub session_id: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
}

pub struct WebChannel {
    rate_limit: RateLimit,
    _allowed_origins: Vec<String>,
}

impl WebChannel {
    pub fn new() -> Self {
        Self {
            rate_limit: RateLimit {
                max_per_minute: 30,
                burst_size: 10,
            },
            _allowed_origins: vec![
                "http://localhost:5173".to_string(),
                "http://localhost:5174".to_string(),
                "http://localhost:5680".to_string(),
            ],
        }
    }

    pub fn with_config(rate_limit: RateLimit, allowed_origins: Vec<String>) -> Self {
        Self {
            rate_limit,
            _allowed_origins: allowed_origins,
        }
    }
}

impl Default for WebChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChannelHandler for WebChannel {
    fn name(&self) -> &str {
        "web"
    }

    async fn receive(&self, raw: Vec<u8>) -> Result<Message> {
        // Deserialize JSON request
        let req: WebMessageRequest = serde_json::from_slice(&raw)?;

        // Generate session key
        let session_id = req.session_id.unwrap_or_else(|| {
            // Use a simple random session ID if not provided
            format!("web_{}", chrono::Utc::now().timestamp())
        });

        let session_key = format!("web:{}", session_id);

        Ok(Message {
            session_key,
            text: req.text,
            metadata: req.metadata.unwrap_or_default(),
        })
    }

    async fn send(&self, session_key: &str, response: Response) -> Result<()> {
        // For web, the response is returned synchronously via HTTP
        // This method is mainly for logging/tracking purposes
        log::debug!(
            "WebChannel send to {}: {} ({})",
            session_key,
            response.message,
            if response.success { "success" } else { "error" }
        );
        Ok(())
    }

    fn rate_limit(&self) -> RateLimit {
        self.rate_limit.clone()
    }

    fn is_allowed(&self, _user_id: &str) -> bool {
        // Web channel allows all localhost requests
        // CORS is handled by API server
        true
    }
}
