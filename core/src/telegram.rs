mod polling;
mod reporting;
#[cfg(test)]
mod tests;
mod types;

use crate::llm_gateway::LLMClient;
use crate::telegram_transport;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Semaphore;

#[cfg(test)]
pub(crate) use reporting::RunResultContext;
use types::{GetUpdatesResponse, TelegramApiStatusResponse, Update};
pub(crate) use types::{Message, User};

pub struct TelegramBot {
    token: String,
    allowed_user_id: Option<u64>,
    client: reqwest::Client,
    poll_timeout_sec: u64,
    llm: Arc<dyn LLMClient>,
    tx_analyzer: Option<mpsc::Sender<String>>,
}

impl TelegramBot {
    pub(crate) fn env_truthy_default(key: &str, default_value: bool) -> bool {
        match std::env::var(key) {
            Ok(v) => matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ),
            Err(_) => default_value,
        }
    }

    pub(crate) fn is_webhook_conflict_error(msg: &str) -> bool {
        let lower = msg.to_ascii_lowercase();
        (lower.contains("409") && lower.contains("conflict"))
            || (lower.contains("webhook") && lower.contains("active"))
            || lower.contains("can't use getupdates")
    }

    async fn delete_webhook(&self, drop_pending_updates: bool) -> Result<()> {
        let drop_param = if drop_pending_updates {
            "true"
        } else {
            "false"
        };
        let url = format!(
            "https://api.telegram.org/bot{}/deleteWebhook?drop_pending_updates={}",
            self.token, drop_param
        );
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "deleteWebhook failed (status={}): {}",
                status,
                body_text
            ));
        }
        let parsed: TelegramApiStatusResponse = serde_json::from_str(&body_text).map_err(|e| {
            anyhow::anyhow!("deleteWebhook parse failed: {} (body={})", e, body_text)
        })?;
        if !parsed.ok {
            return Err(anyhow::anyhow!(
                "deleteWebhook returned ok=false: {}",
                parsed
                    .description
                    .unwrap_or_else(|| "unknown error".to_string())
            ));
        }
        Ok(())
    }

    fn run_semaphore() -> Arc<Semaphore> {
        static SEM: std::sync::OnceLock<Arc<Semaphore>> = std::sync::OnceLock::new();
        SEM.get_or_init(|| {
            let max_parallel = std::env::var("STEER_TELEGRAM_MAX_CONCURRENT")
                .ok()
                .and_then(|v| v.trim().parse::<usize>().ok())
                .map(|v| v.clamp(1, 8))
                .unwrap_or(1);
            Arc::new(Semaphore::new(max_parallel))
        })
        .clone()
    }

    pub fn new(
        token: String,
        allowed_user_id: Option<u64>,
        llm: Arc<dyn LLMClient>,
        tx_analyzer: Option<mpsc::Sender<String>>,
    ) -> Self {
        let poll_timeout_sec = std::env::var("STEER_TELEGRAM_POLL_TIMEOUT_SEC")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(|v| v.clamp(3, 60))
            .unwrap_or(12);
        let http_timeout_sec = std::env::var("STEER_TELEGRAM_HTTP_TIMEOUT_SEC")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(|v| v.clamp(10, 120))
            .unwrap_or((poll_timeout_sec + 8).clamp(10, 120));

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(http_timeout_sec))
            .build()
            .unwrap_or_default();

        Self {
            token,
            allowed_user_id,
            client,
            poll_timeout_sec,
            llm,
            tx_analyzer,
        }
    }

    pub fn from_env(
        llm: Arc<dyn LLMClient>,
        tx_analyzer: Option<mpsc::Sender<String>>,
    ) -> Option<Self> {
        crate::load_env_with_fallback();
        let token = std::env::var("TELEGRAM_BOT_TOKEN").ok()?;
        let allowed_user_id = std::env::var("TELEGRAM_USER_ID")
            .ok()
            .and_then(|id| id.parse().ok());

        Some(Self::new(token, allowed_user_id, llm, tx_analyzer))
    }

    async fn get_updates(&self, offset: u64) -> Result<Vec<Update>> {
        let url = format!(
            "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout={}",
            self.token, offset, self.poll_timeout_sec
        );
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "Telegram API Error: {} ({})",
                status,
                body_text
            ));
        }
        let body: GetUpdatesResponse = serde_json::from_str(&body_text).map_err(|e| {
            anyhow::anyhow!(
                "Telegram getUpdates decode failed: {} (body={})",
                e,
                body_text
            )
        })?;
        if !body.ok {
            return Err(anyhow::anyhow!(
                "Telegram API returned ok=false: {}",
                body_text
            ));
        }
        Ok(body.result.unwrap_or_default())
    }

    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<()> {
        telegram_transport::send_message_chunked(
            &self.client,
            &self.token,
            &chat_id.to_string(),
            text,
            None,
            telegram_transport::DEFAULT_MAX_SEND_ATTEMPTS,
        )
        .await
    }

    pub async fn send_message_chunked(&self, chat_id: i64, text: &str) -> Result<()> {
        telegram_transport::send_message_chunked(
            &self.client,
            &self.token,
            &chat_id.to_string(),
            text,
            None,
            telegram_transport::DEFAULT_MAX_SEND_ATTEMPTS,
        )
        .await
    }

    fn is_allowed(&self, user: &Option<User>) -> bool {
        match (self.allowed_user_id, user) {
            (Some(allowed), Some(u)) => u.id == allowed,
            (None, _) => true,
            _ => false,
        }
    }
}
