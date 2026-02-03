use serde::{Deserialize, Serialize};
use serde_json::json;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use crate::llm_gateway::LLMClient;
use crate::dynamic_controller::DynamicController;
use log::{info, error};

#[derive(Serialize, Deserialize, Debug)]
struct Update {
    update_id: u64,
    message: Option<Message>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    message_id: u64,
    from: Option<User>,
    chat: Chat,
    text: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct User {
    id: u64,
    username: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Chat {
    id: i64,
}

#[derive(Serialize, Deserialize, Debug)]
struct GetUpdatesResponse {
    ok: bool,
    result: Vec<Update>,
}

pub struct TelegramBot {
    token: String,
    allowed_user_id: Option<u64>,
    client: reqwest::Client,
    llm: Arc<LLMClient>,
    tx_analyzer: Option<mpsc::Sender<String>>,
}

impl TelegramBot {
    pub fn new(token: String, allowed_user_id: Option<u64>, llm: Arc<LLMClient>, tx_analyzer: Option<mpsc::Sender<String>>) -> Self {
        Self {
            token,
            allowed_user_id,
            client: reqwest::Client::new(),
            llm,
            tx_analyzer,
        }
    }

    pub fn from_env(llm: Arc<LLMClient>, tx_analyzer: Option<mpsc::Sender<String>>) -> Option<Self> {
        let token = std::env::var("TELEGRAM_BOT_TOKEN").ok()?;
        let allowed_user_id = std::env::var("TELEGRAM_USER_ID")
            .ok()
            .and_then(|id| id.parse().ok());
        
        Some(Self::new(token, allowed_user_id, llm, tx_analyzer))
    }

    pub async fn start_polling(self: Arc<Self>) {
        info!("🤖 Telegram Bot started. Waiting for messages...");
        let mut offset = 0;

        loop {
            match self.get_updates(offset).await {
                Ok(updates) => {
                    for update in updates {
                        offset = update.update_id + 1;
                        if let Some(msg) = update.message {
                            if let Some(text) = msg.text {
                                if self.is_allowed(&msg.from) {
                                    info!("📩 Received command: '{}'", text);
                                    let bot_clone = self.clone();
                                    let chat_id = msg.chat.id;
                                    let text_clone = text.clone();
                                    
                                    // Ack reception
                                    let _ = self.send_message(chat_id, "🤖 Command received. Processing...").await;

                                    // Spawn agent task
                                    tokio::spawn(async move {
                                        let controller = DynamicController::new((*bot_clone.llm).clone(), bot_clone.tx_analyzer.clone());
                                        
                                        // TODO: Pass Telegram context to Surf so it knows where to reply?
                                        // For now, we just run the task. 
                                        // But Agent needs to REPLY. 
                                        // We can inject "GOAL: <text> (Reply to Telegram Chat ID: <id>)"
                                        // or better, adding a "telegram_reply" action that uses this ID.
                                        // For MVP, if surf succeeds, we send "Done".
                                        
                                        match controller.surf(&text_clone).await {
                                            Ok(_) => {
                                                let _ = bot_clone.send_message(chat_id, "✅ Task Completed.").await;
                                            },
                                            Err(e) => {
                                                let _ = bot_clone.send_message(chat_id, &format!("❌ Task Failed: {}", e)).await;
                                            }
                                        }
                                    });
                                } else {
                                    info!("🚫 Ignored message from unauthorized user: {:?}", msg.from);
                                }
                            }
                        }
                    }
                },
                Err(e) => {
                    error!("⚠️ Telegram Poll Error: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    async fn get_updates(&self, offset: u64) -> Result<Vec<Update>> {
        let url = format!("https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=30", self.token, offset);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
             return Err(anyhow::anyhow!("Telegram API Error: {}", resp.status()));
        }
        let body: GetUpdatesResponse = resp.json().await?;
        if !body.ok {
             return Err(anyhow::anyhow!("Telegram API returned ok=false"));
        }
        Ok(body.result)
    }

    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<()> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);
        let body = json!({
            "chat_id": chat_id,
            "text": text
        });
        let _ = self.client.post(&url).json(&body).send().await?;
        Ok(())
    }

    fn is_allowed(&self, user: &Option<User>) -> bool {
        match (self.allowed_user_id, user) {
            (Some(allowed), Some(u)) => u.id == allowed,
            (None, _) => true, // IF id not set, allow all (Dangerous! Warn in logs)
            _ => false,
        }
    }
}
