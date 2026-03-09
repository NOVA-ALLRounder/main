use serde_json::json;

use crate::controller::actions::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) async fn handle_telegram_send(
        plan: &serde_json::Value,
        goal: &str,
        history: &mut Vec<String>,
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
        action_data: &mut Option<serde_json::Value>,
    ) {
        crate::load_env_with_fallback();
        let token = std::env::var("TELEGRAM_BOT_TOKEN")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        let chat_id = plan["chat_id"]
            .as_str()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .or_else(|| {
                std::env::var("TELEGRAM_CHAT_ID")
                    .ok()
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty())
            });
        let message = Self::preferred_telegram_message(plan, goal, history);
        if token.is_none() {
            *description = "telegram_send failed: TELEGRAM_BOT_TOKEN not set".to_string();
            *action_status_override = Some("failed");
        } else if chat_id.is_none() {
            *description = "telegram_send failed: TELEGRAM_CHAT_ID not set".to_string();
            *action_status_override = Some("failed");
        } else {
            let token = token.unwrap_or_default();
            let chat_id = chat_id.unwrap_or_default();
            if let Err(policy_err) =
                crate::outbound_policy::enforce_telegram_send_policy(&chat_id, &message)
            {
                *description = format!("Telegram send blocked by outbound policy: {}", policy_err);
                *action_status_override = Some("failed");
            } else {
                let ctx = crate::send_policy::SendPolicyContext {
                    session_key: Some(format!("telegram_chat_{}", chat_id)),
                    channel: Some("telegram".to_string()),
                    chat_type: None,
                    target_id: Some(chat_id.clone()),
                };
                if matches!(
                    crate::send_policy::should_send_with_context("telegram", &message, Some(&ctx)),
                    crate::send_policy::SendDecision::Deny
                ) {
                    *description = "Telegram send blocked by send policy".to_string();
                    *action_status_override = Some("failed");
                } else {
                    let client = reqwest::Client::new();
                    match crate::telegram_transport::send_message_chunked(
                        &client, &token, &chat_id, &message, None, 4,
                    )
                    .await
                    {
                        Ok(_) => {
                            let message_id =
                                format!("local-{}", chrono::Utc::now().timestamp_millis());
                            *description = "Telegram send completed".to_string();
                            *action_status_override = Some("success");
                            *action_data = Some(json!({
                                "proof": "telegram_send",
                                "chat_id": chat_id,
                                "message_id": message_id,
                                "message_len": message.chars().count()
                            }));
                            Self::log_evidence(
                                "telegram",
                                "send",
                                &[
                                    ("status", "sent".to_string()),
                                    ("chat_id", chat_id),
                                    ("message_id", message_id),
                                    ("message_len", message.chars().count().to_string()),
                                ],
                            );
                            history.push("telegram: sent".to_string());
                        }
                        Err(e) => {
                            *description = format!("telegram_send failed: {}", e);
                            *action_status_override = Some("failed");
                        }
                    }
                }
            }
        }
    }
}
