use anyhow::Result;
use serde_json::json;

use crate::llm_gateway::LLMClient;
use crate::visual_driver::{SmartStep, VisualDriver};

#[path = "actions/clipboard.rs"]
mod clipboard;
#[path = "actions/content/mod.rs"]
mod content;
#[path = "actions/dispatch.rs"]
mod dispatch;
#[path = "actions/documents.rs"]
mod documents;
#[path = "actions/input.rs"]
mod input;
#[path = "actions/outbound.rs"]
mod outbound;
#[path = "actions/runtime.rs"]
mod runtime;
#[path = "actions/support.rs"]
mod support;
#[path = "actions/ui.rs"]
mod ui;

pub struct ActionRunner;

#[derive(Debug, Clone)]
pub(in crate::controller::actions) struct MailSendResult {
    status: String,
    outgoing_before: Option<i64>,
    outgoing_after: Option<i64>,
    recipient: String,
    subject: String,
    draft_id: String,
    body_len: Option<i64>,
}

pub(in crate::controller::actions) struct NotesWriteResult {
    note_id: String,
    note_name: String,
    body_len: i64,
}

pub(in crate::controller::actions) struct TextEditWriteResult {
    doc_id: String,
    doc_name: String,
    body_len: i64,
}

pub(in crate::controller::actions) struct NotionWriteResult {
    page_id: String,
    page_url: String,
    title: String,
}

#[derive(Debug, Clone)]
pub(in crate::controller::actions) struct AiNewsItem {
    title: String,
    link: String,
    summary: String,
    published_at: String,
}

impl ActionRunner {
    #[allow(clippy::too_many_arguments)]
    pub async fn execute(
        plan: &serde_json::Value,
        driver: &mut VisualDriver,
        llm: Option<&dyn LLMClient>,
        session_steps: &mut Vec<SmartStep>,
        session: &mut crate::session_store::Session,
        history: &mut Vec<String>,
        consecutive_failures: &mut usize,
        last_read_number: &mut Option<String>,
        goal: &str,
    ) -> Result<()> {
        let action_type = plan["action"].as_str().unwrap_or("fail");
        let mut description = format!("Executing {}", action_type);
        let mut action_status_override: Option<&'static str> = None;
        let mut action_data: Option<serde_json::Value> = None;
        let focus_guard_required = matches!(
            action_type,
            "type"
                | "paste"
                | "copy"
                | "select_all"
                | "shortcut"
                | "key"
                | "open_app"
                | "switch_app"
        );
        let mut focus_guard_target =
            if focus_guard_required && !matches!(action_type, "open_app" | "switch_app") {
                Self::preferred_target_app_from_history(action_type, plan, history)
            } else {
                None
            };
        let idempotency_key = if Self::bool_env_with_default("STEER_ACTION_IDEMPOTENCY", true) {
            Self::action_idempotency_key(action_type, plan, goal, history)
        } else {
            None
        };

        if let Some(key) = idempotency_key.as_deref() {
            let already_done = if Self::is_single_fire_idempotency_key(key) {
                Self::idempotency_success_hit(session, key, None)
            } else {
                Self::idempotency_recent_hit(session, key, Self::idempotency_recent_window())
            };
            if already_done {
                let skip_description = format!("Idempotent skip: {}", key);
                history.push(skip_description.clone());
                session.add_step(
                    action_type,
                    &skip_description,
                    "success",
                    Some(json!({
                        "proof": "idempotent_skip",
                        "idempotency_key": key
                    })),
                );
                let _ = crate::session_store::save_session(session);
                *consecutive_failures = 0;
                return Ok(());
            }
        }

        // Pre-action focus: keep action target app frontmost to prevent drift.
        Self::stabilize_focus_for_action(action_type, plan, history, goal).await;

        if action_type == "snapshot" {
            let _ = crate::platform::current_platform().prepare_snapshot_surface();
        }

        Self::dispatch_action(
            action_type,
            plan,
            driver,
            llm,
            session_steps,
            session,
            history,
            last_read_number,
            goal,
            consecutive_failures,
            &mut description,
            &mut action_status_override,
            &mut action_data,
            &mut focus_guard_target,
        )
        .await?;

        if !driver.steps.is_empty() {
            match driver.execute(llm).await {
                Ok(_) => {}
                Err(e) => {
                    description = format!("{} | driver execution failed: {}", description, e);
                    action_status_override = Some("failed");
                }
            }
            driver.steps.clear();
        }

        Self::apply_focus_guard(
            focus_guard_required,
            focus_guard_target.as_deref(),
            action_type,
            &mut action_status_override,
            &mut action_data,
            &mut description,
        )
        .await;

        let status = Self::finalize_action_status(
            action_status_override,
            &description,
            action_data.as_ref(),
        );
        Self::persist_action_outcome(
            session,
            history,
            action_type,
            &description,
            status,
            &mut action_data,
            idempotency_key,
        );
        Self::update_consecutive_failures(consecutive_failures, status, action_type);

        if status != "success"
            && (Self::is_hard_fail_action(action_type) || Self::strict_fail_all_actions())
        {
            return Err(anyhow::anyhow!(
                "Critical {} action failed: {}",
                action_type,
                description
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "actions/tests.rs"]
mod tests;
