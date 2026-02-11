// APIChannel - Legacy API endpoint for backward compatibility

use super::{ChannelHandler, RateLimit};
use crate::jarvis::models::*;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequest {
    pub text: String,
    pub source: Option<String>,
    pub params: Option<HashMap<String, serde_json::Value>>,
}

pub struct APIChannel {
    rate_limit: RateLimit,
}

impl APIChannel {
    pub fn new() -> Self {
        Self {
            rate_limit: RateLimit {
                max_per_minute: 60,  // Higher limit for API
                burst_size: 20,
            },
        }
    }
}

impl Default for APIChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChannelHandler for APIChannel {
    fn name(&self) -> &str {
        "api"
    }

    async fn receive(&self, raw: Vec<u8>) -> Result<Message> {
        // Deserialize legacy API request
        let req: ApiRequest = serde_json::from_slice(&raw)?;

        // Use fixed session key for API (stateless)
        let session_key = "api:legacy".to_string();

        // Convert params to metadata
        let mut metadata = HashMap::new();
        if let Some(params) = req.params {
            for (key, value) in params {
                metadata.insert(key, value.to_string());
            }
        }

        if let Some(source) = req.source {
            metadata.insert("source".to_string(), source);
        }

        Ok(Message {
            session_key,
            text: req.text,
            metadata,
        })
    }

    async fn send(&self, session_key: &str, response: Response) -> Result<()> {
        // API responses are returned synchronously
        log::debug!(
            "APIChannel send to {}: {} ({})",
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
        // API channel allows all requests (protected by API server auth)
        true
    }
}
