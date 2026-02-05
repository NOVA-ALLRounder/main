use anyhow::{Result, Context};
use log::{info, error};
use serde_json::Value; // Added serde_json import
use crate::visual_driver::{VisualDriver, SmartStep, UiAction};
use crate::controller::heuristics;
use crate::db;
use crate::applescript;

pub struct ActionRunner;

impl ActionRunner {
    pub async fn execute(
        plan: &serde_json::Value,
        driver: &mut VisualDriver,
        session_steps: &mut Vec<SmartStep>,
        session: &mut crate::session_store::Session,
        history: &mut Vec<String>,
        consecutive_failures: &mut usize,
        last_read_number: &mut Option<String>,
        goal: &str,
    ) -> Result<()> {
        let action_type = plan["action"].as_str().unwrap_or("fail");
        let mut description = format!("Executing {}", action_type);
        let mut action_status_override: Option<&str> = None;
        let mut step_to_record: Option<SmartStep> = None;

        // Pre-action focus: ensure target app is frontmost for UI-sensitive actions
        if matches!(action_type, "snapshot" | "click_visual" | "read") {
            if let Some(target_app) = heuristics::goal_primary_app(goal) {
                let _ = heuristics::ensure_app_focus(target_app, 3).await;
            } else if heuristics::prefer_lucky_only(goal) {
                let _ = heuristics::ensure_app_focus("Safari", 2).await;
                let _ = heuristics::ensure_app_focus("Google Chrome", 2).await;
            }
        }
        
        // Safari privacy report popover close (pre-step safeguard)
        if action_type == "snapshot" {
            if let Ok(front) = crate::tool_chaining::CrossAppBridge::get_frontmost_app() {
                if front.eq_ignore_ascii_case("Safari") {
                    let close_script = r#"
                        tell application "System Events"
                            tell process "Safari"
                                if exists window 1 then
                                    if exists pop over 1 of window 1 then
                                        try
                                            click button 1 of pop over 1 of window 1
                                        end try
                                    end if
                                end if
                            end tell
                        end tell
                    "#;
                    let _ = applescript::run(close_script);
                }
            }
        }

        match action_type {
            "click_visual" => {
                let desc = plan["description"].as_str().unwrap_or("element");
                let looks_like_dialog = heuristics::looks_like_dialog(desc);
                
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
                            description = "Closed dialog via button click".to_string();
                            action_status_override = Some("success");
                        }
                        Err(e) => {
                            description = format!("Dialog close failed: {}", e);
                            action_status_override = Some("failed");
                            *consecutive_failures += 1;
                        }
                    }
                } else {
                    let step = SmartStep::new(UiAction::ClickVisual(desc.to_string()), desc);
                    driver.add_step(step.clone());
                    step_to_record = Some(step);
                    description = format!("Clicked '{}'", desc);
                }
            },
            "type" => {
                let mut text = plan["text"].as_str().unwrap_or("").to_string();
                let mut forced_app = false;
                if let Some(app_name) = plan.get("app").and_then(|v| v.as_str()) {
                    let _ = crate::tool_chaining::CrossAppBridge::switch_to_app(app_name);
                    let _ = heuristics::ensure_app_focus(app_name, 3).await;
                    forced_app = true;
                }

                let looks_like_calc = text.contains('*') || text.contains('+') || text.contains('-') || text.contains('/') || text.contains('=');
                if !forced_app && looks_like_calc {
                    if let Ok(front) = crate::tool_chaining::CrossAppBridge::get_frontmost_app() {
                        if !front.eq_ignore_ascii_case("Calculator") {
                            let _ = heuristics::ensure_app_focus("Calculator", 3).await;
                        }
                    }
                } else if !forced_app {
                    if let Some(target_app) = heuristics::goal_primary_app(goal) {
                        if let Ok(front) = crate::tool_chaining::CrossAppBridge::get_frontmost_app() {
                            if !front.eq_ignore_ascii_case(target_app) {
                                let _ = heuristics::ensure_app_focus(target_app, 3).await;
                            }
                        }
                    }
                }

                if let Ok(app_name) = crate::tool_chaining::CrossAppBridge::get_frontmost_app() {
                    if app_name.eq_ignore_ascii_case("Calculator") {
                        let mut cleaned = text.replace('×', "*")
                            .replace('x', "*")
                            .replace('X', "*")
                            .replace(' ', "");

                        if cleaned.chars().all(|c| c.is_ascii_digit()) {
                            if let Some(num) = last_read_number.as_ref() {
                                if num.contains('.') {
                                    cleaned = num.clone();
                                }
                            }
                        }

                        if (cleaned.contains('*') || cleaned.contains('+') || cleaned.contains('-') || cleaned.contains('/'))
                            && !cleaned.ends_with('=')
                        {
                            cleaned.push('=');
                        }
                        text = cleaned;
                    }

                    if app_name.eq_ignore_ascii_case("Mail") {
                        let _ = heuristics::focus_text_area("Mail", heuristics::looks_like_subject(&text));
                    } else if app_name.eq_ignore_ascii_case("Notes") {
                        let _ = heuristics::focus_text_area("Notes", false);
                    } else if app_name.eq_ignore_ascii_case("TextEdit") {
                        let _ = heuristics::focus_text_area("TextEdit", false);
                    }
                }

                let step = SmartStep::new(UiAction::Type(text.to_string()), "Typing");
                driver.add_step(step.clone());
                step_to_record = Some(step);
                description = format!("Typed '{}'", text);
            },
            "key" => {
                let key_raw = plan["key"].as_str().unwrap_or("return");
                let key_norm = key_raw.trim().to_lowercase().replace(' ', "");
                let mut shortcut_modifiers: Vec<String> = Vec::new();
                let mut shortcut_key: Option<String> = None;

                if key_norm.contains('+') {
                    for part in key_norm.split('+').filter(|p| !p.is_empty()) {
                        match part {
                            "cmd" | "command" => shortcut_modifiers.push("command".to_string()),
                            "shift" => shortcut_modifiers.push("shift".to_string()),
                            "option" | "alt" => shortcut_modifiers.push("option".to_string()),
                            "control" | "ctrl" => shortcut_modifiers.push("control".to_string()),
                            other => shortcut_key = Some(other.to_string()),
                        }
                    }
                }

                if key_norm == "escape" || key_norm == "esc" {
                    let script = "tell application \"System Events\" to key code 53";
                    let _ = std::process::Command::new("osascript").arg("-e").arg(script).status();
                    description = "Pressed 'escape'".to_string();
                } else if !shortcut_modifiers.is_empty() && shortcut_key.is_some() {
                    let key = shortcut_key.unwrap_or_default();
                    let step = SmartStep::new(UiAction::KeyboardShortcut(key.clone(), shortcut_modifiers.clone()), "Shortcut");
                    driver.add_step(step.clone());
                    step_to_record = Some(step);
                    description = format!("Shortcut '{}' + {:?}", key, shortcut_modifiers);
                } else {
                    let key_char = match key_norm.as_str() {
                        "return" | "enter" => "\r",
                        "tab" => "\t",
                        _ => key_raw
                    };
                    let step = SmartStep::new(UiAction::Type(key_char.to_string()), "Pressing Key");
                    driver.add_step(step.clone());
                    step_to_record = Some(step);
                    description = format!("Pressed '{}'", key_raw);
                }
            },
            "shortcut" => {
                let mut key = plan["key"].as_str().unwrap_or("").to_string();
                let mut modifiers: Vec<String> = if let Some(arr) = plan["modifiers"].as_array() {
                    arr.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect()
                } else {
                    Vec::new()
                };
                
                if let Some(app_name) = plan.get("app").and_then(|v| v.as_str()) {
                    let _ = crate::tool_chaining::CrossAppBridge::switch_to_app(app_name);
                    let _ = heuristics::ensure_app_focus(app_name, 3).await;
                }
                
                let step = SmartStep::new(UiAction::KeyboardShortcut(key.clone(), modifiers.clone()), "Shortcut");
                driver.add_step(step.clone());
                step_to_record = Some(step);
                description = format!("Shortcut '{}' + {:?}", key, modifiers);
            },
             "scroll" => {
                let dir = plan["direction"].as_str().unwrap_or("down");
                let step = SmartStep::new(UiAction::Scroll(dir.to_string()), "Scrolling");
                driver.add_step(step.clone());
                step_to_record = Some(step);
                description = format!("Scrolled {}", dir);
            },
             "open_url" => {
                let url = plan["url"].as_str().unwrap_or("https://google.com");
                info!("      🌐 Opening URL: '{}'", url);
                if let Err(e) = applescript::open_url(url) {
                    error!("      ❌ Open URL failed: {}", e);
                    description = format!("Failed to open URL: {}", e);
                    action_status_override = Some("failed");
                    *consecutive_failures += 1;
                } else {
                    description = format!("Opened URL '{}'", url);
                    let mut browser_auto = crate::browser_automation::get_browser_automation();
                    browser_auto.reset_snapshot();
                    action_status_override = Some("success");
                }
            },
            "open_app" => {
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
                        info!("      🚀 Launching/Focusing App: '{}' (Canonical: '{}')", name, canonical_name);
                        match crate::tool_chaining::CrossAppBridge::switch_to_app(&canonical_name) {
                            Ok(_) => {
                                let _ = heuristics::ensure_app_focus(&canonical_name, 3).await;
                                let step = SmartStep::new(UiAction::Type(canonical_name.clone()), "Open App");
                                session_steps.push(step);
                                description = format!("Opened app: {}", canonical_name);
                                session.add_message("tool", &format!("open_app: {}", canonical_name));
                            }
                            Err(e) => {
                                error!("      ❌ App open failed: {}", e);
                                description = format!("Open app failed: {}", e);
                                action_status_override = Some("failed");
                            }
                        }
                    },
                    Err(e) => {
                         error!("      ❌ [Reality] REJECTED: {}", e);
                         description = format!("Failed: {}", e);
                         action_status_override = Some("failed");
                    }
                }
            },
            "wait" => {
                let secs = plan["seconds"].as_u64().unwrap_or(2);
                let step = SmartStep::new(UiAction::Wait(secs), "Waiting");
                driver.add_step(step.clone());
                step_to_record = Some(step);
                description = format!("Waited {}s", secs);
            },
            "fail" => {
                let reason = plan["reason"].as_str().unwrap_or("Unknown");
                return Err(anyhow::anyhow!("Agent failed: {}", reason));
            },
            _ => {
                // For other actions like save_routine, replay_routine, read, read_file - allow them or add implementations here
                // For simplicity in this migration step, we handle common actions.
                description = format!("Action '{}' not fully implemented in ActionRunner yet", action_type);
            }
        }
        
        // Log to history and session
        let status = action_status_override.unwrap_or("success");
        history.push(description.clone());
        session.add_step(action_type, &description, status, None);
        let _ = crate::session_store::save_session(session);

        if status != "success" && action_type != "fail" {
             *consecutive_failures += 1;
        } else {
             *consecutive_failures = 0;
        }

        Ok(())
    }
}
