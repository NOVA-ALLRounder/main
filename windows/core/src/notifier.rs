use crate::send_policy::{self, SendDecision};
use anyhow::Result;
#[cfg(target_os = "macos")]
use anyhow::Context;
#[cfg(target_os = "macos")]
use std::process::Command;

pub fn send(title: &str, message: &str) -> Result<()> {
    if matches!(send_policy::should_send(title, message), SendDecision::Deny) {
        println!(
            "🔕 [NOTIFICATION] Suppressed by policy: {}: {}",
            title, message
        );
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        // Escape quotes to prevent injection
        // Using Debug formatter {:?} adds surrounding quotes and escapes internal quotes
        let script = format!("display notification {:?} with title {:?}", message, title);

        Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()
            .context("Failed to send notification via osascript")?;
    }

    #[cfg(target_os = "windows")]
    {
        // Windows Toast Notification via winrt-notification
        match send_windows_toast(title, message) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("[NOTIFICATION] Toast failed ({}), falling back to console", e);
            }
        }
    }

    // Fallback log for all platforms (or debugging)
    println!("\n🔔 [NOTIFICATION] {}: {}\n", title, message);

    Ok(())
}

#[cfg(target_os = "windows")]
fn send_windows_toast(title: &str, message: &str) -> Result<()> {
    use winrt_notification::{Duration, Sound, Toast};

    Toast::new(Toast::POWERSHELL_APP_ID)
        .title(title)
        .text1(message)
        .duration(Duration::Short)
        .sound(Some(Sound::Default))
        .show()
        .map_err(|e| anyhow::anyhow!("Windows toast error: {:?}", e))?;

    Ok(())
}
