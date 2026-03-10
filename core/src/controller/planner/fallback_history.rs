use super::Planner;
use crate::platform::{parse_opened_or_switched_app_history_entry, AppRole};

impl Planner {
    fn opened_app_from_history_entry(entry: &str) -> Option<&str> {
        parse_opened_or_switched_app_history_entry(entry)
    }

    pub(super) fn history_has_mail_subject(history: &[String]) -> bool {
        history.iter().any(|h| {
            let lower = h.to_lowercase();
            lower.contains("(mail subject)") || lower.contains("mail subject")
        })
    }

    pub(super) fn history_has_read_result(history: &[String]) -> bool {
        history.iter().any(|h| h.starts_with("READ_RESULT: "))
    }

    pub(super) fn history_has_telegram_send_done(history: &[String]) -> bool {
        history.iter().any(|entry| {
            let lower = entry.to_lowercase();
            lower.contains("telegram send completed")
                || lower.contains("telegram: sent")
                || lower.contains("target=telegram|event=send|status=sent")
        })
    }

    pub(super) fn history_has_type_permission_block(history: &[String]) -> bool {
        history.iter().any(|entry| {
            let lower = entry.to_lowercase();
            if !lower.contains("type failed") && !lower.contains("critical type action failed") {
                return false;
            }
            lower.contains("keystroke")
                || lower.contains("키스트로크")
                || lower.contains("(1002)")
                || lower.contains("native fallback timed out")
        })
    }

    pub(super) fn history_has_n8n_workflow_created(history: &[String]) -> bool {
        history.iter().any(|entry| {
            let lower = entry.to_lowercase();
            lower.contains("n8n workflow created:")
                || lower.contains("target=n8n|event=workflow|status=confirmed")
        })
    }

    pub(super) fn history_has_n8n_execution_done(history: &[String]) -> bool {
        history.iter().any(|entry| {
            let lower = entry.to_lowercase();
            lower.contains("n8n execution completed:")
                || lower.contains("target=n8n|event=execution|status=completed")
                || (lower.contains("execution_id=")
                    && (lower.contains("status=success") || lower.contains("status=completed")))
        })
    }

    pub(super) fn last_opened_app(history: &[String]) -> Option<String> {
        for entry in history.iter().rev() {
            if let Some(app) = Self::opened_app_from_history_entry(entry) {
                return Some(app.to_string());
            }
        }
        None
    }

    pub(super) fn last_opened_app_from_history(history: &[String]) -> Option<String> {
        Self::last_opened_app(history)
    }

    pub(super) fn history_contains_opened_app(history: &[String], app_name: &str) -> bool {
        history.iter().any(|entry| {
            Self::opened_app_from_history_entry(entry)
                .map(|opened| opened.eq_ignore_ascii_case(app_name))
                .unwrap_or(false)
        })
    }

    pub(super) fn history_contains_opened_role_app(history: &[String], role: AppRole) -> bool {
        history.iter().any(|entry| {
            Self::opened_app_from_history_entry(entry)
                .map(|opened| Self::app_is_role(opened, role))
                .unwrap_or(false)
        })
    }

    pub(super) fn last_history_index_opened_role_app(
        history: &[String],
        role: AppRole,
    ) -> Option<usize> {
        history.iter().rposition(|entry| {
            Self::opened_app_from_history_entry(entry)
                .map(|opened| Self::app_is_role(opened, role))
                .unwrap_or(false)
        })
    }

    pub(super) fn history_last_opened_is_app(history: &[String], app_name: &str) -> bool {
        Self::last_opened_app(history)
            .map(|opened| opened.eq_ignore_ascii_case(app_name))
            .unwrap_or(false)
    }

    pub(super) fn has_recent_created_item(history: &[String]) -> bool {
        history
            .iter()
            .rev()
            .take(8)
            .any(|h| h.to_lowercase().contains("created new item"))
    }

    pub(super) fn plan_is_cmd_n_shortcut(plan: &serde_json::Value) -> bool {
        if plan["action"].as_str() != Some("shortcut") {
            return false;
        }
        let key_is_n = plan["key"]
            .as_str()
            .map(|k| k.eq_ignore_ascii_case("n"))
            .unwrap_or(false);
        if !key_is_n {
            return false;
        }
        plan["modifiers"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .any(|m| m.as_str().unwrap_or("").eq_ignore_ascii_case("command"))
            })
            .unwrap_or(false)
    }

    pub(super) fn history_has_recent_new_item_for_app(history: &[String], app_name: &str) -> bool {
        let mut in_target_context = Self::history_last_opened_is_app(history, app_name);

        for entry in history.iter().rev().take(24) {
            if let Some(opened) = Self::opened_app_from_history_entry(entry) {
                if opened.eq_ignore_ascii_case(app_name) {
                    in_target_context = true;
                    continue;
                }
                if in_target_context {
                    break;
                }
                continue;
            }
            let lower = entry.to_lowercase();
            if !in_target_context {
                continue;
            }
            if lower.contains("shortcut 'n'") && lower.contains("created new item") {
                return true;
            }
            if lower.contains("mail send completed") || lower.contains("(mail sent)") {
                return false;
            }
        }
        false
    }
}
