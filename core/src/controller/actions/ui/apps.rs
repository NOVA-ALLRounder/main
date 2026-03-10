use log::{error, info};
use serde_json::json;

use crate::controller::heuristics;
use crate::platform::{current_platform, AppRole};
use crate::session_store::Session;
use crate::visual_driver::{SmartStep, UiAction};

use super::super::ActionRunner;

impl ActionRunner {
    pub(in crate::controller::actions) async fn handle_switch_app(
        plan: &serde_json::Value,
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
        action_data: &mut Option<serde_json::Value>,
        focus_guard_target: &mut Option<String>,
    ) {
        let app_name = plan["name"]
            .as_str()
            .or_else(|| plan["app"].as_str())
            .unwrap_or("");
        if app_name.is_empty() {
            *description = "switch_app failed: missing app/name".to_string();
            *action_status_override = Some("failed");
            return;
        }

        let mut redundant_skip = false;
        if Self::bool_env_with_default("STEER_BLOCK_REDUNDANT_SWITCH_APP", true) {
            if let Some(front_app) = current_platform().frontmost_app_name().ok().flatten() {
                if front_app.eq_ignore_ascii_case(app_name) {
                    *description = format!("Switched to app: {} (skipped redundant)", app_name);
                    *action_status_override = Some("success");
                    *action_data = Some(json!({
                        "proof": "redundant_switch_app_skip",
                        "front_app": front_app
                    }));
                    redundant_skip = true;
                }
            }
        }
        if redundant_skip {
            return;
        }

        match crate::tool_chaining::CrossAppBridge::switch_to_app(app_name) {
            Ok(_) => {
                let _ = heuristics::ensure_app_focus(app_name, 3).await;
                *focus_guard_target = Some(app_name.to_string());
                *description = format!("Switched to app: {}", app_name);
            }
            Err(e) => {
                *description = format!("switch_app failed: {}", e);
                *action_status_override = Some("failed");
            }
        }
    }

    pub(in crate::controller::actions) fn handle_open_url(
        plan: &serde_json::Value,
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
        consecutive_failures: &mut usize,
    ) {
        let url = plan["url"].as_str().unwrap_or("https://google.com");
        info!("      🌐 Opening URL: '{}'", url);
        if let Err(e) = current_platform().browser_navigate(url, None) {
            error!("      ❌ Open URL failed: {}", e);
            *description = format!("Failed to open URL: {}", e);
            *action_status_override = Some("failed");
            *consecutive_failures += 1;
        } else {
            *description = format!("Opened URL '{}'", url);
            let mut browser_auto = crate::browser_automation::get_browser_automation();
            browser_auto.reset_snapshot();
            *action_status_override = Some("success");
        }
    }

    pub(in crate::controller::actions) async fn handle_open_app(
        plan: &serde_json::Value,
        history: &[String],
        session_steps: &mut Vec<SmartStep>,
        session: &mut Session,
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
        action_data: &mut Option<serde_json::Value>,
        focus_guard_target: &mut Option<String>,
    ) {
        let name = plan["name"]
            .as_str()
            .or_else(|| plan["app"].as_str())
            .unwrap_or(Self::role_app_name(AppRole::FileManager));
        info!("      🚀 Launching/Focusing App: '{}'", name);
        let front_app = current_platform().frontmost_app_name().ok().flatten();
        let browser_name;
        let name = if Self::app_has_role(name, AppRole::Browser) {
            browser_name = heuristics::frontmost_browser(front_app.as_deref())
                .unwrap_or_else(|| Self::role_app_name(AppRole::Browser).to_string());
            browser_name.as_str()
        } else {
            name
        };

        match crate::reality_check::verify_app_exists(name) {
            Ok(canonical_name) => {
                info!(
                    "      🚀 Launching/Focusing App: '{}' (Canonical: '{}')",
                    name, canonical_name
                );
                let mut redundant_skip = false;
                if Self::bool_env_with_default("STEER_BLOCK_REDUNDANT_OPEN_APP", true) {
                    if let Some(front_app_now) =
                        current_platform().frontmost_app_name().ok().flatten()
                    {
                        if front_app_now.eq_ignore_ascii_case(&canonical_name)
                            && Self::history_has_recent_open_for_app(history, &canonical_name)
                        {
                            *description =
                                format!("Opened app: {} (skipped redundant)", canonical_name);
                            *action_status_override = Some("success");
                            *action_data = Some(json!({
                                "proof": "redundant_open_app_skip",
                                "front_app": front_app_now
                            }));
                            redundant_skip = true;
                        }
                    }
                }
                if redundant_skip {
                    return;
                }

                match crate::tool_chaining::CrossAppBridge::switch_to_app(&canonical_name) {
                    Ok(_) => {
                        let _ = heuristics::ensure_app_focus(&canonical_name, 3).await;
                        *focus_guard_target = Some(canonical_name.clone());
                        let step =
                            SmartStep::new(UiAction::Type(canonical_name.clone()), "Open App");
                        session_steps.push(step);
                        *description = format!("Opened app: {}", canonical_name);
                        session.add_message("tool", &format!("open_app: {}", canonical_name));
                    }
                    Err(e) => {
                        error!("      ❌ App open failed: {}", e);
                        *description = format!("Open app failed: {}", e);
                        *action_status_override = Some("failed");
                    }
                }
            }
            Err(e) => {
                error!("      ❌ [Reality] REJECTED: {}", e);
                *description = format!("Failed: {}", e);
                *action_status_override = Some("failed");
            }
        }
    }
}
