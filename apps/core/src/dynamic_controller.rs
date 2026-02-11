use crate::llm_gateway::LLMClient;
use crate::visual_driver::{VisualDriver, UiAction, SmartStep};
use crate::policy::PolicyEngine;
use crate::schema::AgentAction;
use anyhow::Result;
use serde_json::Value; 
use crate::db; 
use log::{info, warn, error};
use tokio::sync::mpsc;
use crate::schema::EventEnvelope;
use chrono::Utc;
use uuid::Uuid;
use crate::screen_recorder::ScreenRecorder;
use crate::applescript;

pub struct DynamicController {
    llm: LLMClient,
    max_steps: usize,
    tx: Option<mpsc::Sender<String>>,
}

impl DynamicController {
    pub fn new(llm: LLMClient, tx: Option<mpsc::Sender<String>>) -> Self {
        Self {
            llm,
            max_steps: 25,
            tx,
        }
    }

    /// Detect if the same action has been repeated 3+ times (clawdbot anti-loop pattern)
    fn detect_action_loop(history: &[String], current_action: &str) -> bool {
        if history.len() < 2 {
            return false;
        }
        
        // Extract action type from current plan
        let current_key = Self::extract_action_key(current_action);
        
        // Check last 2 entries
        let mut match_count = 0;
        for entry in history.iter().rev().take(2) {
            if Self::extract_action_key(entry) == current_key {
                match_count += 1;
            }
        }
        
        match_count >= 2 // If last 2 are same as current, it's a loop
    }
    
    /// Extract a simplified key from action for comparison (e.g., "key:command+l" or "click:button")
    fn extract_action_key(action_str: &str) -> String {
        // Simple extraction: look for action type and key values
        if action_str.contains("\"action\":\"key\"") {
            if let Some(key_start) = action_str.find("\"key\":\"") {
                let rest = &action_str[key_start + 7..];
                if let Some(key_end) = rest.find("\"") {
                    return format!("key:{}", &rest[..key_end]);
                }
            }
        } else if action_str.contains("\"action\":\"click_visual\"") {
            return "click_visual".to_string();
        } else if action_str.contains("\"action\":\"type\"") {
            return "type".to_string();
        }
        action_str.chars().take(50).collect()
    }

    pub async fn surf(&self, goal: &str) -> Result<()> {
        self.surf_with_session(goal, None).await
    }
    
    /// Surf with optional session key for persistence
    pub async fn surf_with_session(&self, goal: &str, session_key: Option<&str>) -> Result<()> {
        println!("🌊 Starting Dynamic Surf: '{}'", goal);
        
        // [Session] Initialize session store and create/resume session
        let _ = crate::session_store::init_session_store();
        let mut session = crate::session_store::Session::new(goal, session_key);
        session.add_message("user", goal);
        
        // [Blackbox] Start Recording
        let recorder = ScreenRecorder::new();
        recorder.cleanup_old_recordings(); // Auto-clean
        let _ = recorder.start(); // Start recording

        // [Reality] Scan available applications
        let _ = crate::reality_check::scan_app_inventory();

        let mut history: Vec<String> = Vec::new(); // ... (existing code)
        let mut action_history: Vec<String> = Vec::new(); // For loop detection (raw plan JSON)
        let mut session_steps: Vec<SmartStep> = Vec::new(); // Track for macro recording
        let mut consecutive_failures = 0;
        let mut last_read_number: Option<String> = None;

        fn extract_number(text: &str) -> Option<String> {
            let mut buf = String::new();
            let mut started = false;
            for ch in text.chars() {
                if ch.is_ascii_digit() || ch == '.' || ch == ',' {
                    buf.push(ch);
                    started = true;
                } else if started {
                    break;
                }
            }
            if buf.is_empty() {
                None
            } else {
                Some(buf.replace(',', ""))
            }
        }

        fn calculator_has_input(history: &[String]) -> bool {
            let mut seen_open = false;
            for entry in history.iter().rev() {
                if entry.contains("Opened app: Calculator") {
                    seen_open = true;
                    break;
                }
            }
            if !seen_open {
                return false;
            }
            for entry in history.iter().rev() {
                if entry.contains("Opened app: Calculator") {
                    break;
                }
                if entry.starts_with("Typed '") {
                    return true;
                }
            }
            false
        }

        fn goal_mentions_calculation(goal: &str) -> bool {
            let lower = goal.to_lowercase();
            lower.contains("계산")
                || lower.contains("calculate")
                || lower.contains("곱")
                || lower.contains("×")
                || lower.contains("*")
                || lower.contains("plus")
                || lower.contains("minus")
        }

        fn goal_is_ui_task(goal: &str) -> bool {
            let lower = goal.to_lowercase();
            let apps = ["safari", "notes", "finder", "preview", "textedit", "mail", "calculator"];
            apps.iter().any(|app| lower.contains(app))
        }

        fn goal_mentions_desktop(goal: &str) -> bool {
            let lower = goal.to_lowercase();
            lower.contains("desktop") || lower.contains("데스크탑")
        }

        fn goal_mentions_image(goal: &str) -> bool {
            let lower = goal.to_lowercase();
            lower.contains("image") || lower.contains("이미지") || lower.contains(".png") || lower.contains(".jpg")
        }

        for i in 1..=self.max_steps {
            println!("\n🔄 [Step {}/{}] Observing...", i, self.max_steps);
            
            // 1. Capture Screen
            let (image_b64, _scale) = VisualDriver::capture_screen()?;
            
            // 2. Plan (Think) - WITH RETRY
            println!("   🧠 Thinking...");
            let retry_config = crate::retry_logic::RetryConfig::default();
            let llm_ref = &self.llm;
            let goal_ref = goal;
            let image_ref = &image_b64;
            let history_ref = &history;
            
            let mut plan = crate::retry_logic::with_retry(&retry_config, "LLM Vision", || async {
                llm_ref.plan_vision_step(goal_ref, image_ref, history_ref).await
            }).await?;
            
            // 2.5 Anti-Loop Detection (clawdbot pattern)
            let action_str = plan.to_string();
            if Self::detect_action_loop(&action_history, &action_str) {
                println!("   🔄 LOOP DETECTED: Same action {} times. Injecting alternative...", 3);
                if goal_mentions_desktop(goal) && goal_mentions_image(goal) {
                    plan = serde_json::json!({
                        "action": "open_desktop_image"
                    });
                } else {
                    // Force a wait + report to break the loop
                    plan = serde_json::json!({
                        "action": "report",
                        "message": "I'm stuck repeating the same action. The screen may have changed. Let me try a different approach."
                    });
                }
            }
            // Track action for loop detection
            action_history.push(action_str.clone());
            
            println!("   💡 Idea: {}", plan);


            // 2.6 Flatten Nested JSON (Common LLM error: {"action": {"action": "type"...}})
            if plan["action"].is_object() {
                println!("   🔧 Flattening nested JSON action...");
                plan = plan["action"].clone();
            }

            // 3. Act
            let action_type = plan["action"].as_str().unwrap_or("fail");
            let mut driver = VisualDriver::new();
            let mut description = format!("Step {}", i);
            let mut step_to_record: Option<SmartStep> = None; // Step to save (if Action)
            let mut event_type = "action";
            let mut action_status_override: Option<&str> = None;

            match action_type {
                "click_visual" => {
                    let desc = plan["description"].as_str().unwrap_or("element");
                    let desc_lower = desc.to_lowercase();
                    let looks_like_dialog = desc_lower.contains("cancel")
                        || desc_lower.contains("취소")
                        || desc_lower.contains("open dialog")
                        || desc_lower.contains("open file")
                        || desc_lower.contains("save dialog");
                    
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
                                consecutive_failures += 1;
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
                    }

                    let step = SmartStep::new(UiAction::Type(text.to_string()), "Typing");
                    driver.add_step(step.clone());
                    step_to_record = Some(step);
                    description = format!("Typed '{}'", text);
                },
                "key" => {
                    let key_raw = plan["key"].as_str().unwrap_or("return");

                    // Support shortcut-like strings (e.g., "command+l", "cmd+shift+g")
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
                    let key = plan["key"].as_str().unwrap_or("");
                    let modifiers_val = plan["modifiers"].as_array();
                    let modifiers: Vec<String> = if let Some(arr) = modifiers_val {
                        arr.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect()
                    } else {
                        Vec::new()
                    };
                    
                    let step = SmartStep::new(UiAction::KeyboardShortcut(key.to_string(), modifiers.clone()), "Shortcut");
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
                "read" => {
                    let query = plan["query"].as_str().unwrap_or("Describe the screen");
                    info!("      📖 Reading: '{}'", query);
                    if let Ok(app_name) = crate::tool_chaining::CrossAppBridge::get_frontmost_app() {
                        if app_name.eq_ignore_ascii_case("Calculator")
                            && goal_mentions_calculation(goal)
                            && !calculator_has_input(&history)
                        {
                            description = "BLOCKED: Calculator read requested before calculation".to_string();
                            consecutive_failures += 1;
                            history.push(description.clone());
                            continue;
                        }
                    }

                    match self.llm.analyze_screen(query, &image_b64).await {
                        Ok(text) => {
                            info!("      📝 Extracted: {}", text);
                            description = format!("Read Info: '{}' -> '{}'", query, text);
                            last_read_number = extract_number(&text);
                        },
                        Err(e) => {
                             error!("      ❌ Read failed: {}", e);
                             description = format!("Failed to read: {}", e);
                        }
                    }
                },
                "open_url" => {
                    let url = plan["url"].as_str().unwrap_or("https://google.com");
                    info!("      🌐 Opening URL: '{}'", url);
                    if let Err(e) = applescript::open_url(url) {
                        error!("      ❌ Open URL failed: {}", e);
                        description = format!("Failed to open URL: {}", e);
                    } else {
                        description = format!("Opened URL '{}'", url);
                    }
                },
                "save_routine" => {
                    let name = plan["name"].as_str().unwrap_or("unnamed");
                    info!("      💾 Saving Routine as '{}' ({} steps)...", name, session_steps.len());
                    match serde_json::to_string(&session_steps) {
                        Ok(json) => {
                            if let Err(e) = db::save_learned_routine(name, &json) {
                                error!("      ❌ Save failed: {}", e);
                                description = format!("Failed to save routine: {}", e);
                            } else {
                                info!("      ✅ Routine Saved!");
                                description = format!("Saved Routine '{}'", name);
                            }
                        },
                        Err(e) => {
                            error!("      ❌ Serialization failed: {}", e);
                             description = format!("Failed to serialize routine: {}", e);
                        }
                    }
                },
                "replay_routine" => {
                    let name = plan["name"].as_str().unwrap_or("unnamed");
                    info!("      ⏪ Replaying Routine '{}'...", name);
                    match db::get_learned_routine(name) {
                        Ok(Some(routine)) => {
                            match serde_json::from_str::<Vec<SmartStep>>(&routine.steps_json) {
                                Ok(steps) => {
                                    info!("      ▶️ Loaded {} steps. Executing...", steps.len());
                                    // Construct a driver with all steps and run it
                                    let mut replay_driver = VisualDriver::new();
                                    for s in steps {
                                        replay_driver.add_step(s);
                                    }
                                    if let Err(e) = replay_driver.execute(Some(&self.llm)).await {
                                        error!("      ❌ Replay failed: {}", e);
                                        description = format!("Replay '{}' failed: {}", name, e);
                                    } else {
                                        description = format!("Replayed Routine '{}'", name);
                                    }
                                },

                                Err(e) => { // JSON parse error
                                     error!("      ❌ Corrupt routine data: {}", e);
                                     description = format!("Corrupt routine data: {}", e);
                                }
                            }
                        },
                        Ok(None) => {
                             error!("      ❌ Routine '{}' not found.", name);
                             description = format!("Routine '{}' not found", name);
                        },
                        Err(e) => {
                             error!("      ❌ DB Error: {}", e);
                             description = format!("DB Error: {}", e);
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
                "done" => {
                    println!("✅ Goal Achieved!");
                    let _ = recorder.stop(); // Stop recording
                    return Ok(());
                },
                "reply" => {
                    let text = plan["text"].as_str().unwrap_or("...");
                    info!("🤖 Agent: {}", text);
                    let _ = recorder.stop(); // Stop recording
                    return Ok(()); // Chat is a single-turn action
                },
                "fail" => {
                    let reason = plan["reason"].as_str().unwrap_or("Unknown");
                    println!("❌ Agent gave up: {}", reason);
                    let _ = recorder.stop(); // Stop recording
                    return Err(anyhow::anyhow!("Agent failed: {}", reason));
                },
                "open_app" => {
                    let name = plan["name"].as_str().unwrap_or("Finder");
                    info!("      🚀 Launching/Focusing App: '{}'", name);
                    
                    // [Reality] Verify App Exists & Get Canonical Name
                    match crate::reality_check::verify_app_exists(name) {
                        Ok(canonical_name) => {
                            info!("      🚀 Launching/Focusing App: '{}' (Canonical: '{}')", name, canonical_name);
                            // Use CrossAppBridge with canonical name
                            match crate::tool_chaining::CrossAppBridge::switch_to_app(&canonical_name) {
                                Ok(_) => {
                                    let step = SmartStep::new(UiAction::Type(canonical_name.clone()), "Open App");
                                    session_steps.push(step);
                                    description = format!("Opened app: {}", canonical_name);
                                    session.add_message("tool", &format!("open_app: {}", canonical_name));
                                }
                                Err(e) => {
                                    error!("      ❌ App open failed: {}", e);
                                    description = format!("Open app failed: {}", e);
                                }
                            }
                        },
                        Err(e) => {
                             error!("      ❌ [Reality] REJECTED: {}", e);
                             description = format!("Failed: {}", e);
                        }
                    }
            }, // Close match arm
                "open_url" | "open_browser" => {
                    let url = plan["url"].as_str().unwrap_or("https://google.com");
                    info!("      🌐 Opening URL: '{}'", url);
                    // Use 'open' command which uses default browser
                    match std::process::Command::new("open").arg(url).output() {
                        Ok(_) => {
                            let step = SmartStep::new(UiAction::Type(url.to_string()), "Open URL");
                            session_steps.push(step);
                            description = format!("Opened URL '{}'", url);
                            tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                        },
                        Err(e) => {
                             error!("      ❌ Failed to open URL: {}", e);
                             description = format!("Failed to open URL: {}", e);
                        }
                    }
                },
                "read_file" => {
                    let path = plan["path"].as_str().unwrap_or("");
                    info!("      📄 Reading File: '{}'", path);
                    match crate::content_extractor::ContentExtractor::extract(path) {
                        Ok(content) => {
                            let preview = if content.len() > 500 {
                                format!("{}...", &content[..500])
                            } else {
                                content.clone()
                            };
                            info!("      📝 Extracted: {}", preview);
                            description = format!("Read File '{}':\n{}", path, content);
                        },
                        Err(e) => {
                             error!("      ❌ Read failed: {}", e);
                             description = format!("Failed to read file '{}': {}", path, e);
                        }
                    }
                },
                // =====================================================
                // NEW CLAWDBOT-PORTED ACTIONS
                // =====================================================
                "shell" | "run_shell" => {
                    let cmd = plan["command"].as_str().unwrap_or("");
                    info!("      🐚 Shell Command: '{}'", cmd);
                    
                    // [ApprovalGate] Check command safety
                    let level = crate::approval_gate::ApprovalGate::check_command(cmd);
                    match level {
                        crate::approval_gate::ApprovalLevel::Blocked => {
                            error!("      🚫 BLOCKED: Command is dangerous: '{}'", cmd);
                            description = format!("BLOCKED: Dangerous command '{}'", cmd);
                        },
                        crate::approval_gate::ApprovalLevel::RequireApproval => {
                            warn!("      ⚠️ APPROVAL REQUIRED: '{}'", cmd);
                            description = format!("Approval required for: '{}'", cmd);
                            // TODO: Implement user approval flow
                        },
                        crate::approval_gate::ApprovalLevel::AutoApprove => {
                            // Use advanced bash executor
                            let config = crate::bash_executor::BashExecConfig {
                                timeout_ms: 30000,
                                approval_required: false, // Already checked above
                                ..Default::default()
                            };
                            
                            match crate::bash_executor::execute_bash(cmd, &config) {
                                Ok(result) => {
                                    if result.success {
                                        let preview = if result.stdout.len() > 500 {
                                            format!("{}...", &result.stdout[..500])
                                        } else {
                                            result.stdout.clone()
                                        };
                                        info!("      ✅ Output ({}ms): {}", result.duration_ms, preview);
                                        description = format!("Shell '{}' -> {}", cmd, preview);
                                        
                                        // Record to session
                                        session.add_message("tool", &format!("shell: {}\n{}", cmd, result.stdout));
                                    } else {
                                        error!("      ❌ Shell failed: {}", result.stderr);
                                        description = format!("Shell failed: {}", result.stderr);
                                    }
                                },
                                Err(e) => {
                                    error!("      ❌ Bash executor error: {}", e);
                                    description = format!("Shell failed: {}", e);
                                }
                            }
                        }
                    }
                },
                "spawn_agent" => {
                    let name = plan["name"].as_str().unwrap_or("worker");
                    let task = plan["task"].as_str().unwrap_or("");
                    info!("      🤖 Spawning Subagent '{}' for: '{}'", name, task);
                    
                    let agent_id = crate::subagent::SubagentManager::spawn(name, task);
                    crate::subagent::SubagentManager::start(&agent_id);
                    
                    // For now, subagents run in the same context (future: spawn threads)
                    description = format!("Spawned subagent '{}' (id: {})", name, agent_id);
                    
                    // TODO: Actually execute the subtask
                    // For now, just mark as started
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    crate::subagent::SubagentManager::complete(&agent_id, "Subtask simulated");
                },
                "snapshot" | "take_snapshot" => {
                    info!("      📸 Taking Browser/UI Snapshot...");
                    
                    let browser = crate::browser_automation::get_browser_automation();
                    match browser.take_snapshot() {
                        Ok(refs) => {
                            info!("      ✅ Snapshot captured: {} elements", refs.len());
                            // Log some refs for debugging
                            for r in refs.iter().take(5) {
                                info!("         - {} [{}]: '{}'", r.id, r.role, r.name);
                            }
                            description = format!("Snapshot: {} UI elements captured", refs.len());
                        },
                        Err(e) => {
                            error!("      ❌ Snapshot failed: {}", e);
                            description = format!("Snapshot failed: {}", e);
                        }
                    }
                },
                "click_ref" => {
                    let ref_id = plan["ref"].as_str().unwrap_or("");
                    let double_click = plan["double_click"].as_bool().unwrap_or(false);
                    info!("      🖱️ Click by Ref: '{}'", ref_id);
                    
                    let browser = crate::browser_automation::get_browser_automation();
                    match browser.click_by_ref(ref_id, double_click) {
                        Ok(_) => {
                            description = format!("Clicked ref '{}'", ref_id);
                        },
                        Err(e) => {
                            error!("      ❌ Click failed: {}", e);
                            description = format!("Click ref failed: {}", e);
                        }
                    }
                },
                // =====================================================
                // CROSS-APP BRIDGE ACTIONS (NEW!)
                // =====================================================
                "switch_app" | "activate_app" => {
                    let app_name = plan["app"].as_str()
                        .or(plan["name"].as_str())
                        .unwrap_or("");
                    info!("      🔀 Switching to app: '{}'", app_name);
                    
                    match crate::tool_chaining::CrossAppBridge::switch_to_app(app_name) {
                        Ok(_) => {
                            description = format!("Switched to app: {}", app_name);
                            session.add_message("tool", &format!("switch_app: {}", app_name));
                        }
                        Err(e) => {
                            error!("      ❌ App switch failed: {}", e);
                            description = format!("Failed to switch to {}: {}", app_name, e);
                        }
                    }
                },
                "copy" | "copy_to_clipboard" => {
                    let text = plan["text"].as_str()
                        .or(plan["content"].as_str())
                        .unwrap_or("");
                    info!("      📋 Copying to clipboard: '{}...'", &text[..text.len().min(30)]);
                    
                    match crate::tool_chaining::CrossAppBridge::copy_to_clipboard(text) {
                        Ok(_) => {
                            description = format!("Copied {} chars to clipboard", text.len());
                        }
                        Err(e) => {
                            error!("      ❌ Copy failed: {}", e);
                            description = format!("Copy failed: {}", e);
                        }
                    }
                },
                "paste" | "paste_clipboard" => {
                    info!("      📋 Pasting from clipboard...");
                    
                    match crate::tool_chaining::CrossAppBridge::paste() {
                        Ok(_) => {
                            description = "Pasted from clipboard".to_string();
                        }
                        Err(e) => {
                            error!("      ❌ Paste failed: {}", e);
                            description = format!("Paste failed: {}", e);
                        }
                    }
                },
                "read_clipboard" | "get_clipboard" => {
                    info!("      📋 Reading clipboard...");
                    
                    match crate::tool_chaining::CrossAppBridge::get_clipboard() {
                        Ok(content) => {
                            let preview = if content.len() > 100 {
                                format!("{}...", &content[..100])
                            } else {
                                content.clone()
                            };
                            info!("      ✅ Clipboard: {}", preview);
                            description = format!("Clipboard: {}", preview);
                            session.add_message("tool", &format!("clipboard: {}", content));
                        }
                        Err(e) => {
                            error!("      ❌ Read clipboard failed: {}", e);
                            description = format!("Read clipboard failed: {}", e);
                        }
                    }
                },
                "transfer" | "copy_between_apps" => {
                    // Complex action: read from one app, paste to another
                    let from_app = plan["from"].as_str().unwrap_or("");
                    let to_app = plan["to"].as_str().unwrap_or("");
                    info!("      🔄 Transferring data: {} -> {}", from_app, to_app);
                    
                    // Step 1: Switch to source app
                    if !from_app.is_empty() {
                        let _ = crate::tool_chaining::CrossAppBridge::switch_to_app(from_app);
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    }
                    
                    // Step 2: Select all & copy (Cmd+A, Cmd+C)
                    let script = r#"tell application "System Events"
                        keystroke "a" using command down
                        delay 0.2
                        keystroke "c" using command down
                    end tell"#;
                    let _ = std::process::Command::new("osascript").arg("-e").arg(script).status();
                    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                    
                    // Step 3: Switch to destination app
                    if !to_app.is_empty() {
                        let _ = crate::tool_chaining::CrossAppBridge::switch_to_app(to_app);
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    }
                    
                    // Step 4: Paste (Cmd+V)
                    let _ = crate::tool_chaining::CrossAppBridge::paste();
                    
                    description = format!("Transferred data from {} to {}", from_app, to_app);
                    session.add_message("tool", &format!("transfer: {} -> {}", from_app, to_app));
                },
                // =====================================================
                // MCP - EXTERNAL SERVICE INTEGRATION
                // =====================================================
                "mcp" | "mcp_call" | "external_tool" => {
                    let server = plan["server"].as_str().unwrap_or("filesystem");
                    let tool = plan["tool"].as_str().unwrap_or("");
                    let args = plan["arguments"].clone();
                    let mut handled = false;
                    
                    if server == "filesystem" && goal_is_ui_task(goal) {
                        if tool == "search_files" && goal_mentions_desktop(goal) && goal_mentions_image(goal) {
                            let script = r#"
                                tell application "Finder"
                                    activate
                                    set desktopFolder to (path to desktop folder)
                                    set theFiles to files of desktopFolder whose name extension is in {"png","jpg","jpeg","PNG","JPG","JPEG"}
                                    if (count of theFiles) > 0 then
                                        open item 1 of theFiles
                                    end if
                                end tell
                            "#;
                            match applescript::run(script) {
                                Ok(_) => {
                                    description = "Opened first Desktop image via Finder".to_string();
                                    action_status_override = Some("success");
                                }
                                Err(e) => {
                                    description = format!("Failed to open Desktop image: {}", e);
                                    action_status_override = Some("failed");
                                    consecutive_failures += 1;
                                }
                            }
                            handled = true;
                        }
                        if !handled {
                            description = "BLOCKED: MCP filesystem usage disallowed for UI tasks".to_string();
                            consecutive_failures += 1;
                            history.push(description.clone());
                            continue;
                        }
                    }
                    if handled {
                        // Skip MCP call entirely, already handled by fallback.
                    } else {
                        println!("      🌐 [MCP] Calling {}/{} with {:?}", server, tool, args);
                    
                    // Initialize MCP if needed
                    let _ = crate::mcp_client::init_mcp();
                    
                    match crate::mcp_client::call_mcp_tool(server, tool, args) {
                        Ok(result) => {
                            let result_str = serde_json::to_string_pretty(&result)
                                .unwrap_or_else(|_| result.to_string());
                            let preview = if result_str.len() > 300 {
                                format!("{}...", &result_str[..300])
                            } else {
                                result_str.clone()
                            };
                            println!("      ✅ MCP Result: {}", preview);
                            description = format!("MCP {}/{}: {}", server, tool, preview);
                            session.add_message("tool", &format!("mcp: {}/{}\n{}", server, tool, result_str));
                        }
                        Err(e) => {
                            println!("      ❌ MCP call failed: {}", e);
                            description = format!("MCP failed: {}", e);
                        }
                    }
                    }
                },
                "mcp_list" | "list_mcp_tools" => {
                    info!("      📋 [MCP] Listing available tools...");
                    
                    let _ = crate::mcp_client::init_mcp();
                    
                    match crate::mcp_client::get_mcp_registry() {
                        Ok(guard) => {
                            if let Some(registry) = guard.as_ref() {
                                let tools = registry.list_all_tools();
                                let tools_str: Vec<String> = tools.iter()
                                    .map(|(server, tool)| format!("{}/{}: {}", server, tool.name, tool.description))
                                    .collect();
                                description = format!("MCP Tools: {}", tools_str.join(", "));
                                session.add_message("tool", &format!("mcp_tools: {:?}", tools_str));
                            } else {
                                description = "MCP not initialized".to_string();
                            }
                        }
                        Err(e) => {
                            description = format!("MCP list failed: {}", e);
                        }
                    }
                },
                "select_text" => {
                    let text = plan["text"].as_str().unwrap_or("");
                    let safe_text = text.replace('\\', "\\\\").replace('\"', "\\\"");
                    let script = format!(r#"
                        tell application "System Events"
                            keystroke "f" using command down
                            delay 0.2
                            keystroke "{}"
                            delay 0.2
                            key code 36
                        end tell
                    "#, safe_text);
                    
                    let task = tokio::task::spawn_blocking(move || {
                        applescript::run(&script)
                    });
                    match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                        Ok(Ok(Ok(_))) => {
                            description = format!("Selected text '{}'", text);
                        },
                        Ok(Ok(Err(e))) => {
                            description = format!("Select text failed: {}", e);
                            action_status_override = Some("failed");
                            consecutive_failures += 1;
                        },
                        Ok(Err(_)) => {
                            description = "Select text failed: Task Panic".to_string();
                            action_status_override = Some("failed");
                            consecutive_failures += 1;
                        },
                        Err(_) => {
                            description = "Select text failed: Timed Out".to_string();
                            action_status_override = Some("failed");
                            consecutive_failures += 1;
                        }
                    }
                }
                "open_desktop_image" => {
                    let script = r#"
                        tell application "Finder"
                            activate
                            set desktopFolder to (path to desktop folder)
                            set theFiles to files of desktopFolder whose name extension is in {"png","jpg","jpeg","PNG","JPG","JPEG"}
                            if (count of theFiles) > 0 then
                                open item 1 of theFiles
                            end if
                        end tell
                    "#;
                    
                    match applescript::run(script) {
                        Ok(_) => {
                            description = "Opened first Desktop image via Finder".to_string();
                            action_status_override = Some("success");
                        }
                        Err(e) => {
                            description = format!("Failed to open Desktop image: {}", e);
                            action_status_override = Some("failed");
                            consecutive_failures += 1;
                        }
                    }
                }
                _ => {
                    if action_type.starts_with("filesystem/") {
                        description = format!("BLOCKED: Use mcp action for filesystem tools (got {})", action_type);
                        action_status_override = Some("blocked");
                        consecutive_failures += 1;
                    } else {
                        warn!("⚠️ Unknown action: {}", action_type);
                    }
                }
            }

            // Execute the single step (Unless it was pure cognitive like read/save/replay which have handled themselves)
            // Replay handles its own execution. Read/Save are instant.
            // Only Execute if we added steps to `driver` (Action steps)
            if !driver.steps.is_empty() {
                // [Phase 28] PolicyEngine Check - Enforce security policies on UI actions
                // Note: For surf actions (user-initiated), write_lock is DISABLED
                // because the user explicitly asked the agent to perform UI interactions.
                let mut policy = PolicyEngine::new();
                policy.unlock(); // Allow Agent to act on user's behalf
                let action_for_policy = AgentAction::UiClick { element_id: description.clone(), double_click: false };
                
                if policy.check(&action_for_policy).is_err() {
                    println!("   🛡️ Action blocked by PolicyEngine: {}", description);
                    history.push(format!("BLOCKED_BY_POLICY: {}", description));
                    session.add_step(action_type, &description, "blocked", None);
                    consecutive_failures += 1;
                } else {
                    println!("   ▶️ Executing action via VisualDriver...");
                    if let Err(e) = driver.execute(Some(&self.llm)).await {
                        println!("   ⚠️ Action failed: {}", e);
                        history.push(format!("FAILED: {}", description));
                        session.add_step(action_type, &description, "failed", Some(serde_json::json!({"error": e.to_string()})));
                        consecutive_failures += 1;
                    } else {
                        history.push(description.clone());
                        session.add_step(action_type, &description, "success", None);
                        // Record success step
                        if let Some(s) = step_to_record {
                            session_steps.push(s);
                        }
                        consecutive_failures = 0; // Reset on success
                    }
                }
                
                // [Session] Auto-save after each step
                let _ = crate::session_store::save_session(&session);
            } else {
                // For non-driver actions, just push history
                let status = action_status_override.unwrap_or("success");
                history.push(description.clone());
                session.add_step(action_type, &description, status, None);
                let _ = crate::session_store::save_session(&session);
                if status == "success" {
                    consecutive_failures = 0;
                } else {
                    consecutive_failures += 1;
                }
            }
            
            // SEND EVENT TO ANALYZER
            if let Some(tx) = &self.tx {
                let event = EventEnvelope {
                    schema_version: "1.0".to_string(),
                    event_id: Uuid::new_v4().to_string(),
                    ts: Utc::now().to_rfc3339(),
                    source: "dynamic_agent".to_string(),
                    app: "Agent".to_string(),
                    event_type: event_type.to_string(),
                    priority: "P1".to_string(),
                    resource: None,
                    payload: serde_json::json!({
                        "goal": goal,
                        "step": i,
                        "action": action_type,
                        "description": description,
                        "plan": plan
                    }),
                    privacy: None,
                    pid: None,
                    window_id: None,
                    window_title: None,
                    browser_url: None,
                    raw: None, 
                };
                
                if let Ok(json) = serde_json::to_string(&event) {
                    let _ = tx.try_send(json); 
                }
            }
            
            // COMBAT MODE CHECK
            if consecutive_failures >= 2 {
                println!("      ⚔️ CRITICAL: Consecutive failures detected ({})", consecutive_failures);
                println!("      🛡️ ENGAGING COMBAT PROTOCOL: Attempting to clear obstacles...");
                
                // Strategy 1: The "Escape" Hatch (Close Modals)
                println!("         👉 Strategy 1: Pressing ESC");
                let esc_script = "tell application \"System Events\" to key code 53"; // 53 is Esc
                let _ = std::process::Command::new("osascript").arg("-e").arg(esc_script).status();
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                // Strategy 2: The "Enter" Ram (Confirm Dialogs)
                println!("         👉 Strategy 2: Pressing ENTER");
                let enter_script = "tell application \"System Events\" to keystroke return";
                let _ = std::process::Command::new("osascript").arg("-e").arg(enter_script).status();
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                
                // Logic: After attempting blind fixes, we hope the next iteration sees a clear screen.
                // We reset the counter to give it another chance before spamming keys again.
                // But we keep it high enough next time? No, let's reset to 1.
                consecutive_failures = 1; 
                history.push("Combat Protocol executed: Esc + Enter".to_string());
            }
            
            // Adaptive Wait (Hyper-Speed)
            println!("      👁️ Monitoring UI stability...");
            if let Err(e) = VisualDriver::wait_for_ui_settle(3000).await {
                println!("      ⚠️ Adaptive wait error: {}", e);
            }
            // Fallback safety sleep (short)
            // tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        println!("🛑 Max steps reached.");
        let _ = recorder.stop(); // Stop recording
        Ok(())
    }
}
