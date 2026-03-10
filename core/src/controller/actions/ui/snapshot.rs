use serde_json::json;

use crate::session_store::Session;

use super::super::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) fn handle_snapshot(
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
        action_data: &mut Option<serde_json::Value>,
        history: &mut Vec<String>,
        session: &mut Session,
    ) {
        let mut browser_auto = crate::browser_automation::get_browser_automation();
        match browser_auto.take_snapshot() {
            Ok(refs) => {
                let summary =
                    crate::browser_automation::BrowserAutomation::summarize_refs(&refs, 20);
                *description = format!("Captured snapshot refs ({} elements)", refs.len());
                history.push(summary.clone());
                session.add_message("tool", &summary);
                *action_data = Some(json!({
                    "proof": "snapshot_refs",
                    "refs": refs.len()
                }));
            }
            Err(e) => {
                *description = format!("snapshot failed: {}", e);
                *action_status_override = Some("failed");
            }
        }
    }

    pub(in crate::controller::actions) fn handle_report(
        plan: &serde_json::Value,
        session: &mut Session,
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
        action_data: &mut Option<serde_json::Value>,
    ) {
        let message = plan["message"].as_str().unwrap_or("Progress update");
        *description = format!("Reported progress: {}", message);
        session.add_message("tool", message);
        *action_data = Some(json!({
            "proof": "report",
            "message_len": message.chars().count()
        }));
        *action_status_override = Some("success");
    }
}
