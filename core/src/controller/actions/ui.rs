use log::{error, info};
use serde_json::json;

use crate::applescript;
use crate::controller::heuristics;
use crate::llm_gateway::LLMClient;
use crate::session_store::Session;
use crate::visual_driver::{SmartStep, UiAction, VisualDriver};

use super::ActionRunner;

impl ActionRunner {
    pub(super) fn handle_snapshot(
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

    pub(super) async fn handle_click_ref(
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
        let front_app =
            crate::tool_chaining::CrossAppBridge::get_frontmost_app().unwrap_or_default();
        let finder_download_target = ref_id.eq_ignore_ascii_case("LeftSidebarDownloads")
            || Self::goal_mentions_downloads(goal);
        if finder_download_target
            && (front_app.eq_ignore_ascii_case("Finder")
                || plan["app"].as_str() == Some("Finder")
                || plan["name"].as_str() == Some("Finder"))
        {
            match Self::finder_open_downloads() {
                Ok(_) => {
                    *description =
                        "Opened Downloads folder in Finder (deterministic fallback)".to_string();
                    *action_status_override = Some("success");
                }
                Err(e) => {
                    *description = format!(
                        "click_ref '{}' failed (finder downloads fallback): {}",
                        ref_id, e
                    );
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

    pub(super) async fn handle_click_visual(
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
        let front_app =
            crate::tool_chaining::CrossAppBridge::get_frontmost_app().unwrap_or_default();
        let desc_lc = desc.to_lowercase();
        let looks_like_downloads_target = desc_lc.contains("download") || desc.contains("다운로드");
        let looks_like_mail_body_target = desc_lc.contains("message body")
            || desc_lc.contains("mail body")
            || desc_lc.contains("compose body")
            || desc_lc.contains("본문")
            || desc_lc.contains("메시지");

        if front_app.eq_ignore_ascii_case("Mail") && looks_like_mail_body_target {
            *description =
                "Skipped visual click (Mail body target); deterministic body append path will be used"
                    .to_string();
            *action_status_override = Some("success");
            *action_data = Some(json!({
                "proof": "mail_body_focus_skipped",
                "target": desc
            }));
            return;
        }

        if front_app.eq_ignore_ascii_case("Finder")
            && Self::goal_mentions_downloads(goal)
            && looks_like_downloads_target
        {
            match Self::finder_open_downloads() {
                Ok(_) => {
                    *description =
                        "Opened Downloads folder in Finder (deterministic visual fallback)"
                            .to_string();
                    *action_status_override = Some("success");
                }
                Err(e) => {
                    *description = format!("Finder downloads fallback failed: {}", e);
                    *action_status_override = Some("failed");
                    *consecutive_failures += 1;
                }
            }
            return;
        }

        if looks_like_dialog {
            let script = r#"
                tell application "System Events"
                    set frontApp to name of first application process whose frontmost is true
                    tell process frontApp
                        if exists sheet 1 of window 1 then
                            if exists button "Cancel" of sheet 1 of window 1 then
                                click button "Cancel" of sheet 1 of window 1
                            else if exists button "취소" of sheet 1 of window 1 then
                                click button "취소" of sheet 1 of window 1
                            else if exists button "닫기" of sheet 1 of window 1 then
                                click button "닫기" of sheet 1 of window 1
                            end if
                        end if
                    end tell
                end tell
            "#;

            match applescript::run(script) {
                Ok(_) => {
                    *description = "Closed dialog via button click".to_string();
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

    pub(super) async fn handle_read(
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
            let front_app =
                crate::tool_chaining::CrossAppBridge::get_frontmost_app().unwrap_or_default();
            if Self::is_text_app(&front_app) {
                let _ = heuristics::focus_text_area(&front_app, false);
                let _ = std::process::Command::new("osascript")
                    .arg("-e")
                    .arg(r#"tell application "System Events" to keystroke "a" using command down"#)
                    .status();
                std::thread::sleep(std::time::Duration::from_millis(120));
            }
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg(r#"tell application "System Events" to keystroke "c" using command down"#)
                .status();
            std::thread::sleep(std::time::Duration::from_millis(120));
            read_text = crate::tool_chaining::CrossAppBridge::get_clipboard().unwrap_or_default();
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

    pub(super) fn handle_report(
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

    pub(super) async fn handle_switch_app(
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
            if let Ok(front_app) = crate::tool_chaining::CrossAppBridge::get_frontmost_app() {
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

    pub(super) fn handle_scroll(
        plan: &serde_json::Value,
        driver: &mut VisualDriver,
        description: &mut String,
    ) {
        let dir = plan["direction"].as_str().unwrap_or("down");
        let step = SmartStep::new(UiAction::Scroll(dir.to_string()), "Scrolling");
        driver.add_step(step);
        *description = format!("Scrolled {}", dir);
    }

    pub(super) fn handle_open_url(
        plan: &serde_json::Value,
        description: &mut String,
        action_status_override: &mut Option<&'static str>,
        consecutive_failures: &mut usize,
    ) {
        let url = plan["url"].as_str().unwrap_or("https://google.com");
        info!("      🌐 Opening URL: '{}'", url);
        if let Err(e) = applescript::open_url(url) {
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

    pub(super) async fn handle_open_app(
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
            .unwrap_or("Finder");
        info!("      🚀 Launching/Focusing App: '{}'", name);
        let front_app = crate::tool_chaining::CrossAppBridge::get_frontmost_app().ok();
        let name = if name.eq_ignore_ascii_case("Safari") {
            heuristics::frontmost_browser(front_app.as_deref()).unwrap_or("Safari")
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
                    if let Ok(front_app_now) =
                        crate::tool_chaining::CrossAppBridge::get_frontmost_app()
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

    pub(super) fn handle_wait(
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
