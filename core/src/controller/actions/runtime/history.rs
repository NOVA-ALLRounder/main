use log::info;

use super::super::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) fn bool_env_with_default(key: &str, default: bool) -> bool {
        match std::env::var(key) {
            Ok(v) => matches!(v.trim(), "1" | "true" | "TRUE" | "yes" | "YES"),
            Err(_) => default,
        }
    }

    pub(in crate::controller::actions) fn is_test_mode_enabled() -> bool {
        match std::env::var("STEER_TEST_MODE") {
            Ok(v) => matches!(v.trim(), "1" | "true" | "TRUE" | "yes" | "YES"),
            Err(_) => false,
        }
    }

    pub(in crate::controller::actions) fn mail_fresh_recovery_used(history: &[String]) -> bool {
        history
            .iter()
            .any(|entry| entry.trim() == "MAIL_FRESH_RECOVERY_USED")
    }

    pub(in crate::controller::actions) fn mark_mail_fresh_recovery_used(history: &mut Vec<String>) {
        if !Self::mail_fresh_recovery_used(history) {
            history.push("MAIL_FRESH_RECOVERY_USED".to_string());
        }
    }

    pub(in crate::controller::actions) fn mail_current_draft_id(
        history: &[String],
    ) -> Option<String> {
        for entry in history.iter().rev() {
            if let Some(rest) = entry.strip_prefix("MAIL_DRAFT_ID:") {
                let id = rest.trim();
                if !id.is_empty() {
                    return Some(id.to_string());
                }
            }
        }
        None
    }

    pub(in crate::controller::actions) fn has_tracked_mail_draft(history: &[String]) -> bool {
        Self::mail_current_draft_id(history).is_some()
    }

    pub(in crate::controller::actions) fn remember_mail_draft_id(
        history: &mut Vec<String>,
        draft_id: &str,
    ) {
        let trimmed = draft_id.trim();
        if trimmed.is_empty() {
            return;
        }
        info!("      📧 [MailDraft] tracking draft_id={}", trimmed);
        history.push(format!("MAIL_DRAFT_ID:{}", trimmed));
    }

    pub(in crate::controller::actions) fn last_opened_app_from_history(
        history: &[String],
    ) -> Option<String> {
        for entry in history.iter().rev() {
            if let Some(rest) = entry.strip_prefix("Opened app: ") {
                let app = rest.trim();
                if !app.is_empty() {
                    return Some(app.to_string());
                }
            }
        }
        None
    }

    pub(in crate::controller::actions) fn history_has_recent_open_for_app(
        history: &[String],
        app_name: &str,
    ) -> bool {
        let target = app_name.trim().to_lowercase();
        if target.is_empty() {
            return false;
        }
        for entry in history.iter().rev().take(12) {
            if let Some(rest) = entry.strip_prefix("Opened app: ") {
                let opened = rest.trim().to_lowercase();
                return opened == target;
            }
        }
        false
    }

    pub(in crate::controller::actions) fn history_has_recent_created_new_item_for_app(
        history: &[String],
        app_name: &str,
    ) -> bool {
        let target = app_name.to_lowercase();
        let mut in_target_context = Self::last_opened_app_from_history(history)
            .map(|app| app.eq_ignore_ascii_case(app_name))
            .unwrap_or(false);

        for entry in history.iter().rev().take(24) {
            let lower = entry.to_lowercase();
            if let Some(rest) = lower.strip_prefix("opened app: ") {
                let opened = rest.trim();
                if opened.eq_ignore_ascii_case(&target) {
                    in_target_context = true;
                    continue;
                }
                if in_target_context {
                    break;
                }
                continue;
            }
            if !in_target_context {
                continue;
            }
            if lower.contains("shortcut 'n'") && lower.contains("created new item") {
                return true;
            }
        }
        false
    }

    pub(in crate::controller::actions) fn should_skip_redundant_cmd_n(
        history: &[String],
        app_name: &str,
    ) -> bool {
        if !Self::bool_env_with_default("STEER_BLOCK_REDUNDANT_CMD_N", true) {
            return false;
        }
        Self::history_has_recent_created_new_item_for_app(history, app_name)
    }

    pub(in crate::controller::actions) fn cmd_n_window_flood_limit() -> usize {
        std::env::var("STEER_CMD_N_WINDOW_FLOOD_LIMIT")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .map(|v| v.clamp(1, 30))
            .unwrap_or(3)
    }

    pub(in crate::controller::actions) fn cmd_n_window_flood_limit_for_app(
        app_name: &str,
    ) -> usize {
        let normalized = app_name
            .trim()
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_uppercase()
                } else {
                    '_'
                }
            })
            .collect::<String>();
        let env_key = format!("STEER_CMD_N_WINDOW_FLOOD_LIMIT_{}", normalized);
        if let Ok(raw) = std::env::var(&env_key) {
            if let Ok(parsed) = raw.trim().parse::<usize>() {
                return parsed.clamp(1, 30);
            }
        }
        if app_name.trim().eq_ignore_ascii_case("Mail") {
            return 1;
        }
        Self::cmd_n_window_flood_limit()
    }

    pub(in crate::controller::actions) fn cmd_n_window_flood_history_window() -> usize {
        std::env::var("STEER_CMD_N_WINDOW_FLOOD_WINDOW")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .map(|v| v.clamp(12, 512))
            .unwrap_or(96)
    }

    pub(in crate::controller::actions) fn history_recent_cmd_n_created_count(
        history: &[String],
        app_name: &str,
        recent_window: usize,
    ) -> usize {
        let target = app_name.trim().to_lowercase();
        if target.is_empty() {
            return 0;
        }

        let start_idx = history.len().saturating_sub(recent_window);
        let mut count = 0usize;
        let mut current_app = String::new();

        for (idx, entry) in history.iter().enumerate() {
            let lower = entry.to_lowercase();
            if let Some(rest) = lower.strip_prefix("opened app: ") {
                current_app = rest.trim().to_string();
                continue;
            }
            if idx < start_idx {
                continue;
            }
            if current_app != target {
                continue;
            }
            if lower.contains("shortcut 'n'") && lower.contains("created new item") {
                count += 1;
            }
        }

        count
    }
}
