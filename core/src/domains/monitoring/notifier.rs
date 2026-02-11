use anyhow::Result;
use crate::send_policy::{self, SendDecision};

pub fn send(title: &str, message: &str) -> Result<()> {
    if matches!(send_policy::should_send(title, message), SendDecision::Deny) {
        println!("?逾?[NOTIFICATION] Suppressed by policy: {}: {}", title, message);
        return Ok(());
    }

    // Use platform-agnostic notification
    let platform = crate::platform::get_platform();
    match platform.notifications().send_notification(title, message) {
        Ok(_) => {},
        Err(e) => {
            // Fallback to console output
            println!("?醫묓닔  Notification failed: {}", e);
        }
    }

    // Also log to console
    println!("\n?逾?[NOTIFICATION] {}: {}\n", title, message);

    Ok(())
}
