/// macOS application control using AppleScript

use crate::platform::traits::AppController;
use anyhow::{Context, Result};
use std::process::Command;

/// macOS app controller using AppleScript
pub struct MacOSAppController;

impl AppController for MacOSAppController {
    fn launch_app(&self, app_name: &str) -> Result<()> {
        let script = format!("tell application {:?} to launch", app_name);
        run_applescript(&script)?;
        Ok(())
    }

    fn activate_app(&self, app_name: &str) -> Result<()> {
        let script = format!("tell application {:?} to activate", app_name);
        run_applescript(&script)?;
        Ok(())
    }

    fn get_active_app(&self) -> Result<(String, String)> {
        // Get frontmost app name
        let app_script = r#"tell application "System Events" to get name of first application process whose frontmost is true"#;
        let app_name = run_applescript(app_script)?;

        // Get window title (try to get from frontmost window)
        let window_script = r#"
            tell application "System Events"
                tell (first application process whose frontmost is true)
                    try
                        get title of front window
                    on error
                        return ""
                    end try
                end tell
            end tell
        "#;
        let window_title = run_applescript(window_script).unwrap_or_default();

        Ok((app_name, window_title))
    }

    fn open_url(&self, url: &str) -> Result<()> {
        Command::new("open")
            .arg(url)
            .spawn()
            .with_context(|| format!("Failed to open URL: {}", url))?;
        Ok(())
    }

    fn run_script(&self, script: &str) -> Result<String> {
        run_applescript(script)
    }
}

/// Execute AppleScript and return stdout
fn run_applescript(script: &str) -> Result<String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .context("Failed to run AppleScript")?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        anyhow::bail!("AppleScript Error: {}", stderr);
    }

    Ok(stdout)
}

/// Execute multi-line AppleScript with arguments
#[allow(dead_code)]
pub fn run_applescript_with_args(lines: &[&str], args: &[&str]) -> Result<String> {
    let mut cmd = Command::new("osascript");

    for line in lines {
        cmd.arg("-e").arg(line);
    }

    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run AppleScript")?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        anyhow::bail!("AppleScript Error: {}", stderr);
    }

    Ok(stdout)
}

/// Control specific apps (Music, Notes, etc.)
#[allow(dead_code)]
pub fn control_app(app: &str, command: &str) -> Result<String> {
    let script = match (app.to_lowercase().as_str(), command) {
        ("music", "play") => "tell application \"Music\" to play",
        ("music", "pause") => "tell application \"Music\" to pause",
        ("music", "next") => "tell application \"Music\" to next track",
        ("notes", "new") => "tell application \"Notes\" to make new note at folder \"Notes\"",
        _ => return Err(anyhow::anyhow!("Unknown app control command")),
    };

    run_applescript(script)
}

/// Execute JavaScript in Chrome
#[allow(dead_code)]
pub fn execute_js_in_chrome(script: &str) -> Result<String> {
    let lines = [
        "on run argv",
        "  set js to item 1 of argv",
        "  tell application \"Google Chrome\"",
        "    execute javascript js in active tab of front window",
        "  end tell",
        "end run",
    ];

    run_applescript_with_args(&lines, &[script])
}

/// Get active window context (window title, browser URL)
#[allow(dead_code)]
pub fn get_active_window_context() -> Result<(String, String)> {
    // Get frontmost app
    let app_script = r#"tell application "System Events" to get name of first application process whose frontmost is true"#;
    let app_name = run_applescript(app_script)?;

    // Get window title
    let window_script = format!(
        r#"tell application "System Events" to tell process "{}" to get title of front window"#,
        app_name
    );
    let window_title = run_applescript(&window_script).unwrap_or_default();

    // Try to get URL for browsers
    let url = if app_name.contains("Chrome") {
        let url_script = r#"tell application "Google Chrome" to get URL of active tab of front window"#;
        run_applescript(url_script).unwrap_or_default()
    } else if app_name.contains("Safari") {
        let url_script = r#"tell application "Safari" to get URL of front document"#;
        run_applescript(url_script).unwrap_or_default()
    } else {
        String::new()
    };

    if url.is_empty() {
        Ok((window_title, String::new()))
    } else {
        Ok((window_title, url))
    }
}
