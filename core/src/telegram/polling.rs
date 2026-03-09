use super::{Message, TelegramBot};
use crate::controller::planner::Planner;
use log::{error, info};
use std::sync::Arc;
use std::time::Duration;

impl TelegramBot {
    fn task_timeout_sec() -> u64 {
        std::env::var("STEER_TELEGRAM_TASK_TIMEOUT_SEC")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(|v| v.clamp(30, 1800))
            .unwrap_or(240)
    }

    pub(crate) fn telegram_sender_key(user: Option<&super::User>, chat_id: i64) -> String {
        match user {
            Some(user) => format!("telegram_user_{}_chat_{}", user.id, chat_id),
            None => format!("telegram_chat_{}", chat_id),
        }
    }

    fn build_chat_request(
        message: &str,
        chat_type: Option<&str>,
        sender_key: &str,
    ) -> crate::api_server::ChatRequest {
        crate::api_server::ChatRequest {
            message: message.to_string(),
            channel: Some("telegram".to_string()),
            chat_type: chat_type.map(str::to_string),
            sender: Some(sender_key.to_string()),
            mentioned: None,
        }
    }

    pub(crate) fn chat_response_requires_goal_fallback(
        response: &crate::api_server::ChatResponse,
    ) -> bool {
        response.command.is_none()
            && (response
                .response
                .starts_with("🤔 요청을 정확히 해석하지 못했어요.")
                || response
                    .response
                    .starts_with("❓ 무슨 말인지 잘 모르겠어요."))
    }

    pub(crate) fn infer_n8n_digest_request(message: &str) -> Option<String> {
        crate::ai_digest::infer_program_route(message, Some("telegram"))
            .map(|route| route.request_text)
    }

    pub async fn start_polling(self: Arc<Self>) {
        info!("🤖 Telegram Bot started. Waiting for messages...");
        let auto_clear_webhook =
            Self::env_truthy_default("STEER_TELEGRAM_CLEAR_WEBHOOK_ON_POLL", true);
        if auto_clear_webhook {
            match self.delete_webhook(false).await {
                Ok(_) => info!("ℹ️ Telegram webhook cleared for long polling."),
                Err(e) => error!("⚠️ Telegram webhook clear failed: {}", e),
            }
        }
        let mut offset = 0;
        let mut last_webhook_reset: Option<std::time::Instant> = None;

        loop {
            match self.get_updates(offset).await {
                Ok(updates) => {
                    for update in updates {
                        offset = update.update_id + 1;
                        if let Some(msg) = update.message {
                            if let Some(text) = msg.text.clone() {
                                self.handle_incoming_message(msg, text).await;
                            }
                        }
                    }
                }
                Err(e) => {
                    let msg = e.to_string();
                    if auto_clear_webhook && Self::is_webhook_conflict_error(&msg) {
                        let should_retry_reset = last_webhook_reset
                            .map(|ts| ts.elapsed() >= Duration::from_secs(30))
                            .unwrap_or(true);
                        if should_retry_reset {
                            match self.delete_webhook(true).await {
                                Ok(_) => info!(
                                    "ℹ️ Telegram webhook conflict recovered (deleteWebhook drop_pending_updates=true)."
                                ),
                                Err(reset_err) => error!(
                                    "⚠️ Telegram webhook conflict recovery failed: {}",
                                    reset_err
                                ),
                            }
                            last_webhook_reset = Some(std::time::Instant::now());
                        }
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        continue;
                    }
                    if msg.to_ascii_lowercase().contains("timed out") {
                        info!("ℹ️ Telegram poll timeout; retrying quickly.");
                        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
                    } else {
                        error!("⚠️ Telegram Poll Error: {}", msg);
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    }
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    async fn handle_incoming_message(self: &Arc<Self>, msg: Message, text: String) {
        if self.is_allowed(&msg.from) {
            info!("📩 Received command: '{}'", text);
            let bot_clone = self.clone();
            let chat_id = msg.chat.id;
            let chat_type = msg.chat.kind.clone();
            let sender_key = Self::telegram_sender_key(msg.from.as_ref(), chat_id);
            let run_sem = Self::run_semaphore();
            let queued = run_sem.available_permits() == 0;

            let ack = if queued {
                "🤖 Command received. Queued after current task..."
            } else {
                "🤖 Command received. Processing..."
            };
            let _ = self.send_message(chat_id, ack).await.map_err(|e| {
                error!("⚠️ Telegram ack send failed: {}", e);
                e
            });

            tokio::spawn(async move {
                bot_clone
                    .execute_message_task(chat_id, chat_type, sender_key, text)
                    .await;
            });
            return;
        }

        info!("🚫 Ignored message from unauthorized user: {:?}", msg.from);
        let _ = self
            .send_message(
                msg.chat.id,
                "⛔️ 허용된 사용자만 이 봇을 사용할 수 있습니다.",
            )
            .await
            .map_err(|e| {
                error!("⚠️ Telegram unauthorized notice send failed: {}", e);
                e
            });
    }

    async fn execute_message_task(
        self: Arc<Self>,
        chat_id: i64,
        chat_type: Option<String>,
        sender_key: String,
        text_clone: String,
    ) {
        let sem = Self::run_semaphore();
        let _permit = match sem.acquire_owned().await {
            Ok(v) => v,
            Err(_) => {
                let _ = self
                    .send_message_chunked(chat_id, "❌ Task Failed: internal queue unavailable")
                    .await;
                return;
            }
        };

        let goal_text = {
            let cleaned = crate::ai_digest::strip_local_execution_prefix(&text_clone);
            if cleaned.is_empty() {
                text_clone.clone()
            } else {
                cleaned
            }
        };
        let chat_state = crate::api_server::AppState {
            llm_client: Some(self.llm.clone()),
            current_goal: Arc::new(std::sync::Mutex::new(None)),
        };
        let chat_request = Self::build_chat_request(&goal_text, chat_type.as_deref(), &sender_key);
        let chat_response = crate::api_server::process_chat_request(chat_state, chat_request).await;
        if !Self::chat_response_requires_goal_fallback(&chat_response) {
            let _ = self
                .send_message_chunked(chat_id, &chat_response.response)
                .await;
            return;
        }
        if let Some(request_text) = Self::infer_n8n_digest_request(&text_clone) {
            let reply = match crate::ai_digest::trigger_program_webhook_human_summary(
                request_text.trim(),
                None,
            )
            .await
            {
                Ok(summary) => summary,
                Err(e) => {
                    format!("❌ n8n digest trigger failed: {}", e)
                }
            };
            let _ = self.send_message_chunked(chat_id, &reply).await;
            return;
        }

        let planner = Planner::new(self.llm.clone(), self.tx_analyzer.clone());
        let session_key = sender_key.clone();
        let timeout_sec = Self::task_timeout_sec();

        match tokio::time::timeout(
            Duration::from_secs(timeout_sec),
            planner.run_goal_tracked(&goal_text, Some(&session_key)),
        )
        .await
        {
            Ok(Ok(outcome)) => {
                let stage_runs =
                    crate::db::list_task_stage_runs(&outcome.run_id).unwrap_or_default();
                let assertions =
                    crate::db::list_task_stage_assertions(&outcome.run_id).unwrap_or_default();
                let result_context = Self::collect_result_context(&session_key);
                let reply = Self::build_run_report(
                    &outcome,
                    &stage_runs,
                    &assertions,
                    &goal_text,
                    &result_context,
                );
                let _ = self.send_message_chunked(chat_id, &reply).await;
            }
            Ok(Err(e)) => {
                let _ = self
                    .send_message_chunked(chat_id, &format!("❌ Task Failed: {}", e))
                    .await;
            }
            Err(_) => {
                let _ = self
                    .send_message_chunked(
                        chat_id,
                        &format!(
                            "❌ Task Failed: timeout (>{}s). 다음 명령으로 넘어갑니다.",
                            timeout_sec
                        ),
                    )
                    .await;
            }
        }
    }
}
