use crate::controller::heuristics;

use crate::controller::actions::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) fn session_has_single_fire_new_item(
        session: &crate::session_store::Session,
        key: &str,
    ) -> bool {
        for step in session.steps.iter().rev().take(96) {
            let same_key = step
                .data
                .as_ref()
                .and_then(|v| v.get("idempotency_key"))
                .and_then(|v| v.as_str())
                .map(|v| v == key)
                .unwrap_or(false);
            if !same_key {
                continue;
            }

            let proof = step
                .data
                .as_ref()
                .and_then(|v| v.get("proof"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let desc_lower = step.description.to_lowercase();

            if matches!(proof, "mail_draft_ready" | "redundant_new_item_skip")
                || desc_lower.contains("created new item")
                || step.status == "success"
            {
                return true;
            }
        }
        false
    }

    pub(in crate::controller::actions) fn idempotency_recent_hit(
        session: &crate::session_store::Session,
        key: &str,
        recent_window: usize,
    ) -> bool {
        Self::idempotency_success_hit(session, key, Some(recent_window))
    }

    pub(in crate::controller::actions) fn idempotency_success_hit(
        session: &crate::session_store::Session,
        key: &str,
        recent_window: Option<usize>,
    ) -> bool {
        for (inspected, step) in session.steps.iter().rev().enumerate() {
            if let Some(limit) = recent_window {
                if inspected >= limit {
                    break;
                }
            }
            if step.status != "success" {
                continue;
            }
            let hit = step
                .data
                .as_ref()
                .and_then(|v| v.get("idempotency_key"))
                .and_then(|v| v.as_str())
                .map(|v| v == key)
                .unwrap_or(false);
            if hit {
                return true;
            }
        }
        false
    }

    pub(in crate::controller::actions) fn idempotency_recent_window() -> usize {
        std::env::var("STEER_ACTION_IDEMPOTENCY_WINDOW")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .map(|v| v.clamp(4, 256))
            .unwrap_or(16)
    }

    pub(in crate::controller::actions) fn max_cmd_n_attempts_per_app() -> usize {
        std::env::var("STEER_CMD_N_MAX_ATTEMPTS_PER_APP")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .map(|v| v.clamp(1, 6))
            .unwrap_or(2)
    }

    pub(in crate::controller::actions) fn session_cmd_n_stats(
        session: &crate::session_store::Session,
        app_name: &str,
    ) -> (usize, usize) {
        let app_lower = app_name.trim().to_lowercase();
        if app_lower.is_empty() {
            return (0, 0);
        }
        let key = format!("shortcut:{}:command+n:new_item", app_lower);
        let mut attempts = 0usize;
        let mut successes = 0usize;

        for step in &session.steps {
            let matches_key = step
                .data
                .as_ref()
                .and_then(|v| v.get("idempotency_key"))
                .and_then(|v| v.as_str())
                .map(|v| v == key)
                .unwrap_or(false);
            if !matches_key {
                continue;
            }
            attempts += 1;
            if step.status == "success" {
                successes += 1;
            }
        }

        (attempts, successes)
    }

    pub(in crate::controller::actions) fn is_single_fire_idempotency_key(key: &str) -> bool {
        key.contains(":command+n:new_item")
    }

    pub(in crate::controller::actions) fn action_idempotency_key(
        action_type: &str,
        plan: &serde_json::Value,
        goal: &str,
        history: &[String],
    ) -> Option<String> {
        match action_type {
            "open_app" | "switch_app" => {
                let app = plan["name"]
                    .as_str()
                    .or_else(|| plan["app"].as_str())
                    .unwrap_or("")
                    .trim()
                    .to_lowercase();
                if app.is_empty() {
                    None
                } else {
                    Some(format!("{}:{}", action_type, app))
                }
            }
            "open_url" => {
                let url = plan["url"].as_str().unwrap_or("").trim().to_lowercase();
                if url.is_empty() {
                    None
                } else {
                    Some(format!("open_url:{}", url))
                }
            }
            "mail_send" => {
                let recipient = Self::preferred_mail_recipient(Some(goal)).unwrap_or_default();
                let subject = Self::preferred_mail_subject(Some(goal)).unwrap_or_default();
                let scope = Self::preferred_run_scope_marker(Some(goal)).unwrap_or_default();
                let draft_id = plan["draft_id"].as_str().unwrap_or("").trim().to_string();
                let suffix = if !draft_id.is_empty() {
                    format!("draft={}", draft_id)
                } else if !scope.is_empty() {
                    format!("scope={}", scope.to_lowercase())
                } else if !recipient.is_empty() || !subject.is_empty() {
                    format!(
                        "recipient={}::subject={}",
                        recipient.to_lowercase(),
                        subject.to_lowercase()
                    )
                } else {
                    "generic".to_string()
                };
                Some(format!("mail_send:{}", suffix))
            }
            "telegram_send" => {
                let scope = Self::preferred_run_scope_marker(Some(goal)).unwrap_or_default();
                let chat_id = plan["chat_id"]
                    .as_str()
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty())
                    .or_else(|| {
                        std::env::var("TELEGRAM_CHAT_ID")
                            .ok()
                            .map(|v| v.trim().to_string())
                            .filter(|v| !v.is_empty())
                    })
                    .unwrap_or_default();
                let msg = Self::preferred_telegram_message(plan, goal, history);
                let msg_len = msg.chars().count();
                if !scope.is_empty() {
                    Some(format!(
                        "telegram_send:{}:scope={}",
                        chat_id,
                        scope.to_lowercase()
                    ))
                } else {
                    Some(format!("telegram_send:{}:len={}", chat_id, msg_len))
                }
            }
            "notion_write" => {
                let scope = Self::preferred_run_scope_marker(Some(goal)).unwrap_or_default();
                let title = plan["title"]
                    .as_str()
                    .map(|v| v.trim().to_lowercase())
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| "steer_note".to_string());
                if !scope.is_empty() {
                    Some(format!(
                        "notion_write:{}:scope={}",
                        title,
                        scope.to_lowercase()
                    ))
                } else {
                    let content_len = plan["content"]
                        .as_str()
                        .map(|v| v.chars().count())
                        .unwrap_or_default();
                    Some(format!("notion_write:{}:len={}", title, content_len))
                }
            }
            "n8n_create_workflow" => {
                let marker = plan["marker"]
                    .as_str()
                    .map(|v| v.trim().to_lowercase())
                    .filter(|v| !v.is_empty())
                    .or_else(|| {
                        Self::preferred_run_scope_marker(Some(goal))
                            .map(|v| v.trim().to_lowercase())
                            .filter(|v| !v.is_empty())
                    })
                    .unwrap_or_else(|| "run_scope_test_03".to_string());
                Some(format!("n8n_create_workflow:{}", marker))
            }
            "n8n_execute_workflow" => {
                let workflow_id = plan["workflow_id"]
                    .as_str()
                    .map(|v| v.trim().to_lowercase())
                    .filter(|v| !v.is_empty())
                    .or_else(|| {
                        Self::extract_latest_n8n_workflow_id(history)
                            .map(|v| v.trim().to_lowercase())
                            .filter(|v| !v.is_empty())
                    })
                    .unwrap_or_else(|| "latest".to_string());
                Some(format!("n8n_execute_workflow:{}", workflow_id))
            }
            "shortcut" | "key" => {
                let raw_key = plan["key"].as_str().unwrap_or("").to_string();
                let raw_modifiers: Vec<String> = plan["modifiers"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let (key, modifiers) = Self::normalize_shortcut_parts(&raw_key, &raw_modifiers);
                let app_context = plan["app"]
                    .as_str()
                    .map(|v| v.trim().to_lowercase())
                    .filter(|v| !v.is_empty())
                    .or_else(|| {
                        Self::preferred_target_app_from_history("shortcut", plan, history)
                            .map(|v| v.trim().to_lowercase())
                            .filter(|v| !v.is_empty())
                    })
                    .or_else(|| heuristics::goal_primary_app(goal).map(|v| v.trim().to_lowercase()))
                    .or_else(|| {
                        Self::last_opened_app_from_history(history).map(|v| v.trim().to_lowercase())
                    })
                    .unwrap_or_else(|| "unknown".to_string());
                let has_command = modifiers.iter().any(|m| m == "command");
                let has_shift = modifiers.iter().any(|m| m == "shift");
                if key == "n" && has_command {
                    return Some(format!("shortcut:{}:command+n:new_item", app_context));
                }
                if key == "d" && has_command && has_shift {
                    return Some(format!(
                        "shortcut:{}:command+shift+d:mail_send",
                        app_context
                    ));
                }
                None
            }
            _ => None,
        }
    }
}
