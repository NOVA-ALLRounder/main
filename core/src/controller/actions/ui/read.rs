use crate::controller::heuristics;
use crate::llm_gateway::LLMClient;
use crate::platform::current_platform;
use crate::session_store::Session;
use crate::visual_driver::VisualDriver;

use super::super::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) async fn handle_read(
        plan: &serde_json::Value,
        llm: Option<&dyn LLMClient>,
        history: &mut Vec<String>,
        last_read_number: &mut Option<String>,
        session: &mut Session,
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
    ) {
        let query = plan["query"].as_str().unwrap_or("Describe the screen");
        let mut read_text = String::new();

        if let Some(app_name) = plan.get("app").and_then(|v| v.as_str()) {
            let _ = heuristics::ensure_app_focus(app_name, 1).await;
        }

        if let Some(brain) = llm {
            match VisualDriver::capture_screen() {
                Ok((b64, _)) => match brain.analyze_screen(query, &b64).await {
                    Ok(resp) => {
                        read_text = resp.trim().to_string();
                    }
                    Err(e) => {
                        *description = format!("read failed (vision): {}", e);
                        *action_status_override = Some("failed");
                    }
                },
                Err(e) => {
                    *description = format!("read failed (capture): {}", e);
                    *action_status_override = Some("failed");
                }
            }
        } else {
            let front_app = current_platform()
                .frontmost_app_name()
                .ok()
                .flatten()
                .unwrap_or_default();
            let mut select_all_first = false;
            if Self::is_text_app(&front_app) {
                let _ = heuristics::focus_text_area(&front_app, false);
                select_all_first = true;
            }
            read_text = Self::capture_front_text_via_platform(select_all_first, 120, 120).await;
        }

        if *action_status_override == Some("failed") {
            return;
        }

        if let Some(num) = Self::extract_first_number(&read_text) {
            *last_read_number = Some(num.clone());
            history.push(format!("READ_NUMBER: {}", num));
        }
        let preview = Self::preview_text(&read_text, 180);
        *description = format!("Read '{}' -> {}", query, preview);
        session.add_message("tool", &format!("read: {}", preview));
        history.push(format!("READ_RESULT: {}", preview));
    }
}
