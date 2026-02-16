use crate::llm_gateway::LLMClient;
use crate::visual_driver::{VisualDriver, UiAction, SmartStep};
use crate::policy::PolicyEngine;
use crate::schema::AgentAction;
use anyhow::Result;
use crate::db; 
use log::{info, warn};
use tokio::sync::mpsc;
use crate::schema::EventEnvelope;
use chrono::Utc;
use uuid::Uuid;
use crate::screen_recorder::ScreenRecorder;
#[cfg(target_os = "macos")]
use crate::applescript;
use std::sync::Arc;

pub struct DynamicController {
    llm: Arc<dyn LLMClient>,
    max_steps: usize,
    tx: Option<mpsc::Sender<String>>,
}

impl DynamicController {
    pub fn new(llm: Arc<dyn LLMClient>, tx: Option<mpsc::Sender<String>>) -> Self {
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
        let current_key = Self::extract_action_key(current_action);
        let mut match_count = 0;
        for entry in history.iter().rev().take(2) {
            if Self::extract_action_key(entry) == current_key {
                match_count += 1;
            }
        }
        match_count >= 2
    }
    
    /// Extract a simplified key from action for comparison
    fn extract_action_key(action_str: &str) -> String {
        if action_str.contains("\"action\":\"key\"") {
            if let Some(key_start) = action_str.find("\"key\":\"") {
                let rest = &action_str[key_start + 7..];
                if let Some(key_end) = rest.find('"') {
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
        recorder.cleanup_old_recordings();
        let _ = recorder.start();

        // [Reality] Scan available applications
        let _ = crate::reality_check::scan_app_inventory();

        let mut history: Vec<String> = Vec::new();
        let mut action_history: Vec<String> = Vec::new();
        let mut session_steps: Vec<SmartStep> = Vec::new();
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
            if buf.is_empty() { None } else { Some(buf.replace(',', "")) }
        }

        fn calculator_has_input(history: &[String]) -> bool {
            let mut seen_open = false;
            for entry in history.iter().rev() {
                if entry.contains("Opened app: Calculator") {
                    seen_open = true;
                    break;
                }
            }
            if !seen_open { return false; }
            for entry in history.iter().rev() {
                if entry.contains("Opened app: Calculator") { break; }
                if entry.starts_with("Typed '") { return true; }
            }
            false
        }

        fn goal_mentions_calculation(goal: &str) -> bool {
            let lower = goal.to_lowercase();
            lower.contains("계산") || lower.contains("calculate") || lower.contains("곱")
                || lower.contains("×") || lower.contains("*") || lower.contains("plus") || lower.contains("minus")
        }

        fn goal_is_ui_task(goal: &str) -> bool {
            let lower = goal.to_lowercase();
            let apps = ["safari", "notes", "finder", "preview", "textedit", "mail", "calculator",
                         "explorer", "notepad", "edge", "chrome"];
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
            
            // 2.5 Anti-Loop Detection
            let action_str = plan.to_string();
            if Self::detect_action_loop(&action_history, &action_str) {
                println!("   🔄 LOOP DETECTED: Same action {} times. Injecting alternative...", 3);
                if goal_mentions_desktop(goal) && goal_mentions_image(goal) {
                    plan = serde_json::json!({ "action": "open_desktop_image" });
                } else {
                    plan = serde_json::json!({
                        "action": "report",
                        "message": "I'm stuck repeating the same action. Let me try a different approach."
                    });
                }
            }
            action_history.push(action_str.clone());
            
            println!("   💡 Idea: {}", plan);

            // 2.6 Flatten Nested JSON
            if plan["action"].is_object() {
                println!("   🔧 Flattening nested JSON action...");
                plan = plan["action"].clone();
            }

            // 3. Act
            let action_type = plan["action"].as_str().unwrap_or("fail");
            let mut driver = VisualDriver::new();
            let mut description = format!("Step {}", i);
            let mut step_to_record: Option<SmartStep> = None;
            let _event_type = "action";
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
                        #[cfg(target_os = "macos")]
                        {
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
                        }
                        #[cfg(target_os = "windows")]
                        {
                            let _ = std::process::Command::new("powershell")
                                .args(["-NoProfile", "-Command",
                                    "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('{ESC}')"])
                                .status();
                            description = "Closed dialog via Escape key (Windows)".to_string();
                            action_status_override = Some("success");
                        }
                        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                        {
                            description = "Dialog close not supported on this platform".to_string();
                            action_status_override = Some("failed");
                            consecutive_failures += 1;
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
                            let mut cleaned = text.replace('×', "*").replace('x', "*").replace('X', "*").replace(' ', "");
                            if cleaned.chars().all(|c| c.is_ascii_digit()) {
                                if let Some(num) = last_read_number.as_ref() {
                                    if num.contains('.') { cleaned = num.clone(); }
                                }
                            }
                            if (cleaned.contains('*') || cleaned.contains('+') || cleaned.contains('-') || cleaned.contains('/'))
                                && !cleaned.ends_with('=')
                            { cleaned.push('='); }
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
                        #[cfg(target_os = "macos")]
                        { let _ = std::process::Command::new("osascript").arg("-e").arg("tell application \"System Events\" to key code 53").status(); }
                        #[cfg(target_os = "windows")]
                        { let _ = std::process::Command::new("powershell").args(["-NoProfile", "-Command", "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('{ESC}')"]).status(); }
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
                    let modifiers: Vec<String> = plan["modifiers"].as_array()
                        .map(|arr| arr.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect())
                        .unwrap_or_default();
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
                        if app_name.eq_ignore_ascii_case("Calculator") && goal_mentions_calculation(goal) && !calculator_has_input(&history) {
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
                        Err(e) => { description = format!("Failed to read: {}", e); }
                    }
                },
                "open_url" => {
                    let url = plan["url"].as_str().unwrap_or("https://google.com");
                    info!("      🌐 Opening URL: '{}'", url);
                    #[cfg(target_os = "macos")]
                    { match applescript::open_url(url) { Ok(_) => { description = format!("Opened URL '{}'", url); }, Err(e) => { description = format!("Failed: {}", e); } } }
                    #[cfg(target_os = "windows")]
                    { let _ = std::process::Command::new("cmd").args(["/C", "start", url]).status(); description = format!("Opened URL '{}'", url); }
                    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                    { let _ = std::process::Command::new("xdg-open").arg(url).status(); description = format!("Opened URL '{}'", url); }
                },
                "save_routine" => {
                    let name = plan["name"].as_str().unwrap_or("unnamed");
                    match serde_json::to_string(&session_steps) {
                        Ok(json) => { if let Err(e) = db::save_learned_routine(name, &json) { description = format!("Failed: {}", e); } else { description = format!("Saved Routine '{}'", name); } },
                        Err(e) => { description = format!("Failed: {}", e); }
                    }
                },
                "replay_routine" => {
                    let name = plan["name"].as_str().unwrap_or("unnamed");
                    match db::get_learned_routine(name) {
                        Ok(Some(routine)) => {
                            match serde_json::from_str::<Vec<SmartStep>>(&routine.steps_json) {
                                Ok(steps) => {
                                    let mut replay_driver = VisualDriver::new();
                                    for s in steps { replay_driver.add_step(s); }
                                    if let Err(e) = replay_driver.execute(Some(self.llm.as_ref())).await { description = format!("Replay failed: {}", e); }
                                    else { description = format!("Replayed Routine '{}'", name); }
                                },
                                Err(e) => { description = format!("Corrupt routine: {}", e); }
                            }
                        },
                        Ok(None) => { description = format!("Routine '{}' not found", name); },
                        Err(e) => { description = format!("DB Error: {}", e); }
                    }
                },
                "wait" => {
                    let secs = plan["seconds"].as_u64().unwrap_or(2);
                    let step = SmartStep::new(UiAction::Wait(secs), "Waiting");
                    driver.add_step(step.clone());
                    step_to_record = Some(step);
                    description = format!("Waited {}s", secs);
                },
                "done" => { println!("✅ Goal Achieved!"); let _ = recorder.stop(); return Ok(()); },
                "reply" => { let text = plan["text"].as_str().unwrap_or("..."); info!("🤖 Agent: {}", text); let _ = recorder.stop(); return Ok(()); },
                "fail" => { let reason = plan["reason"].as_str().unwrap_or("Unknown"); let _ = recorder.stop(); return Err(anyhow::anyhow!("Agent failed: {}", reason)); },
                "open_app" => {
                    let name = plan["name"].as_str().unwrap_or("Finder");
                    match crate::reality_check::verify_app_exists(name) {
                        Ok(canonical_name) => {
                            match crate::tool_chaining::CrossAppBridge::switch_to_app(&canonical_name) {
                                Ok(_) => {
                                    session_steps.push(SmartStep::new(UiAction::Type(canonical_name.clone()), "Open App"));
                                    description = format!("Opened app: {}", canonical_name);
                                    session.add_message("tool", &format!("open_app: {}", canonical_name));
                                }
                                Err(e) => { description = format!("Open app failed: {}", e); }
                            }
                        },
                        Err(e) => { description = format!("Failed: {}", e); }
                    }
                },
                "open_browser" => {
                    let url = plan["url"].as_str().unwrap_or("https://google.com");
                    #[cfg(target_os = "macos")]
                    { let _ = std::process::Command::new("open").arg(url).output(); }
                    #[cfg(target_os = "windows")]
                    { let _ = std::process::Command::new("cmd").args(["/C", "start", url]).output(); }
                    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                    { let _ = std::process::Command::new("xdg-open").arg(url).output(); }
                    session_steps.push(SmartStep::new(UiAction::Type(url.to_string()), "Open URL"));
                    description = format!("Opened URL '{}'", url);
                    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                },
                "read_file" => {
                    let path = plan["path"].as_str().unwrap_or("");
                    match crate::content_extractor::ContentExtractor::extract(path) {
                        Ok(content) => { description = format!("Read File '{}':\n{}", path, if content.len() > 500 { format!("{}...", &content[..500]) } else { content }); },
                        Err(e) => { description = format!("Failed to read '{}': {}", path, e); }
                    }
                },
                "shell" | "run_shell" => {
                    let cmd = plan["command"].as_str().unwrap_or("");
                    let level = crate::approval_gate::ApprovalGate::check_command(cmd);
                    match level {
                        crate::approval_gate::ApprovalLevel::Blocked => { description = format!("BLOCKED: {}", cmd); },
                        crate::approval_gate::ApprovalLevel::RequireApproval => { description = format!("Approval required: {}", cmd); },
                        crate::approval_gate::ApprovalLevel::AutoApprove => {
                            let config = crate::bash_executor::BashExecConfig { timeout_ms: 30000, approval_required: false, ..Default::default() };
                            match crate::bash_executor::execute_bash(cmd, &config) {
                                Ok(result) if result.success => {
                                    let preview = if result.stdout.len() > 500 { format!("{}...", &result.stdout[..500]) } else { result.stdout.clone() };
                                    description = format!("Shell '{}' -> {}", cmd, preview);
                                    session.add_message("tool", &format!("shell: {}\n{}", cmd, result.stdout));
                                },
                                Ok(result) => { description = format!("Shell failed: {}", result.stderr); },
                                Err(e) => { description = format!("Shell failed: {}", e); }
                            }
                        }
                    }
                },
                "spawn_agent" => {
                    let name = plan["name"].as_str().unwrap_or("worker");
                    let task = plan["task"].as_str().unwrap_or("");
                    let agent_id = crate::subagent::SubagentManager::spawn(name, task);
                    crate::subagent::SubagentManager::start(&agent_id);
                    description = format!("Spawned subagent '{}' (id: {})", name, agent_id);
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    crate::subagent::SubagentManager::complete(&agent_id, "Subtask simulated");
                },
                "snapshot" | "take_snapshot" => {
                    let mut browser = crate::browser_automation::get_browser_automation();
                    match browser.take_snapshot() {
                        Ok(refs) => { description = format!("Snapshot: {} UI elements captured", refs.len()); },
                        Err(e) => { description = format!("Snapshot failed: {}", e); }
                    }
                },
                "click_ref" => {
                    let ref_id = plan["ref"].as_str().unwrap_or("");
                    let double_click = plan["double_click"].as_bool().unwrap_or(false);
                    let browser = crate::browser_automation::get_browser_automation();
                    match browser.click_by_ref(ref_id, double_click) {
                        Ok(_) => { description = format!("Clicked ref '{}'", ref_id); },
                        Err(e) => { description = format!("Click ref failed: {}", e); }
                    }
                },
                "switch_app" | "activate_app" => {
                    let app_name = plan["app"].as_str().or(plan["name"].as_str()).unwrap_or("");
                    match crate::tool_chaining::CrossAppBridge::switch_to_app(app_name) {
                        Ok(_) => { description = format!("Switched to app: {}", app_name); session.add_message("tool", &format!("switch_app: {}", app_name)); }
                        Err(e) => { description = format!("Failed to switch to {}: {}", app_name, e); }
                    }
                },
                "copy" | "copy_to_clipboard" => {
                    let text = plan["text"].as_str().or(plan["content"].as_str()).unwrap_or("");
                    match crate::tool_chaining::CrossAppBridge::copy_to_clipboard(text) {
                        Ok(_) => { description = format!("Copied {} chars", text.len()); }
                        Err(e) => { description = format!("Copy failed: {}", e); }
                    }
                },
                "paste" | "paste_clipboard" => {
                    match crate::tool_chaining::CrossAppBridge::paste() {
                        Ok(_) => { description = "Pasted from clipboard".to_string(); }
                        Err(e) => { description = format!("Paste failed: {}", e); }
                    }
                },
                "read_clipboard" | "get_clipboard" => {
                    match crate::tool_chaining::CrossAppBridge::get_clipboard() {
                        Ok(content) => { description = format!("Clipboard: {}", if content.len() > 100 { format!("{}...", &content[..100]) } else { content.clone() }); session.add_message("tool", &format!("clipboard: {}", content)); }
                        Err(e) => { description = format!("Read clipboard failed: {}", e); }
                    }
                },
                "transfer" | "copy_between_apps" => {
                    let from_app = plan["from"].as_str().unwrap_or("");
                    let to_app = plan["to"].as_str().unwrap_or("");
                    if !from_app.is_empty() { let _ = crate::tool_chaining::CrossAppBridge::switch_to_app(from_app); tokio::time::sleep(tokio::time::Duration::from_millis(500)).await; }
                    #[cfg(target_os = "macos")]
                    { let _ = std::process::Command::new("osascript").arg("-e").arg("tell application \"System Events\"\nkeystroke \"a\" using command down\ndelay 0.2\nkeystroke \"c\" using command down\nend tell").status(); }
                    #[cfg(target_os = "windows")]
                    { let _ = std::process::Command::new("powershell").args(["-NoProfile", "-Command", "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('^a'); Start-Sleep -Milliseconds 200; [System.Windows.Forms.SendKeys]::SendWait('^c')"]).status(); }
                    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                    if !to_app.is_empty() { let _ = crate::tool_chaining::CrossAppBridge::switch_to_app(to_app); tokio::time::sleep(tokio::time::Duration::from_millis(500)).await; }
                    let _ = crate::tool_chaining::CrossAppBridge::paste();
                    description = format!("Transferred data from {} to {}", from_app, to_app);
                    session.add_message("tool", &format!("transfer: {} -> {}", from_app, to_app));
                },
                "mcp" | "mcp_call" | "external_tool" => {
                    let server = plan["server"].as_str().unwrap_or("filesystem");
                    let tool = plan["tool"].as_str().unwrap_or("");
                    let args = plan["arguments"].clone();
                    if server == "filesystem" && goal_is_ui_task(goal) {
                        description = "BLOCKED: MCP filesystem usage disallowed for UI tasks".to_string();
                        consecutive_failures += 1;
                        history.push(description.clone());
                        continue;
                    }
                    let _ = crate::mcp_client::init_mcp();
                    match crate::mcp_client::call_mcp_tool(server, tool, args) {
                        Ok(result) => {
                            let result_str = serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
                            description = format!("MCP {}/{}: {}", server, tool, if result_str.len() > 300 { format!("{}...", &result_str[..300]) } else { result_str.clone() });
                            session.add_message("tool", &format!("mcp: {}/{}\n{}", server, tool, result_str));
                        }
                        Err(e) => { description = format!("MCP failed: {}", e); }
                    }
                },
                "mcp_list" | "list_mcp_tools" => {
                    let _ = crate::mcp_client::init_mcp();
                    match crate::mcp_client::get_mcp_registry() {
                        Ok(guard) => {
                            if let Some(registry) = guard.as_ref() {
                                let tools = registry.list_all_tools();
                                let tools_str: Vec<String> = tools.iter().map(|(s, t)| format!("{}/{}: {}", s, t.name, t.description)).collect();
                                description = format!("MCP Tools: {}", tools_str.join(", "));
                            } else { description = "MCP not initialized".to_string(); }
                        }
                        Err(e) => { description = format!("MCP list failed: {}", e); }
                    }
                },
                "select_text" => {
                    let text = plan["text"].as_str().unwrap_or("");
                    #[cfg(target_os = "macos")]
                    {
                        let safe_text = text.replace('\\', "\\\\").replace('"', "\\\"");
                        let script = format!("tell application \"System Events\"\nkeystroke \"f\" using command down\ndelay 0.2\nkeystroke \"{}\"\ndelay 0.2\nkey code 36\nend tell", safe_text);
                        match applescript::run(&script) { Ok(_) => { description = format!("Selected text '{}'", text); }, Err(_) => { description = format!("Select failed: '{}'", text); action_status_override = Some("failed"); consecutive_failures += 1; } }
                    }
                    #[cfg(target_os = "windows")]
                    {
                        let _ = std::process::Command::new("powershell").args(["-NoProfile", "-Command", &format!("Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('^f'); Start-Sleep -Milliseconds 200; [System.Windows.Forms.SendKeys]::SendWait('{}'); [System.Windows.Forms.SendKeys]::SendWait('{{ENTER}}')", text)]).status();
                        description = format!("Selected text '{}'", text);
                    }
                    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                    { description = "Select text not supported".to_string(); action_status_override = Some("failed"); }
                },
                "open_desktop_image" => {
                    #[cfg(target_os = "macos")]
                    {
                        match applescript::run(r#"tell application "Finder" to open item 1 of (files of (path to desktop folder) whose name extension is in {"png","jpg","jpeg"})"#) {
                            Ok(_) => { description = "Opened desktop image".to_string(); action_status_override = Some("success"); }
                            Err(e) => { description = format!("Failed: {}", e); action_status_override = Some("failed"); consecutive_failures += 1; }
                        }
                    }
                    #[cfg(target_os = "windows")]
                    {
                        let desktop = dirs::desktop_dir().unwrap_or_else(|| std::path::PathBuf::from("C:\\Users\\Public\\Desktop"));
                        let mut found = false;
                        if let Ok(entries) = std::fs::read_dir(&desktop) {
                            for entry in entries.flatten() {
                                let path = entry.path();
                                if let Some(ext) = path.extension() {
                                    let ext = ext.to_string_lossy().to_lowercase();
                                    if ext == "png" || ext == "jpg" || ext == "jpeg" {
                                        let _ = std::process::Command::new("cmd").args(["/C", "start", &path.to_string_lossy()]).status();
                                        description = format!("Opened: {}", path.display()); action_status_override = Some("success"); found = true; break;
                                    }
                                }
                            }
                        }
                        if !found { description = "No desktop images found".to_string(); action_status_override = Some("failed"); consecutive_failures += 1; }
                    }
                    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                    { description = "Not supported".to_string(); action_status_override = Some("failed"); }
                },
                _ => {
                    if action_type.starts_with("filesystem/") { description = format!("BLOCKED: {}", action_type); action_status_override = Some("blocked"); consecutive_failures += 1; }
                    else { warn!("⚠️ Unknown action: {}", action_type); }
                }
            }

            // Execute the single step
            if !driver.steps.is_empty() {
                let mut policy = PolicyEngine::new();
                policy.unlock();
                let action_for_policy = AgentAction::UiClick { element_id: description.clone(), double_click: false };
                if policy.check(&action_for_policy).is_err() {
                    history.push(format!("BLOCKED_BY_POLICY: {}", description));
                    session.add_step(action_type, &description, "blocked", None);
                    consecutive_failures += 1;
                } else {
                    if let Err(e) = driver.execute(Some(self.llm.as_ref())).await {
                        history.push(format!("FAILED: {}", description));
                        session.add_step(action_type, &description, "failed", Some(serde_json::json!({"error": e.to_string()})));
                        consecutive_failures += 1;
                    } else {
                        history.push(description.clone());
                        session.add_step(action_type, &description, "success", None);
                        if let Some(s) = step_to_record { session_steps.push(s); }
                        consecutive_failures = 0;
                    }
                }
                let _ = crate::session_store::save_session(&session);
            } else {
                let status = action_status_override.unwrap_or("success");
                history.push(description.clone());
                session.add_step(action_type, &description, status, None);
                let _ = crate::session_store::save_session(&session);
                if status == "success" { consecutive_failures = 0; } else { consecutive_failures += 1; }
            }
            
            // SEND EVENT TO ANALYZER
            if let Some(tx) = &self.tx {
                let event = EventEnvelope {
                    schema_version: "1.0".to_string(),
                    event_id: Uuid::new_v4().to_string(),
                    ts: Utc::now().to_rfc3339(),
                    source: "dynamic_agent".to_string(),
                    app: "Agent".to_string(),
                    event_type: _event_type.to_string(),
                    priority: "P1".to_string(),
                    resource: None,
                    payload: serde_json::json!({ "goal": goal, "step": i, "action": action_type, "description": description, "plan": plan }),
                    privacy: None, pid: None, window_id: None, window_title: None, browser_url: None, raw: None,
                };
                if let Ok(json) = serde_json::to_string(&event) { let _ = tx.try_send(json); }
            }
            
            // COMBAT MODE CHECK
            if consecutive_failures >= 2 {
                println!("      ⚔️ COMBAT PROTOCOL: {} failures", consecutive_failures);
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("osascript").arg("-e").arg("tell application \"System Events\" to key code 53").status();
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    let _ = std::process::Command::new("osascript").arg("-e").arg("tell application \"System Events\" to keystroke return").status();
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("powershell").args(["-NoProfile", "-Command", "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('{ESC}')"]).status();
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    let _ = std::process::Command::new("powershell").args(["-NoProfile", "-Command", "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')"]).status();
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
                consecutive_failures = 1;
                history.push("Combat Protocol executed: Esc + Enter".to_string());
            }
            
            // Adaptive Wait
            if let Err(e) = VisualDriver::wait_for_ui_settle(3000).await {
                println!("      ⚠️ Adaptive wait error: {}", e);
            }
        }

        println!("🛑 Max steps reached.");
        let _ = recorder.stop();
        Ok(())
    }
}
