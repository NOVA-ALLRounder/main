use anyhow::Result;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::platform::{current_platform, PlatformKind};

/// Bridge for passing data between applications
pub struct CrossAppBridge;

impl CrossAppBridge {
    pub fn copy_to_clipboard(text: &str) -> Result<()> {
        current_platform().set_clipboard_text(text)?;

        println!(
            "📋 [Bridge] Copied to clipboard: {}...",
            &text[..text.len().min(50)]
        );
        Ok(())
    }

    pub fn get_clipboard() -> Result<String> {
        current_platform().get_clipboard_text()
    }

    pub fn paste() -> Result<()> {
        current_platform().paste_clipboard()
    }

    pub fn switch_to_app(app_name: &str) -> Result<()> {
        println!("      🚀 [Bridge] Opening '{}' via CLI...", app_name);

        if current_platform().kind() == PlatformKind::MacOS {
            let peekaboo_enabled = std::env::var("STEER_ENABLE_PEEKABOO_LAUNCH")
                .ok()
                .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
                .unwrap_or(false);
            if peekaboo_enabled && crate::peekaboo_cli::is_available() {
                let timeout_ms = std::env::var("STEER_PEEKABOO_TIMEOUT_MS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(4000);

                let launch = Command::new("peekaboo")
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
        }

        current_platform().activate_app_by_name(app_name)?;
        let _ = Self::wait_for_frontmost(app_name, 8, 200);

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
        Ok(current_platform().frontmost_app_name()?.unwrap_or_default())
    }

    pub fn get_selected_text() -> Result<Option<String>> {
        crate::platform::current_platform().selected_text()
    }

    pub fn write_temp_file(content: &str, extension: &str) -> Result<String> {
        let path = format!("/tmp/steer_bridge_{}.{}", uuid::Uuid::new_v4(), extension);
        std::fs::write(&path, content)?;
        println!("📁 [Bridge] Wrote temp file: {}", path);
        Ok(path)
    }
}
