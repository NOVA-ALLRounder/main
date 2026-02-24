//! Tool Chaining Engine - The REAL missing piece
//!
//! This enables complex multi-step scenarios like:
//! "Check calendar, then send summary to Slack"
//!
//! Core concept: Each tool outputs data that can be consumed by the next tool

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use std::time::{Duration, Instant};
#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{HANDLE, HGLOBAL, HWND},
    System::{
        DataExchange::{
            CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
        },
        Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE, GMEM_ZEROINIT},
        Ole::CF_UNICODETEXT,
    },
    UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL},
};

// =====================================================
// TOOL RESULT CONTEXT
// =====================================================

/// Result from a tool execution that can be passed to next tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_name: String,
    pub success: bool,
    pub output: Value,
    pub extracted_data: HashMap<String, String>, // Key data points
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ToolResult {
    pub fn success(tool_name: &str, output: Value) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            success: true,
            output,
            extracted_data: HashMap::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn failed(tool_name: &str, error: &str) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            success: false,
            output: serde_json::json!({"error": error}),
            extracted_data: HashMap::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Add extracted data point
    pub fn with_data(mut self, key: &str, value: &str) -> Self {
        self.extracted_data
            .insert(key.to_string(), value.to_string());
        self
    }

    /// Get value for use in next tool
    pub fn get(&self, key: &str) -> Option<&String> {
        self.extracted_data.get(key)
    }
}

// =====================================================
// EXECUTION CONTEXT (Cross-tool state)
// =====================================================

/// Shared context across tool chain execution
#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    pub variables: HashMap<String, String>,
    pub tool_results: Vec<ToolResult>,
    pub clipboard: Option<String>,
    pub current_app: Option<String>,
    pub last_screenshot: Option<String>,
}

impl ExecutionContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a variable for later use
    pub fn set(&mut self, key: &str, value: &str) {
        self.variables.insert(key.to_string(), value.to_string());
    }

    /// Get a variable
    pub fn get(&self, key: &str) -> Option<&String> {
        self.variables.get(key)
    }

    /// Get last tool result
    pub fn last_result(&self) -> Option<&ToolResult> {
        self.tool_results.last()
    }

    /// Add tool result
    pub fn add_result(&mut self, result: ToolResult) {
        // Also copy extracted data to context variables
        for (k, v) in &result.extracted_data {
            self.variables.insert(k.clone(), v.clone());
        }
        self.tool_results.push(result);
    }

    /// Template substitution - replace {{var}} with actual values
    pub fn substitute(&self, template: &str) -> String {
        let mut result = template.to_string();
        for (key, value) in &self.variables {
            result = result.replace(&format!("{{{{{}}}}}", key), value);
        }
        // Also support $var syntax
        for (key, value) in &self.variables {
            result = result.replace(&format!("${}", key), value);
        }
        result
    }
}

// =====================================================
// TOOL CHAIN - Multi-step workflow
// =====================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStep {
    pub name: String,
    pub tool: String,
    pub params: HashMap<String, String>,
    #[serde(default)]
    pub extract: HashMap<String, String>, // JSONPath-like extraction rules
    #[serde(default)]
    pub on_fail: FailAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FailAction {
    #[default]
    Stop,
    Continue,
    Retry,
    Skip,
}

#[derive(Debug, Clone)]
pub struct ToolChain {
    pub name: String,
    pub steps: Vec<ToolStep>,
    pub context: ExecutionContext,
}

impl ToolChain {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            steps: Vec::new(),
            context: ExecutionContext::new(),
        }
    }

    /// Add a step to the chain
    pub fn add_step(&mut self, step: ToolStep) {
        self.steps.push(step);
    }

    /// Build a chain from JSON definition
    pub fn from_json(json: &Value) -> Result<Self> {
        let name = json["name"].as_str().unwrap_or("unnamed");
        let mut chain = Self::new(name);

        if let Some(steps) = json["steps"].as_array() {
            for step_json in steps {
                let step: ToolStep = serde_json::from_value(step_json.clone())?;
                chain.add_step(step);
            }
        }

        Ok(chain)
    }
}

// =====================================================
// CROSS-APP BRIDGE
// =====================================================

/// Bridge for passing data between applications
pub struct CrossAppBridge;

#[cfg(target_os = "windows")]
struct ClipboardCloseGuard;

#[cfg(target_os = "windows")]
impl Drop for ClipboardCloseGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

#[cfg(target_os = "windows")]
fn open_clipboard_with_retry() -> Result<ClipboardCloseGuard> {
    let mut last_err: Option<anyhow::Error> = None;
    for _ in 0..10 {
        let opened = unsafe { OpenClipboard(HWND(std::ptr::null_mut())) };
        match opened {
            Ok(()) => return Ok(ClipboardCloseGuard),
            Err(e) => {
                last_err = Some(anyhow::anyhow!(e.to_string()));
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Failed to open clipboard")))
}

#[cfg(target_os = "windows")]
fn set_clipboard_text_windows(text: &str) -> Result<()> {
    let _guard = open_clipboard_with_retry()?;
    unsafe {
        EmptyClipboard()?;
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes_len = wide
            .len()
            .checked_mul(std::mem::size_of::<u16>())
            .ok_or_else(|| anyhow::anyhow!("Clipboard text size overflow"))?;

        let hmem = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes_len)?;
        let ptr = GlobalLock(hmem) as *mut u8;
        if ptr.is_null() {
            return Err(anyhow::anyhow!("GlobalLock failed for clipboard buffer"));
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr, bytes_len);
        let _ = GlobalUnlock(hmem);
        let _ = SetClipboardData(CF_UNICODETEXT.0 as u32, HANDLE(hmem.0))?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn get_clipboard_text_windows() -> Result<String> {
    let _guard = open_clipboard_with_retry()?;
    unsafe {
        let handle = GetClipboardData(CF_UNICODETEXT.0 as u32)?;
        let hglobal = HGLOBAL(handle.0);
        let ptr = GlobalLock(hglobal) as *const u16;
        if ptr.is_null() {
            return Ok(String::new());
        }
        let bytes = GlobalSize(hglobal);
        if bytes == 0 {
            let _ = GlobalUnlock(hglobal);
            return Ok(String::new());
        }

        let max_u16 = bytes / std::mem::size_of::<u16>();
        let slice = std::slice::from_raw_parts(ptr, max_u16);
        let len = slice.iter().position(|&c| c == 0).unwrap_or(max_u16);
        let text = String::from_utf16_lossy(&slice[..len]);
        let _ = GlobalUnlock(hglobal);
        Ok(text)
    }
}

#[cfg(target_os = "windows")]
fn send_ctrl_shortcut_windows(letter: char) -> Result<()> {
    let key = letter.to_ascii_uppercase();
    if !key.is_ascii_alphanumeric() {
        return Err(anyhow::anyhow!("Unsupported shortcut key '{}'", letter));
    }
    unsafe {
        let mut inputs = [INPUT::default(); 4];
        let vk = VIRTUAL_KEY(key as u16);

        // Ctrl down
        inputs[0].r#type = INPUT_KEYBOARD;
        inputs[0].Anonymous.ki.wVk = VK_CONTROL;

        // Key down
        inputs[1].r#type = INPUT_KEYBOARD;
        inputs[1].Anonymous.ki.wVk = vk;

        // Key up
        inputs[2].r#type = INPUT_KEYBOARD;
        inputs[2].Anonymous.ki.wVk = vk;
        inputs[2].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;

        // Ctrl up
        inputs[3].r#type = INPUT_KEYBOARD;
        inputs[3].Anonymous.ki.wVk = VK_CONTROL;
        inputs[3].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;

        let _ = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
    Ok(())
}

impl CrossAppBridge {
    #[cfg(target_os = "windows")]
    fn focus_or_launch_windows_app(app_name: &str) -> bool {
        let lower = app_name.trim().to_ascii_lowercase();

        let mut candidates: Vec<String> = Vec::new();
        if lower.contains("chrome") {
            candidates.extend([
                "chrome".to_string(),
                "chrome.exe".to_string(),
                "Google Chrome".to_string(),
            ]);
        } else if lower.contains("edge") || lower.contains("safari") {
            candidates.extend([
                "msedge".to_string(),
                "msedge.exe".to_string(),
                "Microsoft Edge".to_string(),
            ]);
        } else if lower.contains("firefox") {
            candidates.extend([
                "firefox".to_string(),
                "firefox.exe".to_string(),
                "Firefox".to_string(),
            ]);
        } else if lower.contains("mail") || lower.contains("outlook") {
            candidates.extend([
                "outlook".to_string(),
                "outlook.exe".to_string(),
                "Microsoft Outlook".to_string(),
            ]);
        } else if lower.contains("notes") || lower.contains("textedit") || lower.contains("notepad") {
            candidates.extend([
                "notepad".to_string(),
                "notepad.exe".to_string(),
            ]);
        } else if lower.contains("finder") || lower.contains("explorer") {
            candidates.extend([
                "explorer".to_string(),
                "explorer.exe".to_string(),
                "File Explorer".to_string(),
            ]);
        } else if lower.contains("calculator") || lower.contains("calc") {
            candidates.extend([
                "calculator".to_string(),
                "calc".to_string(),
                "calc.exe".to_string(),
            ]);
        }
        if !app_name.trim().is_empty() {
            candidates.push(app_name.trim().to_string());
        }

        let mut seen = std::collections::HashSet::<String>::new();
        candidates.retain(|c| seen.insert(c.to_ascii_lowercase()));

        for candidate in &candidates {
            if crate::win32_app_control::system_events::set_frontmost_by_name(candidate).is_ok() {
                return true;
            }
        }

        let launch_target = if lower.contains("mail") || lower.contains("outlook") {
            "outlook"
        } else if lower.contains("notes") || lower.contains("textedit") || lower.contains("notepad")
        {
            "notepad"
        } else if lower.contains("finder") || lower.contains("explorer") {
            "explorer"
        } else if lower.contains("calculator") || lower.contains("calc") {
            "calc"
        } else if lower.contains("edge") || lower.contains("safari") {
            "msedge"
        } else {
            app_name.trim()
        };

        std::process::Command::new("cmd")
            .args(["/C", "start", "", launch_target])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
    }

    /// Copy text to system clipboard
    pub fn copy_to_clipboard(text: &str) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            set_clipboard_text_windows(text)?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let script = format!(
                r#"set the clipboard to "{}""#,
                text.replace("\"", "\\\"").replace("\n", "\\n")
            );

            crate::applescript::run(&script)?;
        }

        println!(
            "📋 [Bridge] Copied to clipboard: {}...",
            &text[..text.len().min(50)]
        );
        Ok(())
    }

    /// Get text from system clipboard
    pub fn get_clipboard() -> Result<String> {
        #[cfg(target_os = "windows")]
        {
            get_clipboard_text_windows()
        }
        #[cfg(not(target_os = "windows"))]
        {
            crate::applescript::run("the clipboard").map(|s| s.trim().to_string())
        }
    }

    /// Paste from clipboard (Cmd+V)
    pub fn paste() -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            send_ctrl_shortcut_windows('v')?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let script = r#"tell application "System Events" to keystroke "v" using command down"#;
            crate::applescript::run(script)?;
        }
        Ok(())
    }

    /// Switch to application (Modified to use 'open -a' for reliability)
    pub fn switch_to_app(app_name: &str) -> Result<()> {
        println!("      🚀 [Bridge] Opening '{}' via CLI...", app_name);

        // Prefer Peekaboo app launch if available (stronger focus + permissions)
        let peekaboo_enabled = std::env::var("STEER_ENABLE_PEEKABOO_LAUNCH")
            .ok()
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false);
        if peekaboo_enabled && crate::peekaboo_cli::is_available() {
            let timeout_ms = std::env::var("STEER_PEEKABOO_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(4000);

            let launch = std::process::Command::new("peekaboo")
                .arg("app")
                .arg("launch")
                .arg(app_name)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();

            if let Ok(mut child) = launch {
                let deadline = Instant::now() + Duration::from_millis(timeout_ms);
                let mut launch_ok = false;
                loop {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            launch_ok = status.success();
                            break;
                        }
                        Ok(None) => {
                            if Instant::now() >= deadline {
                                let _ = child.kill();
                                let _ = child.wait();
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(80));
                        }
                        Err(_) => break,
                    }
                }

                if launch_ok && Self::wait_for_frontmost(app_name, 8, 200) {
                    println!("🔀 [Bridge] Switched to: {}", app_name);
                    return Ok(());
                }
            }
        }

        #[cfg(target_os = "windows")]
        let open_ok = Self::focus_or_launch_windows_app(app_name);
        #[cfg(not(target_os = "windows"))]
        let open_ok = {
            let open_timeout_ms = std::env::var("STEER_OPEN_APP_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(5000);
            let mut open_ok = false;
            let launch = std::process::Command::new("open")
                .arg("-a")
                .arg(app_name)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();

            if let Ok(mut child) = launch {
                let deadline = Instant::now() + Duration::from_millis(open_timeout_ms);
                loop {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            open_ok = status.success();
                            break;
                        }
                        Ok(None) => {
                            if Instant::now() >= deadline {
                                let _ = child.kill();
                                let _ = child.wait();
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(80));
                        }
                        Err(_) => break,
                    }
                }
            }
            open_ok
        };
        if !open_ok {
            println!(
                "      ⚠️ [Bridge] 'open -a {}' timeout/failure; falling back to AppleScript activation.",
                app_name
            );
        }

        if !Self::wait_for_frontmost(app_name, 8, 200) {
            let _ = crate::applescript::activate_app(app_name);
        }

        println!("🔀 [Bridge] Switched to: {}", app_name);
        Ok(())
    }

    fn wait_for_frontmost(app_name: &str, retries: usize, wait_ms: u64) -> bool {
        for _ in 0..retries {
            if let Ok(front) = Self::get_frontmost_app() {
                if crate::applescript::app_names_match(&front, app_name) {
                    return true;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(wait_ms));
        }
        false
    }

    /// Get frontmost application name
    pub fn get_frontmost_app() -> Result<String> {
        crate::applescript::frontmost_app_name()
    }

    /// Read selected text from current app
    pub fn get_selected_text() -> Result<Option<String>> {
        // Try Cmd+C and read clipboard
        let old_clipboard = Self::get_clipboard().ok();

        #[cfg(target_os = "windows")]
        {
            send_ctrl_shortcut_windows('c')?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let script = r#"tell application "System Events" to keystroke "c" using command down"#;
            crate::applescript::run(script)?;
        }

        std::thread::sleep(std::time::Duration::from_millis(100));

        let new_clipboard = Self::get_clipboard()?;

        // Restore clipboard if changed
        if Some(&new_clipboard) != old_clipboard.as_ref() {
            Ok(Some(new_clipboard))
        } else {
            Ok(None)
        }
    }

    /// Write to a temporary file and return path
    pub fn write_temp_file(content: &str, extension: &str) -> Result<String> {
        let path = std::env::temp_dir().join(format!(
            "steer_bridge_{}.{}",
            uuid::Uuid::new_v4(),
            extension
        ));
        std::fs::write(&path, content)?;
        let path_str = path.display().to_string();
        println!("📁 [Bridge] Wrote temp file: {}", path_str);
        Ok(path_str)
    }
}

// =====================================================
// HIGH-LEVEL ORCHESTRATION
// =====================================================

/// Execute a complex multi-app scenario
pub async fn execute_scenario(description: &str) -> Result<ExecutionContext> {
    let mut ctx = ExecutionContext::new();

    println!("🎯 [Scenario] Starting: {}", description);

    // Parse the scenario description to identify apps and actions
    // This is where LLM would normally decompose the task

    // For now, set up the context
    ctx.set("scenario", description);
    ctx.current_app = CrossAppBridge::get_frontmost_app().ok();

    Ok(ctx)
}

// =====================================================
// COMMON TOOL IMPLEMENTATIONS
// =====================================================

/// Read content from an app (generic)
pub fn read_from_app(app_name: &str, ctx: &mut ExecutionContext) -> Result<ToolResult> {
    CrossAppBridge::switch_to_app(app_name)?;
    std::thread::sleep(std::time::Duration::from_millis(300));

    // Try to select all and copy
    #[cfg(target_os = "windows")]
    {
        send_ctrl_shortcut_windows('a')?;
        std::thread::sleep(std::time::Duration::from_millis(100));
        send_ctrl_shortcut_windows('c')?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let script = r#"tell application "System Events"
        keystroke "a" using command down
        delay 0.1
        keystroke "c" using command down
    end tell"#;

        crate::applescript::run(script)?;
    }

    std::thread::sleep(std::time::Duration::from_millis(200));

    let content = CrossAppBridge::get_clipboard()?;
    ctx.set("last_read", &content);

    Ok(ToolResult::success(
        "read_from_app",
        serde_json::json!({
            "app": app_name,
            "content": content
        }),
    )
    .with_data("content", &content))
}

/// Write content to an app
pub fn write_to_app(
    app_name: &str,
    content: &str,
    ctx: &mut ExecutionContext,
) -> Result<ToolResult> {
    // Substitute variables in content
    let resolved_content = ctx.substitute(content);

    CrossAppBridge::switch_to_app(app_name)?;
    std::thread::sleep(std::time::Duration::from_millis(300));

    CrossAppBridge::copy_to_clipboard(&resolved_content)?;
    CrossAppBridge::paste()?;

    Ok(ToolResult::success(
        "write_to_app",
        serde_json::json!({
            "app": app_name,
            "written": resolved_content.len()
        }),
    ))
}

/// Calculate expression
pub fn calculate(expression: &str, ctx: &mut ExecutionContext) -> Result<ToolResult> {
    #[cfg(target_os = "windows")]
    let output = std::process::Command::new("python")
        .arg("-c")
        .arg(format!("print({})", expression))
        .output()?;

    #[cfg(not(target_os = "windows"))]
    let output = std::process::Command::new("python3")
        .arg("-c")
        .arg(format!("print({})", expression))
        .output()?;

    let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
    ctx.set("calc_result", &result);

    Ok(ToolResult::success(
        "calculate",
        serde_json::json!({
            "expression": expression,
            "result": result
        }),
    )
    .with_data("result", &result))
}

// =====================================================
// TESTS
// =====================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_substitution() {
        let mut ctx = ExecutionContext::new();
        ctx.set("name", "John");
        ctx.set("value", "123");

        assert_eq!(ctx.substitute("Hello {{name}}!"), "Hello John!");
        assert_eq!(ctx.substitute("Value is $value"), "Value is 123");
    }

    #[test]
    fn test_tool_result_chaining() {
        let mut ctx = ExecutionContext::new();

        let result1 = ToolResult::success("calc", serde_json::json!({"result": 579}))
            .with_data("calc_result", "579");

        ctx.add_result(result1);

        // Now the result should be available as a variable
        assert_eq!(ctx.get("calc_result"), Some(&"579".to_string()));
    }
}
