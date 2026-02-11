/// Windows system notifications using Toast notifications

use crate::platform::traits::NotificationProvider;
use anyhow::Result;
use std::process::Command;

/// Windows Notification Provider using Toast notifications
pub struct WindowsNotifications;

impl NotificationProvider for WindowsNotifications {
    fn send_notification(&self, title: &str, message: &str) -> Result<()> {
        // Use PowerShell to create Toast notification
        let script = format!(
            r#"
            [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
            [Windows.UI.Notifications.ToastNotification, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
            [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null

            $template = @"
            <toast>
                <visual>
                    <binding template='ToastGeneric'>
                        <text>{}</text>
                        <text>{}</text>
                    </binding>
                </visual>
            </toast>
            "@

            $xml = New-Object Windows.Data.Xml.Dom.XmlDocument
            $xml.LoadXml($template)
            $toast = New-Object Windows.UI.Notifications.ToastNotification $xml
            [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Steer OS Agent').Show($toast)
            "#,
            title.replace('"', "\"\""),
            message.replace('"', "\"\"")
        );

        let output = Command::new("powershell")
            .args(&["-NoProfile", "-Command", &script])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to send notification: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("?醫묓닔  Toast notification failed: {}", stderr);
            // Fallback to console output
            println!("[{}] {}", title, message);
        }

        Ok(())
    }
}
