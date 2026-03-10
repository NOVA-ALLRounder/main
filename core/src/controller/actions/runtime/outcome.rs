use crate::platform::current_platform;
use serde_json::json;

use super::super::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) fn put_action_data_field(
        action_data: &mut Option<serde_json::Value>,
        key: &str,
        value: serde_json::Value,
    ) {
        if action_data.is_none() {
            *action_data = Some(json!({}));
        }
        if let Some(obj) = action_data.as_mut().and_then(|v| v.as_object_mut()) {
            obj.insert(key.to_string(), value);
        }
    }

    pub(in crate::controller::actions) fn finalize_action_status(
        mut action_status_override: Option<&'static str>,
        description: &str,
        action_data: Option<&serde_json::Value>,
    ) -> &'static str {
        if action_status_override.is_none() {
            let lower_desc = description.to_lowercase();
            if lower_desc.contains(" failed")
                || lower_desc.contains("blocked")
                || lower_desc.contains("error")
            {
                action_status_override = Some("failed");
            }
        }
        if action_status_override.is_none() {
            if let Some(data) = action_data {
                if let Some(send_status) = data.get("send_status").and_then(|v| v.as_str()) {
                    if send_status != "sent_confirmed" {
                        action_status_override = Some("failed");
                    }
                }
            }
        }
        action_status_override.unwrap_or("success")
    }

    pub(in crate::controller::actions) fn persist_action_outcome(
        session: &mut crate::session_store::Session,
        history: &mut Vec<String>,
        action_type: &str,
        description: &str,
        status: &str,
        action_data: &mut Option<serde_json::Value>,
        idempotency_key: Option<String>,
    ) {
        if let Some(key) = idempotency_key {
            Self::put_action_data_field(action_data, "idempotency_key", json!(key));
        }
        history.push(description.to_string());
        if action_data.is_none() {
            if let Some(front_after) = current_platform().frontmost_app_name().ok().flatten() {
                *action_data = Some(json!({
                    "front_app_after": front_after
                }));
            }
        }
        session.add_step(action_type, description, status, action_data.take());
        let _ = crate::session_store::save_session(session);
    }

    pub(in crate::controller::actions) fn update_consecutive_failures(
        consecutive_failures: &mut usize,
        status: &str,
        action_type: &str,
    ) {
        if status != "success" && action_type != "fail" {
            *consecutive_failures += 1;
        } else {
            *consecutive_failures = 0;
        }
    }

    pub(in crate::controller::actions) fn strict_fail_all_actions() -> bool {
        std::env::var("STEER_STRICT_ACTION_ERRORS")
            .ok()
            .map(|v| {
                matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false)
    }

    pub(in crate::controller::actions) fn is_hard_fail_action(action_type: &str) -> bool {
        matches!(
            action_type,
            "click_visual"
                | "click_ref"
                | "open_app"
                | "switch_app"
                | "type"
                | "paste"
                | "mail_send"
                | "telegram_send"
                | "notion_write"
                | "n8n_create_workflow"
                | "n8n_execute_workflow"
        )
    }
}
