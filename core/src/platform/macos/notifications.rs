/// macOS system notifications using osascript

use crate::platform::traits::NotificationProvider;
use anyhow::{Context, Result};
use std::process::Command;

/// macOS notification provider using osascript
pub struct MacOSNotifications;

impl NotificationProvider for MacOSNotifications {
    fn send_notification(&self, title: &str, message: &str) -> Result<()> {
        let script = format!(
            r#"display notification {:?} with title {:?}"#,
            message, title
        );

        let output = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .context("Failed to send notification via osascript")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Notification failed: {}", stderr);
        }

        Ok(())
    }
}
