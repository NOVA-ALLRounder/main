// Channel system - Multi-protocol abstraction inspired by OpenClaw
// Normalizes messages from different sources (Telegram, Web, Voice, API)

pub mod api;
pub mod telegram;
pub mod web;

use crate::jarvis::models::*;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Channel-agnostic message handler trait
#[async_trait]
pub trait ChannelHandler: Send + Sync {
    /// Channel name (telegram, web, voice, api)
    fn name(&self) -> &str;

    /// Normalize channel-specific input ??Message
    async fn receive(&self, raw: Vec<u8>) -> Result<Message>;

    /// Send response back to channel
    async fn send(&self, session_key: &str, response: Response) -> Result<()>;

    /// Rate limit config (messages per minute)
    fn rate_limit(&self) -> RateLimit;

    /// ACL check - is this user allowed?
    fn is_allowed(&self, user_id: &str) -> bool;
}

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimit {
    pub max_per_minute: u32,
    pub burst_size: u32,
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            max_per_minute: 30,
            burst_size: 5,
        }
    }
}

/// Rate limiter tracker
struct RateLimitTracker {
    requests: Vec<Instant>,
}

impl RateLimitTracker {
    fn new() -> Self {
        Self {
            requests: Vec::new(),
        }
    }

    fn check_and_update(&mut self, limit: &RateLimit) -> bool {
        let now = Instant::now();
        let one_minute_ago = now - Duration::from_secs(60);

        // Remove old requests
        self.requests.retain(|&req_time| req_time > one_minute_ago);

        // Check limit
        if self.requests.len() >= limit.max_per_minute as usize {
            return false;
        }

        // Add current request
        self.requests.push(now);
        true
    }
}

/// Channel Router - Routes messages from multiple channels to orchestrator
pub struct ChannelRouter {
    channels: HashMap<String, Arc<dyn ChannelHandler>>,
    rate_limiters: Arc<RwLock<HashMap<String, RateLimitTracker>>>,
    orchestrator: Option<Arc<crate::jarvis::JarvisOrchestrator>>,
}

impl ChannelRouter {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
            rate_limiters: Arc::new(RwLock::new(HashMap::new())),
            orchestrator: None,
        }
    }

    /// Register a channel handler
    pub fn register(&mut self, handler: Arc<dyn ChannelHandler>) {
        let name = handler.name().to_string();
        log::info!("Registered channel: {}", name);
        self.channels.insert(name, handler);
    }

    /// Set orchestrator (called after both are created)
    pub fn set_orchestrator(&mut self, orchestrator: Arc<crate::jarvis::JarvisOrchestrator>) {
        self.orchestrator = Some(orchestrator);
    }

    /// Route message from channel to orchestrator
    pub async fn route(&self, channel_name: &str, raw: Vec<u8>) -> Result<Response> {
        // Get channel handler
        let channel = self
            .channels
            .get(channel_name)
            .ok_or_else(|| anyhow::anyhow!("Channel not found: {}", channel_name))?;

        // Normalize message
        let msg = channel.receive(raw).await?;

        // Check rate limit
        if !self.check_rate_limit(&msg.session_key, channel.rate_limit()).await {
            return Ok(Response::error("Rate limit exceeded. Please wait a moment."));
        }

        // Extract user_id from session_key (format: "channel:user_id")
        let user_id = msg
            .session_key
            .split(':')
            .nth(1)
            .unwrap_or("unknown");

        // Check ACL
        if !channel.is_allowed(user_id) {
            return Ok(Response::error("Access denied"));
        }

        // Route to orchestrator with enhanced pipeline (Intent ??Plan ??Skills)
        let orchestrator = self
            .orchestrator
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Orchestrator not set"))?;

        let response = orchestrator.handle_message_enhanced(msg).await?;

        Ok(response)
    }

    /// Check and update rate limit
    async fn check_rate_limit(&self, session_key: &str, limit: RateLimit) -> bool {
        let mut limiters = self.rate_limiters.write().await;

        let tracker = limiters
            .entry(session_key.to_string())
            .or_insert_with(RateLimitTracker::new);

        tracker.check_and_update(&limit)
    }

    /// List registered channels
    pub fn list_channels(&self) -> Vec<&str> {
        self.channels.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for ChannelRouter {
    fn default() -> Self {
        Self::new()
    }
}
