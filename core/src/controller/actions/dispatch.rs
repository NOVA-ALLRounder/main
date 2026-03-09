use anyhow::Result;
use serde_json::Value;

use crate::llm_gateway::LLMClient;
use crate::session_store::Session;
use crate::visual_driver::{SmartStep, VisualDriver};

use super::ActionRunner;

impl ActionRunner {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::controller::actions) async fn dispatch_action(
        action_type: &str,
        plan: &Value,
        driver: &mut VisualDriver,
        llm: Option<&dyn LLMClient>,
        session_steps: &mut Vec<SmartStep>,
        session: &mut Session,
        history: &mut Vec<String>,
        last_read_number: &mut Option<String>,
        goal: &str,
        consecutive_failures: &mut usize,
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
        action_data: &mut Option<Value>,
        focus_guard_target: &mut Option<String>,
    ) -> Result<()> {
        match action_type {
            "snapshot" => {
                Self::handle_snapshot(
                    description,
                    action_status_override,
                    action_data,
                    history,
                    session,
                );
            }
            "click_ref" => {
                Self::handle_click_ref(plan, goal, description, action_status_override).await;
            }
            "click_visual" => {
                Self::handle_click_visual(
                    plan,
                    goal,
                    driver,
                    description,
                    action_status_override,
                    action_data,
                    consecutive_failures,
                )
                .await;
            }
            "read" => {
                Self::handle_read(
                    plan,
                    llm,
                    history,
                    last_read_number,
                    session,
                    description,
                    action_status_override,
                )
                .await;
            }
            "report" => {
                Self::handle_report(
                    plan,
                    session,
                    description,
                    action_status_override,
                    action_data,
                );
            }
            "type" => {
                Self::handle_type(
                    plan,
                    goal,
                    driver,
                    history,
                    last_read_number,
                    description,
                    action_status_override,
                    action_data,
                )
                .await;
            }
            "key" => {
                Self::handle_key(
                    plan,
                    goal,
                    driver,
                    session,
                    history,
                    description,
                    action_status_override,
                    action_data,
                )
                .await;
            }
            "shortcut" => {
                Self::handle_shortcut(
                    plan,
                    goal,
                    driver,
                    session,
                    history,
                    description,
                    action_status_override,
                    action_data,
                )
                .await;
            }
            "mail_send" => {
                Self::handle_mail_send(
                    goal,
                    history,
                    description,
                    action_status_override,
                    action_data,
                )
                .await;
            }
            "telegram_send" => {
                Self::handle_telegram_send(
                    plan,
                    goal,
                    history,
                    description,
                    action_status_override,
                    action_data,
                )
                .await;
            }
            "notion_write" => {
                Self::handle_notion_write(
                    plan,
                    goal,
                    description,
                    action_status_override,
                    action_data,
                )
                .await;
            }
            "n8n_create_workflow" => {
                Self::handle_n8n_create_workflow(
                    plan,
                    goal,
                    description,
                    action_status_override,
                    action_data,
                )
                .await;
            }
            "n8n_execute_workflow" => {
                Self::handle_n8n_execute_workflow(
                    plan,
                    history,
                    description,
                    action_status_override,
                    action_data,
                )
                .await;
            }
            "paste" => {
                Self::handle_paste(
                    plan,
                    goal,
                    driver,
                    history,
                    description,
                    action_status_override,
                    action_data,
                )
                .await;
            }
            "copy" => {
                Self::handle_copy(
                    driver,
                    history,
                    goal,
                    description,
                    action_status_override,
                    action_data,
                );
            }
            "select_all" => {
                Self::handle_select_all(driver, description);
            }
            "read_clipboard" => {
                Self::handle_read_clipboard(
                    history,
                    last_read_number,
                    session,
                    description,
                    action_status_override,
                );
            }
            "switch_app" => {
                Self::handle_switch_app(
                    plan,
                    description,
                    action_status_override,
                    action_data,
                    focus_guard_target,
                )
                .await;
            }
            "scroll" => {
                Self::handle_scroll(plan, driver, description);
            }
            "open_url" => {
                Self::handle_open_url(
                    plan,
                    description,
                    action_status_override,
                    consecutive_failures,
                );
            }
            "open_app" => {
                Self::handle_open_app(
                    plan,
                    history,
                    session_steps,
                    session,
                    description,
                    action_status_override,
                    action_data,
                    focus_guard_target,
                )
                .await;
            }
            "wait" => {
                Self::handle_wait(plan, driver, description);
            }
            "fail" => {
                let reason = plan["reason"].as_str().unwrap_or("Unknown");
                return Err(anyhow::anyhow!("Agent failed: {}", reason));
            }
            _ => {
                *description = format!("Action '{}' not implemented in ActionRunner", action_type);
                *action_status_override = Some("failed");
            }
        }

        Ok(())
    }
}
