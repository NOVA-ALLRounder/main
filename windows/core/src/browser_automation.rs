// Browser Automation Module - Ported from clawdbot-main/src/browser/pw-tools-core.interactions.ts
// Provides stable element references and Playwright-style automation

use crate::peekaboo_cli;
use crate::tool_chaining::CrossAppBridge;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "windows")]
use serde_json::Value;
use std::collections::HashMap;
use std::process::Command;

// =====================================================
// Element Reference System (clawdbot pattern)
// =====================================================

/// Element reference from accessibility tree snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementRef {
    pub id: String,   // e.g., "E123"
    pub role: String, // e.g., "button", "textbox"
    pub name: String, // Accessible name
    pub bounds: Option<Bounds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Bounds {
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapshotSource {
    AppleScript,
    Peekaboo,
}

/// Browser automation context
pub struct BrowserAutomation {
    /// Cache of element references from last snapshot
    element_refs: HashMap<String, ElementRef>,
    /// Counter for generating unique element IDs
    ref_counter: u32,
    /// Ordered refs from last snapshot
    last_snapshot_refs: Vec<ElementRef>,
    /// Snapshot source for last capture
    last_snapshot_source: SnapshotSource,
    /// Snapshot id when using Peekaboo
    last_snapshot_id: Option<String>,
}

impl BrowserAutomation {
    pub fn new() -> Self {
        Self {
            element_refs: HashMap::new(),
            ref_counter: 0,
            last_snapshot_refs: Vec::new(),
            last_snapshot_source: SnapshotSource::AppleScript,
            last_snapshot_id: None,
        }
    }

    fn env_bool_with_default(key: &str, default: bool) -> bool {
        std::env::var(key)
            .ok()
            .map(|v| {
                matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(default)
    }

    fn env_usize_bounded(key: &str, default: usize, min: usize, max: usize) -> usize {
        std::env::var(key)
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .map(|v| v.clamp(min, max))
            .unwrap_or(default)
    }

    fn env_u64_bounded(key: &str, default: u64, min: u64, max: u64) -> u64 {
        std::env::var(key)
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(|v| v.clamp(min, max))
            .unwrap_or(default)
    }

    fn snapshot_focus_recovery_enabled() -> bool {
        Self::env_bool_with_default("STEER_BROWSER_SNAPSHOT_FOCUS_RECOVERY", true)
    }

    fn snapshot_retry_count() -> usize {
        Self::env_usize_bounded("STEER_BROWSER_SNAPSHOT_RETRIES", 1, 0, 8)
    }

    fn snapshot_retry_ms() -> u64 {
        Self::env_u64_bounded("STEER_BROWSER_SNAPSHOT_RETRY_MS", 90, 20, 2000)
    }

    fn snapshot_perf_warn_ms() -> u64 {
        Self::env_u64_bounded("STEER_BROWSER_SNAPSHOT_WARN_MS", 850, 100, 10_000)
    }

    fn peekaboo_fallback_enabled() -> bool {
        let default_enabled = !cfg!(target_os = "windows");
        Self::env_bool_with_default("STEER_BROWSER_PEEKABOO_FALLBACK", default_enabled)
    }

    fn snapshot_recovery_apps() -> Vec<String> {
        let default_apps = if cfg!(target_os = "windows") {
            "Google Chrome,Microsoft Edge,Firefox"
        } else {
            "Safari,Google Chrome,Arc"
        };

        let raw = std::env::var("STEER_BROWSER_SNAPSHOT_RECOVERY_APPS")
            .unwrap_or_else(|_| default_apps.to_string());
        raw.split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }

    fn activate_app(app_name: &str) {
        #[cfg(target_os = "windows")]
        {
            let app_lower = app_name.to_ascii_lowercase();
            let mut candidates: Vec<&str> = Vec::new();

            if app_lower.contains("chrome") {
                candidates.extend(["chrome", "chrome.exe", "Google Chrome"]);
            } else if app_lower.contains("edge") {
                candidates.extend(["msedge", "msedge.exe", "Microsoft Edge"]);
            } else if app_lower.contains("firefox") {
                candidates.extend(["firefox", "firefox.exe", "Firefox"]);
            } else if app_lower.contains("arc") {
                candidates.extend(["arc", "arc.exe", "Arc"]);
            } else {
                candidates.push(app_name);
            }

            for candidate in candidates {
                if crate::win32_app_control::system_events::set_frontmost_by_name(candidate)
                    .is_ok()
                {
                    return;
                }
            }

            let _ = Command::new("cmd").args(["/C", "start", "", app_name]).spawn();
        }

        #[cfg(not(target_os = "windows"))]
        {
            let escaped = app_name.replace('\\', "\\\\").replace('"', "\\\"");
            let script = format!("tell application \"{}\" to activate", escaped);
            let _ = Command::new("osascript").arg("-e").arg(script).output();
        }
    }

    fn recover_snapshot_focus(attempt: usize) {
        let apps = Self::snapshot_recovery_apps();
        if apps.is_empty() {
            return;
        }
        let front = CrossAppBridge::get_frontmost_app().unwrap_or_default();
        let front_already_browser = apps.iter().any(|a| a.eq_ignore_ascii_case(front.trim()));
        if front_already_browser {
            return;
        }
        let idx = if attempt == 0 {
            0
        } else {
            (attempt - 1) % apps.len()
        };
        if let Some(target) = apps.get(idx) {
            Self::activate_app(target);
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn parse_snapshot_stdout(&mut self, stdout: &str) -> Vec<ElementRef> {
        let mut refs = Vec::new();
        self.element_refs.clear();
        self.ref_counter = 0;
        self.last_snapshot_refs.clear();
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() < 6 {
                continue;
            }
            self.ref_counter += 1;
            let ref_id = format!("E{}", self.ref_counter);
            let elem_ref = ElementRef {
                id: ref_id.clone(),
                role: parts[0].to_string(),
                name: parts[1].to_string(),
                bounds: Some(Bounds {
                    x: parts[2].parse().unwrap_or(0),
                    y: parts[3].parse().unwrap_or(0),
                    width: parts[4].parse().unwrap_or(0),
                    height: parts[5].parse().unwrap_or(0),
                }),
            };
            self.element_refs.insert(ref_id, elem_ref.clone());
            refs.push(elem_ref);
        }
        refs
    }

    #[cfg(target_os = "windows")]
    fn parse_uia_snapshot_value(&mut self, snapshot: &Value) -> Vec<ElementRef> {
        let mut refs = Vec::new();
        self.element_refs.clear();
        self.ref_counter = 0;
        self.last_snapshot_refs.clear();

        if let Some(root) = snapshot.get("focused_window") {
            self.collect_uia_refs(root, &mut refs);
        } else {
            self.collect_uia_refs(snapshot, &mut refs);
        }

        refs
    }

    #[cfg(target_os = "windows")]
    fn collect_uia_refs(&mut self, node: &Value, refs: &mut Vec<ElementRef>) {
        let role = node
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("Unknown")
            .trim()
            .to_string();

        let name = node
            .get("title")
            .and_then(Value::as_str)
            .filter(|v| !v.trim().is_empty())
            .or_else(|| {
                node.get("name")
                    .and_then(Value::as_str)
                    .filter(|v| !v.trim().is_empty())
            })
            .or_else(|| {
                node.get("automationId")
                    .and_then(Value::as_str)
                    .filter(|v| !v.trim().is_empty())
            })
            .or_else(|| {
                node.get("value")
                    .and_then(Value::as_str)
                    .filter(|v| !v.trim().is_empty())
            })
            .map(|v| v.to_string())
            .unwrap_or_else(|| "(unnamed)".to_string());

        let bounds = bounds_from_value(node);

        self.ref_counter += 1;
        let ref_id = format!("E{}", self.ref_counter);
        let elem_ref = ElementRef {
            id: ref_id.clone(),
            role,
            name,
            bounds,
        };
        self.element_refs.insert(ref_id, elem_ref.clone());
        refs.push(elem_ref);

        if let Some(children) = node.get("children").and_then(Value::as_array) {
            for child in children {
                self.collect_uia_refs(child, refs);
            }
        }
    }

    // =====================================================
    // Snapshot: Build element reference map (like clawdbot's restoreRoleRefsForTarget)
    // =====================================================

    /// Take accessibility snapshot and build element reference map
    pub fn take_snapshot(&mut self) -> Result<Vec<ElementRef>> {
        let started_at = std::time::Instant::now();
        let mut refs: Vec<ElementRef>;
        self.element_refs.clear();
        self.ref_counter = 0;
        self.last_snapshot_refs.clear();
        self.last_snapshot_source = SnapshotSource::AppleScript;
        self.last_snapshot_id = None;

        let mut last_script_issue: Option<String> = None;

        #[cfg(target_os = "windows")]
        {
            refs = Vec::new();
            let retry_count = Self::snapshot_retry_count();
            let retry_sleep = std::time::Duration::from_millis(Self::snapshot_retry_ms());
            let recovery_enabled = Self::snapshot_focus_recovery_enabled();

            for attempt in 0..=retry_count {
                if attempt > 0 && recovery_enabled {
                    Self::recover_snapshot_focus(attempt);
                    std::thread::sleep(retry_sleep);
                }

                let snapshot = crate::macos::accessibility::snapshot(None);
                refs = self.parse_uia_snapshot_value(&snapshot);
                if !refs.is_empty() {
                    break;
                }
                last_script_issue = Some(format!("uia_snapshot_empty(attempt={})", attempt));
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            refs = Vec::new();

            // Use AppleScript to get accessibility tree
            let script = r#"
                tell application "System Events"
                    set frontApp to first application process whose frontmost is true
                    set appName to name of frontApp
                    
                    -- Get UI elements (simplified - full impl would recurse)
                    set output to ""
                    try
                        set allElements to entire contents of window 1 of frontApp
                        repeat with elem in allElements
                            try
                                set elemRole to role of elem
                                set elemName to name of elem
                                set elemPos to position of elem
                                set elemSize to size of elem
                                if elemName is not "" then
                                    set output to output & elemRole & "|" & elemName & "|" & (item 1 of elemPos) & "|" & (item 2 of elemPos) & "|" & (item 1 of elemSize) & "|" & (item 2 of elemSize) & "
    "
                                end if
                            end try
                        end repeat
                    end try
                    return output
                end tell
            "#;

            let retry_count = Self::snapshot_retry_count();
            let retry_sleep = std::time::Duration::from_millis(Self::snapshot_retry_ms());
            let recovery_enabled = Self::snapshot_focus_recovery_enabled();

            for attempt in 0..=retry_count {
                if attempt > 0 && recovery_enabled {
                    Self::recover_snapshot_focus(attempt);
                    std::thread::sleep(retry_sleep);
                }

                match Command::new("osascript").arg("-e").arg(script).output() {
                    Ok(output) => {
                        if output.status.success() {
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            refs = self.parse_snapshot_stdout(&stdout);
                            if !refs.is_empty() {
                                break;
                            }
                            last_script_issue = Some("osascript_ok_but_no_elements".to_string());
                        } else {
                            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                            let detail = if !stderr.is_empty() { stderr } else { stdout };
                            last_script_issue = Some(format!(
                                "osascript_failed(status={} detail={})",
                                output.status, detail
                            ));
                        }
                    }
                    Err(err) => {
                        last_script_issue = Some(format!("osascript_exec_error: {}", err));
                    }
                }
            }
        }

        if refs.is_empty() && Self::peekaboo_fallback_enabled() {
            if peekaboo_cli::is_available() {
                let front_app = CrossAppBridge::get_frontmost_app().ok();
                if let Ok(snapshot) = peekaboo_cli::take_snapshot(front_app.as_deref()) {
                    self.element_refs.clear();
                    self.ref_counter = 0;
                    self.last_snapshot_refs.clear();
                    self.last_snapshot_source = SnapshotSource::Peekaboo;
                    self.last_snapshot_id = snapshot.snapshot_id.clone();

                    for elem in snapshot.elements {
                        let bounds = elem.bounds.map(|(x, y, w, h)| Bounds {
                            x,
                            y,
                            width: w,
                            height: h,
                        });
                        let elem_ref = ElementRef {
                            id: elem.id.clone(),
                            role: elem.role.clone(),
                            name: elem.name.clone(),
                            bounds,
                        };
                        self.element_refs.insert(elem.id.clone(), elem_ref.clone());
                        self.last_snapshot_refs.push(elem_ref.clone());
                        refs.push(elem_ref);
                    }
                    println!(
                        "?벝 [Browser] Snapshot captured via Peekaboo: {} elements",
                        refs.len()
                    );
                    return Ok(refs);
                }
            }
        }

        self.last_snapshot_refs = refs.clone();
        let elapsed_ms = started_at.elapsed().as_millis() as u64;
        if elapsed_ms > Self::snapshot_perf_warn_ms() {
            println!(
                "[Browser][Perf] Snapshot slow: {} ms (refs={})",
                elapsed_ms,
                refs.len()
            );
        }
        println!("?벝 [Browser] Snapshot captured: {} elements", refs.len());
        if refs.is_empty() && !crate::env_flag("STEER_ALLOW_EMPTY_SNAPSHOT_REFS") {
            let front_app = CrossAppBridge::get_frontmost_app().unwrap_or_default();
            let issue = last_script_issue.unwrap_or_else(|| {
                if cfg!(target_os = "windows") {
                    "uia_snapshot_empty".to_string()
                } else {
                    "unknown".to_string()
                }
            });
            return Err(anyhow::anyhow!(
                "snapshot returned zero elements (frontmost='{}', issue='{}'). ensure target window is visible/focused and accessibility permissions are granted",
                front_app,
                issue
            ));
        }
        Ok(refs)
    }

    /// Clear cached refs when navigation changes the DOM (refs are not stable across navigations).
    pub fn reset_snapshot(&mut self) {
        self.element_refs.clear();
        self.ref_counter = 0;
        self.last_snapshot_refs.clear();
        self.last_snapshot_id = None;
    }

    // =====================================================
    // Core Interactions (ported from pw-tools-core.interactions.ts)
    // =====================================================

    /// Click element by reference (like clickViaPlaywright)
    pub fn click_by_ref(&self, ref_id: &str, double_click: bool) -> Result<()> {
        if self.last_snapshot_source == SnapshotSource::Peekaboo {
            let front_app = CrossAppBridge::get_frontmost_app().ok();
            let snapshot_id = self.last_snapshot_id.as_deref();
            peekaboo_cli::click(ref_id, snapshot_id, front_app.as_deref())
                .context("Peekaboo click failed")?;
            if double_click {
                std::thread::sleep(std::time::Duration::from_millis(100));
                peekaboo_cli::click(ref_id, snapshot_id, front_app.as_deref())
                    .context("Peekaboo double click failed")?;
            }
            println!("?뼮截?[Browser] Clicked ref '{}' via Peekaboo", ref_id);
            return Ok(());
        }

        let elem = self.element_refs.get(ref_id).ok_or_else(|| {
            anyhow::anyhow!("Element ref '{}' not found. Take a new snapshot.", ref_id)
        })?;

        let (x, y) = elem
            .bounds
            .as_ref()
            .map(|b| b.center())
            .ok_or_else(|| anyhow::anyhow!("Element '{}' has no bounds", ref_id))?;

        let click_count = if double_click { 2 } else { 1 };

        #[cfg(target_os = "windows")]
        {
            win32_click_at(x, y, click_count)?;
        }

        #[cfg(not(target_os = "windows"))]
        {
            let script = format!(
                r#"tell application "System Events" to click at {{{}, {}}} "#,
                x, y
            );

            for _ in 0..click_count {
                Command::new("osascript")
                    .arg("-e")
                    .arg(&script)
                    .output()
                    .context("Failed to execute click")?;

                if double_click {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }

        println!("?뼮截?[Browser] Clicked '{}' at ({}, {})", elem.name, x, y);
        Ok(())
    }

    /// Hover over element by reference (like hoverViaPlaywright)
    pub fn hover_by_ref(&self, ref_id: &str) -> Result<()> {
        if self.last_snapshot_source == SnapshotSource::Peekaboo {
            let front_app = CrossAppBridge::get_frontmost_app().ok();
            let snapshot_id = self.last_snapshot_id.as_deref();
            peekaboo_cli::click(ref_id, snapshot_id, front_app.as_deref())
                .context("Peekaboo hover fallback (click) failed")?;
            return Ok(());
        }

        let elem = self
            .element_refs
            .get(ref_id)
            .ok_or_else(|| anyhow::anyhow!("Element ref '{}' not found", ref_id))?;

        let (x, y) = elem
            .bounds
            .as_ref()
            .map(|b| b.center())
            .ok_or_else(|| anyhow::anyhow!("Element '{}' has no bounds", ref_id))?;

        #[cfg(target_os = "windows")]
        {
            win32_move_cursor(x, y);
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Move mouse without clicking
            let script = format!(
                r#"
                do shell script "cliclick m:{},{}"
                "#,
                x, y
            );

            // Fallback: Use CoreGraphics via AppleScript
            let _ = Command::new("osascript").arg("-e").arg(&script).output();
        }

        println!("?몘 [Browser] Hover over '{}' at ({}, {})", elem.name, x, y);
        Ok(())
    }

    /// Type text into focused element (like typeViaPlaywright)
    pub fn type_text(&self, text: &str, delay_ms: u64) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            win32_type_text(text, delay_ms)?;
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Use keystroke for reliable typing
            let escaped = text.replace("\"", "\\\"").replace("\\", "\\\\");

            let script = if delay_ms > 0 {
                format!(
                    r#"
                    tell application "System Events"
                        repeat with c in characters of "{}"
                            keystroke c
                            delay {}
                        end repeat
                    end tell
                    "#,
                    escaped,
                    delay_ms as f64 / 1000.0
                )
            } else {
                format!(
                    r#"tell application "System Events" to keystroke "{}""#,
                    escaped
                )
            };

            Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .output()
                .context("Failed to type text")?;
        }

        println!(
            "?⑨툘 [Browser] Typed: '{}'",
            if text.len() > 20 { &text[..20] } else { text }
        );
        Ok(())
    }

    /// Find element by name/text (returns ref_id)
    pub fn find_by_name(&self, name: &str) -> Option<String> {
        let name_lower = name.to_lowercase();

        for (ref_id, elem) in &self.element_refs {
            if elem.name.to_lowercase().contains(&name_lower) {
                return Some(ref_id.clone());
            }
        }
        None
    }

    pub fn find_first_by_role_contains(&self, needle: &str) -> Option<String> {
        let needle_lower = needle.to_lowercase();
        for elem in &self.last_snapshot_refs {
            if elem.role.to_lowercase().contains(&needle_lower) {
                return Some(elem.id.clone());
            }
        }
        None
    }

    pub fn summarize_refs(refs: &[ElementRef], max: usize) -> String {
        if refs.is_empty() {
            return "SNAPSHOT_REFS: (none)".to_string();
        }
        let mut parts: Vec<String> = Vec::new();
        for r in refs.iter().take(max) {
            let name = if r.name.trim().is_empty() {
                "(unnamed)"
            } else {
                r.name.as_str()
            };
            parts.push(format!("{} [{}] \"{}\"", r.id, r.role, name));
        }
        let mut summary = format!("SNAPSHOT_REFS: {}", parts.join("; "));
        if refs.len() > max {
            summary.push_str(&format!("; +{} more", refs.len() - max));
        }
        summary
    }

    /// Navigate to URL (opens in default browser or specified browser)
    pub fn navigate(&self, url: &str, browser: Option<&str>) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            open_url_windows(url, browser)?;
        }

        #[cfg(not(target_os = "windows"))]
        let browser_name = browser.unwrap_or("Safari");

        #[cfg(not(target_os = "windows"))]
        {
            let script = format!(
                r#"
                tell application "{}"
                    activate
                    open location "{}"
                end tell
                "#,
                browser_name, url
            );

            Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .output()
                .context("Failed to navigate")?;
        }

        println!("?뙋 [Browser] Navigate to: {}", url);
        Ok(())
    }

    // =====================================================
    // Helper: AI-Friendly Error (clawdbot pattern)
    // =====================================================

    pub fn to_ai_friendly_error(err: anyhow::Error, context: &str) -> anyhow::Error {
        anyhow::anyhow!(
            "Action failed on '{}': {}. Try taking a new snapshot or using a different approach.",
            context,
            err
        )
    }
}

// =====================================================
// Public API for DynamicController
// =====================================================

/// Singleton-style access
use once_cell::sync::Lazy;
use std::sync::Mutex;

/// Singleton-style access (Thread-Safe)
static BROWSER_AUTOMATION: Lazy<Mutex<BrowserAutomation>> =
    Lazy::new(|| Mutex::new(BrowserAutomation::new()));

pub fn get_browser_automation() -> std::sync::MutexGuard<'static, BrowserAutomation> {
    BROWSER_AUTOMATION.lock().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounds_center() {
        let bounds = Bounds {
            x: 100,
            y: 200,
            width: 50,
            height: 30,
        };
        assert_eq!(bounds.center(), (125, 215));
    }
}

// =====================================================
// LEGACY API COMPATIBILITY (for execution_controller.rs)
// =====================================================

pub fn open_url_in_chrome(url: &str) -> Result<()> {
    get_browser_automation().navigate(url, Some("Google Chrome"))
}

/// Scroll page by pixels - legacy API  
pub fn scroll_page(pixels: i32) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            INPUT, INPUT_MOUSE, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput,
        };

        // Windows wheel is the opposite direction from our existing convention.
        let scroll_amount = -pixels.saturating_mul(120) / 100;
        unsafe {
            let mut input = INPUT {
                r#type: INPUT_MOUSE,
                ..Default::default()
            };
            input.Anonymous.mi = MOUSEINPUT {
                dwFlags: MOUSEEVENTF_WHEEL,
                mouseData: scroll_amount as u32,
                ..Default::default()
            };
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let direction = if pixels > 0 { "down" } else { "up" };
        let amount = pixels.abs();

        let script = format!(
            r#"tell application "System Events" to scroll {} by {}"#,
            direction, amount
        );

        std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .context("Failed to scroll")?;
    }

    Ok(())
}

/// Apply flight filters - legacy API (stub) - Returns bool for success
pub fn apply_flight_filters(
    _budget: Option<&str>,
    _time_window: Option<&str>,
    _direct_only: Option<&str>,
) -> Result<bool> {
    println!("?좑툘 [Browser] apply_flight_filters: Use new ref-based API instead");
    Ok(false) // Return false to indicate manual action needed
}

/// Apply shopping filters - legacy API (stub) - Returns bool for success
pub fn apply_shopping_filters(
    _brand: Option<&str>,
    _price_min: Option<&str>,
    _price_max: Option<&str>,
) -> Result<bool> {
    println!("?좑툘 [Browser] apply_shopping_filters: Use new ref-based API instead");
    Ok(false) // Return false to indicate manual action needed
}

/// Click search button - legacy API (stub) - Returns bool for success
pub fn click_search_button() -> Result<bool> {
    println!("?좑툘 [Browser] click_search_button: Use new click_by_ref API instead");
    Ok(false) // Return false to indicate button not found
}

/// Get page context - legacy API (stub)
pub fn get_page_context() -> Result<String> {
    Ok("Page context: Use take_snapshot() for detailed element refs".to_string())
}

/// Fill flight fields - legacy API (stub) - Returns bool for success
pub fn fill_flight_fields(
    _from: &str,
    _to: &str,
    _date_start: &str,
    _date_end: Option<&str>,
) -> Result<bool> {
    println!("?좑툘 [Browser] fill_flight_fields: Use new type_text API instead");
    Ok(true) // Return true to indicate attempted (stub)
}

/// Fill search query - legacy API (stub) - Returns bool for success
pub fn fill_search_query(query: &str) -> Result<bool> {
    get_browser_automation().type_text(query, 0)?;
    Ok(true)
}

/// Autofill form - legacy API (stub) - Returns bool for success
pub fn autofill_form(
    _name: Option<&str>,
    _email: Option<&str>,
    _phone: Option<&str>,
    _address: Option<&str>,
) -> Result<bool> {
    println!("?좑툘 [Browser] autofill_form: Use new type_text API instead");
    Ok(true) // Return true to indicate attempted (stub)
}

/// Extract flight summary - legacy API (stub)
pub fn extract_flight_summary() -> Result<String> {
    Ok("Flight summary extraction: Use take_snapshot() + find_by_name()".to_string())
}

/// Extract shopping summary - legacy API (stub)
pub fn extract_shopping_summary() -> Result<String> {
    Ok("Shopping summary extraction: Use take_snapshot() + find_by_name()".to_string())
}

#[cfg(target_os = "windows")]
fn bounds_from_value(node: &Value) -> Option<Bounds> {
    let bounds = node.get("bounds")?;
    let x = bounds.get("x").and_then(json_value_i32)?;
    let y = bounds.get("y").and_then(json_value_i32)?;
    let width = bounds.get("width").and_then(json_value_i32)?;
    let height = bounds.get("height").and_then(json_value_i32)?;

    if width <= 0 || height <= 0 {
        return None;
    }

    Some(Bounds {
        x,
        y,
        width,
        height,
    })
}

#[cfg(target_os = "windows")]
fn json_value_i32(value: &Value) -> Option<i32> {
    value
        .as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .or_else(|| value.as_u64().and_then(|n| i32::try_from(n).ok()))
        .or_else(|| {
            value
                .as_f64()
                .and_then(|n| i32::try_from(n.round() as i64).ok())
        })
        .or_else(|| value.as_str().and_then(|s| s.parse::<i32>().ok()))
}

#[cfg(target_os = "windows")]
fn open_url_windows(url: &str, browser: Option<&str>) -> Result<()> {
    if let Some(name) = browser.map(|v| v.to_ascii_lowercase()) {
        let candidates: &[&str] = if name.contains("chrome") {
            &["chrome.exe", "chrome"]
        } else if name.contains("edge") {
            &["msedge.exe", "msedge"]
        } else if name.contains("firefox") {
            &["firefox.exe", "firefox"]
        } else if name.contains("brave") {
            &["brave.exe", "brave"]
        } else {
            &[]
        };

        for candidate in candidates {
            if Command::new(candidate).arg(url).spawn().is_ok() {
                return Ok(());
            }
        }
    }

    if Command::new("explorer.exe").arg(url).spawn().is_ok() {
        return Ok(());
    }

    Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn()
        .context("Failed to open URL on Windows")?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn win32_move_cursor(x: i32, y: i32) {
    use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;

    unsafe {
        let _ = SetCursorPos(x, y);
    }
}

#[cfg(target_os = "windows")]
fn win32_click_at(x: i32, y: i32, click_count: i32) -> Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, SendInput,
    };
    use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;

    let total_clicks = click_count.max(1) as usize;

    unsafe {
        let _ = SetCursorPos(x, y);
        for idx in 0..total_clicks {
            let mut inputs = [INPUT::default(), INPUT::default()];
            inputs[0].r#type = INPUT_MOUSE;
            inputs[0].Anonymous.mi.dwFlags = MOUSEEVENTF_LEFTDOWN;

            inputs[1].r#type = INPUT_MOUSE;
            inputs[1].Anonymous.mi.dwFlags = MOUSEEVENTF_LEFTUP;

            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            if idx + 1 < total_clicks {
                std::thread::sleep(std::time::Duration::from_millis(90));
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn win32_type_text(text: &str, delay_ms: u64) -> Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput,
        VK_RETURN, VIRTUAL_KEY,
    };

    fn send_virtual_key(vk: VIRTUAL_KEY) {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP, SendInput,
        };

        unsafe {
            let mut inputs = [INPUT::default(), INPUT::default()];
            inputs[0].r#type = INPUT_KEYBOARD;
            inputs[0].Anonymous.ki.wVk = vk;

            inputs[1].r#type = INPUT_KEYBOARD;
            inputs[1].Anonymous.ki.wVk = vk;
            inputs[1].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;

            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }

    let delay = std::time::Duration::from_millis(delay_ms.min(500));

    unsafe {
        for ch in text.chars() {
            if ch == '\r' {
                continue;
            }

            if ch == '\n' {
                send_virtual_key(VK_RETURN);
            } else {
                let mut inputs = [INPUT::default(), INPUT::default()];
                inputs[0].r#type = INPUT_KEYBOARD;
                inputs[0].Anonymous.ki = KEYBDINPUT {
                    wScan: ch as u16,
                    dwFlags: KEYEVENTF_UNICODE,
                    ..Default::default()
                };

                inputs[1].r#type = INPUT_KEYBOARD;
                inputs[1].Anonymous.ki = KEYBDINPUT {
                    wScan: ch as u16,
                    dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                    ..Default::default()
                };

                SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            }

            if delay_ms > 0 {
                std::thread::sleep(delay);
            }
        }
    }

    Ok(())
}
