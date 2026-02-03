use std::process::Command;
use anyhow::{Context, Result};
use crate::send_policy::{self, SendDecision};

pub fn send(title: &str, message: &str) -> Result<()> {
    if matches!(send_policy::should_send(title, message), SendDecision::Deny) {
        println!("🔕 [NOTIFICATION] Suppressed by policy: {}: {}", title, message);
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
    
    // Fallback log for non-macOS (or debugging)
    println!("\n🔔 [NOTIFICATION] {}: {}\n", title, message);
    
    Ok(())
}
