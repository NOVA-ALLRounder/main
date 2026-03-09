use anyhow::Result;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Bridge for passing data between applications
pub struct CrossAppBridge;

impl CrossAppBridge {
    fn run_osascript_output(script: &str, timeout_ms: u64) -> Result<Output> {
        let mut child = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let started = Instant::now();
        loop {
            if (child.try_wait()?).is_some() {
                return Ok(child.wait_with_output()?);
            }
            if started.elapsed() >= Duration::from_millis(timeout_ms) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(anyhow::anyhow!(
                    "osascript timed out after {}ms",
                    timeout_ms
                ));
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    }

    pub fn copy_to_clipboard(text: &str) -> Result<()> {
        let script = format!(
            r#"set the clipboard to "{}""#,
            text.replace("\"", "\\\"").replace("\n", "\\n")
        );

        std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .status()?;

        println!(
            "📋 [Bridge] Copied to clipboard: {}...",
            &text[..text.len().min(50)]
        );
        Ok(())
    }

    pub fn get_clipboard() -> Result<String> {
        let output = std::process::Command::new("osascript")
            .arg("-e")
            .arg("the clipboard")
            .output()?;

        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(text)
    }

    pub fn paste() -> Result<()> {
        let script = r#"tell application "System Events" to keystroke "v" using command down"#;
        std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .status()?;
        Ok(())
    }

    pub fn switch_to_app(app_name: &str) -> Result<()> {
        println!("      🚀 [Bridge] Opening '{}' via CLI...", app_name);

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

        let open_timeout_ms = std::env::var("STEER_OPEN_APP_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5000);
        let mut open_ok = false;
        if let Ok(mut child) = std::process::Command::new("open")
            .arg("-a")
            .arg(app_name)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
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
                if front.eq_ignore_ascii_case(app_name) {
                    return true;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(wait_ms));
        }
        false
    }

    pub fn get_frontmost_app() -> Result<String> {
        let output = Self::run_osascript_output(
            r#"tell application "System Events" to get name of first application process whose frontmost is true"#,
            1200,
        )?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn get_selected_text() -> Result<Option<String>> {
        let old_clipboard = Self::get_clipboard().ok();

        let script = r#"tell application "System Events" to keystroke "c" using command down"#;
        std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .status()?;

        std::thread::sleep(std::time::Duration::from_millis(100));

        let new_clipboard = Self::get_clipboard()?;

        if Some(&new_clipboard) != old_clipboard.as_ref() {
            Ok(Some(new_clipboard))
        } else {
            Ok(None)
        }
    }

    pub fn write_temp_file(content: &str, extension: &str) -> Result<String> {
        let path = format!("/tmp/steer_bridge_{}.{}", uuid::Uuid::new_v4(), extension);
        std::fs::write(&path, content)?;
        println!("📁 [Bridge] Wrote temp file: {}", path);
        Ok(path)
    }
}
