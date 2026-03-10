use serde_json::json;

use crate::controller::heuristics;
use crate::platform::{current_platform, AppRole};
use crate::visual_driver::{SmartStep, UiAction, VisualDriver};

use super::super::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) async fn handle_click_ref(
        plan: &serde_json::Value,
        goal: &str,
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
    ) {
        let ref_id = plan["ref"].as_str().unwrap_or("");
        if ref_id.is_empty() {
            *description = "click_ref failed: missing ref".to_string();
            *action_status_override = Some("failed");
            return;
        }

        if let Some(app_name) = plan
            .get("app")
            .and_then(|v| v.as_str())
            .or_else(|| plan.get("name").and_then(|v| v.as_str()))
        {
            let _ = heuristics::ensure_app_focus(app_name, 1).await;
        }
        let front_app = current_platform()
            .frontmost_app_name()
            .ok()
            .flatten()
            .unwrap_or_default();
        let file_manager = Self::role_app_name(AppRole::FileManager);
        let file_manager_target = ref_id.eq_ignore_ascii_case("LeftSidebarDownloads")
            || Self::goal_mentions_downloads(goal);
        if file_manager_target
            && (Self::app_has_role(&front_app, AppRole::FileManager)
                || plan["app"]
                    .as_str()
                    .map(|app| Self::app_has_role(app, AppRole::FileManager))
                    .unwrap_or(false)
                || plan["name"]
                    .as_str()
                    .map(|app| Self::app_has_role(app, AppRole::FileManager))
                    .unwrap_or(false))
        {
            match Self::finder_open_downloads() {
                Ok(_) => {
                    *description = format!(
                        "Opened Downloads folder in {} (deterministic fallback)",
                        file_manager
                    );
                    *action_status_override = Some("success");
                }
                Err(e) => {
                    *description =
                        format!("click_ref '{}' failed (downloads fallback): {}", ref_id, e);
                    *action_status_override = Some("failed");
                }
            }
            return;
        }

        let mut browser_auto = crate::browser_automation::get_browser_automation();
        let click_res = browser_auto.click_by_ref(ref_id, false).or_else(|_| {
            browser_auto.take_snapshot()?;
            browser_auto.click_by_ref(ref_id, false)
        });
        match click_res {
            Ok(_) => {
                *description = format!("Clicked ref '{}'", ref_id);
            }
            Err(e) => {
                *description = format!("click_ref '{}' failed: {}", ref_id, e);
                *action_status_override = Some("failed");
            }
        }
    }

    pub(in crate::controller::actions) async fn handle_click_visual(
        plan: &serde_json::Value,
        goal: &str,
        driver: &mut VisualDriver,
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
        action_data: &mut Option<serde_json::Value>,
        consecutive_failures: &mut usize,
    ) {
        let desc = plan["description"].as_str().unwrap_or("element");
        let looks_like_dialog = heuristics::looks_like_dialog(desc);
        let front_app = current_platform()
            .frontmost_app_name()
            .ok()
            .flatten()
            .unwrap_or_default();
        let desc_lc = desc.to_lowercase();
        let looks_like_downloads_target = desc_lc.contains("download") || desc.contains("다운로드");
        let looks_like_mail_body_target = desc_lc.contains("message body")
            || desc_lc.contains("mail body")
            || desc_lc.contains("compose body")
            || desc_lc.contains("본문")
            || desc_lc.contains("메시지");

        if Self::app_has_role(&front_app, AppRole::MailClient) && looks_like_mail_body_target {
            *description =
                "Skipped visual click (mail body target); deterministic body append path will be used"
                    .to_string();
            *action_status_override = Some("success");
            *action_data = Some(json!({
                "proof": "mail_body_focus_skipped",
                "target": desc
            }));
            return;
        }

        if Self::app_has_role(&front_app, AppRole::FileManager)
            && Self::goal_mentions_downloads(goal)
            && looks_like_downloads_target
        {
            match Self::finder_open_downloads() {
                Ok(_) => {
                    *description = format!(
                        "Opened Downloads folder in {} (deterministic visual fallback)",
                        Self::role_app_name(AppRole::FileManager)
                    );
                    *action_status_override = Some("success");
                }
                Err(e) => {
                    *description = format!(
                        "{} downloads fallback failed: {}",
                        Self::role_app_name(AppRole::FileManager),
                        e
                    );
                    *action_status_override = Some("failed");
                    *consecutive_failures += 1;
                }
            }
            return;
        }

        if looks_like_dialog {
            match current_platform().keyboard_shortcut("escape", &[]) {
                Ok(_) => {
                    *description = "Closed dialog via Escape shortcut".to_string();
                    *action_status_override = Some("success");
                }
                Err(e) => {
                    *description = format!("Dialog close failed: {}", e);
                    *action_status_override = Some("failed");
                    *consecutive_failures += 1;
                }
            }
            return;
        }

        let step = SmartStep::new(UiAction::ClickVisual(desc.to_string()), desc);
        driver.add_step(step);
        *description = format!("Clicked '{}'", desc);
    }

    pub(in crate::controller::actions) fn handle_scroll(
        plan: &serde_json::Value,
        driver: &mut VisualDriver,
        description: &mut String,
    ) {
        let dir = plan["direction"].as_str().unwrap_or("down");
        let step = SmartStep::new(UiAction::Scroll(dir.to_string()), "Scrolling");
        driver.add_step(step);
        *description = format!("Scrolled {}", dir);
    }

    pub(in crate::controller::actions) fn handle_wait(
        plan: &serde_json::Value,
        driver: &mut VisualDriver,
        description: &mut String,
    ) {
        let secs = plan["seconds"].as_u64().unwrap_or(2);
        let step = SmartStep::new(UiAction::Wait(secs), "Waiting");
        driver.add_step(step);
        *description = format!("Waited {}s", secs);
    }
}
