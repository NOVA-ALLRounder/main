use serde_json::json;

use crate::platform::{current_platform, AppRole};
use crate::session_store::Session;
use crate::visual_driver::{SmartStep, UiAction, VisualDriver};

use super::ActionRunner;

impl ActionRunner {
    pub(super) fn handle_copy(
        driver: &mut VisualDriver,
        history: &[String],
        goal: &str,
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
        action_data: &mut Option<serde_json::Value>,
    ) {
        let front_app = current_platform()
            .frontmost_app_name()
            .ok()
            .flatten()
            .unwrap_or_default();
        if Self::app_has_role(&front_app, AppRole::NotesApp) {
            match Self::notes_read_text(Some(goal)) {
                Ok(text) => {
                    if !text.trim().is_empty() {
                        let _ = crate::tool_chaining::CrossAppBridge::copy_to_clipboard(&text);
                    }
                    *description = "Copied selection (notes scripted)".to_string();
                    *action_status_override = Some("success");
                    *action_data = Some(json!({
                        "proof": "notes_read_text",
                        "text_len": text.chars().count()
                    }));
                }
                Err(e) => {
                    *description = format!("Copy failed (notes scripted): {}", e);
                    *action_status_override = Some("failed");
                }
            }
            return;
        }

        if Self::app_has_role(&front_app, AppRole::TextEditor) {
            match Self::textedit_read_text(Some(goal)) {
                Ok(text) => {
                    if !text.trim().is_empty() {
                        let _ = crate::tool_chaining::CrossAppBridge::copy_to_clipboard(&text);
                    }
                    *description = "Copied selection (textedit scripted)".to_string();
                    *action_status_override = Some("success");
                    *action_data = Some(json!({
                        "proof": "textedit_read_text",
                        "text_len": text.chars().count()
                    }));
                }
                Err(e) => {
                    let err_text = e.to_string();
                    if err_text.to_lowercase().contains("marker not found") {
                        let step = SmartStep::new(
                            UiAction::KeyboardShortcut(
                                "c".to_string(),
                                vec!["command".to_string()],
                            ),
                            "Copy",
                        );
                        driver.add_step(step);
                        *description = "Copied selection (textedit shortcut fallback)".to_string();
                        *action_status_override = Some("success");
                        *action_data = Some(json!({
                            "proof": "textedit_shortcut_copy_fallback",
                            "reason": err_text
                        }));
                    } else {
                        *description = format!("Copy failed (textedit scripted): {}", e);
                        *action_status_override = Some("failed");
                    }
                }
            }
            return;
        }

        let inferred_text_app = Self::last_text_app_from_history(history);
        if Self::goal_mentions_mail(goal)
            && inferred_text_app
                .as_deref()
                .map(|app| {
                    Self::app_has_role(app, AppRole::NotesApp)
                        || Self::app_has_role(app, AppRole::TextEditor)
                })
                .unwrap_or(false)
        {
            if let Some((source, scripted)) = Self::scripted_mail_body_fallback(goal, history) {
                if !scripted.trim().is_empty() {
                    let _ = crate::tool_chaining::CrossAppBridge::copy_to_clipboard(&scripted);
                }
                *description = format!("Copied selection ({} scripted fallback)", source);
                *action_status_override = Some("success");
                *action_data = Some(json!({
                    "proof": "scripted_copy_fallback",
                    "source": source,
                    "text_len": scripted.chars().count()
                }));
                return;
            }
        }

        let step = SmartStep::new(
            UiAction::KeyboardShortcut("c".to_string(), vec!["command".to_string()]),
            "Copy",
        );
        driver.add_step(step);
        *description = "Copied selection".to_string();
    }

    pub(super) fn handle_select_all(driver: &mut VisualDriver, description: &mut String) {
        let step = SmartStep::new(
            UiAction::KeyboardShortcut("a".to_string(), vec!["command".to_string()]),
            "Select All",
        );
        driver.add_step(step);
        *description = "Selected all contents".to_string();
    }

    pub(super) fn handle_read_clipboard(
        history: &mut Vec<String>,
        last_read_number: &mut Option<String>,
        session: &mut Session,
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
    ) {
        match crate::tool_chaining::CrossAppBridge::get_clipboard() {
            Ok(text) => {
                if let Some(num) = Self::extract_first_number(&text) {
                    *last_read_number = Some(num.clone());
                    history.push(format!("READ_NUMBER: {}", num));
                }
                let preview = Self::preview_text(&text, 180);
                *description = format!("Read clipboard -> {}", preview);
                session.add_message("tool", &format!("read_clipboard: {}", preview));
                history.push(format!("READ_RESULT: {}", preview));
            }
            Err(e) => {
                *description = format!("read_clipboard failed: {}", e);
                *action_status_override = Some("failed");
            }
        }
    }
}
