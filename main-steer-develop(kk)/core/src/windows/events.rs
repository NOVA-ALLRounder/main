use crate::schema::{EventEnvelope, ResourceContext};
use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use std::process::Command;
use std::thread;
use tokio::sync::mpsc;
use uuid::Uuid;

pub fn start_event_tap(tx: mpsc::Sender<String>) -> Result<()> {
    println!("[Windows] Event tap compatibility mode enabled (window/activity polling).");

    thread::spawn(move || {
        let mut last_app = String::new();
        let mut last_title = String::new();

        loop {
            thread::sleep(std::time::Duration::from_millis(900));

            let Some((app, title)) = read_active_window() else {
                continue;
            };

            if app != last_app || title != last_title {
                last_app = app.clone();
                last_title = title.clone();

                let envelope = base_envelope(
                    "windows_event_poll",
                    &app,
                    "app_switch",
                    "P2",
                    Some(ResourceContext {
                        resource_type: "app".to_string(),
                        id: app.clone(),
                    }),
                    json!({
                        "app": app,
                        "window_title": title,
                    }),
                );

                if let Ok(serialized) = serde_json::to_string(&envelope) {
                    if tx.try_send(serialized).is_err() {
                        // drop when channel is full/closed
                    }
                }
            }
        }
    });

    Ok(())
}

fn read_active_window() -> Option<(String, String)> {
    let script = r#"
Add-Type -TypeDefinition @"
using System;
using System.Text;
using System.Diagnostics;
using System.Runtime.InteropServices;
public static class Win32 {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
"@;
$h = [Win32]::GetForegroundWindow();
if ($h -eq [IntPtr]::Zero) { return }
$sb = New-Object System.Text.StringBuilder 512
[void][Win32]::GetWindowText($h, $sb, $sb.Capacity)
$pid = 0
[void][Win32]::GetWindowThreadProcessId($h, [ref]$pid)
$proc = ""
try { $proc = (Get-Process -Id $pid -ErrorAction Stop).ProcessName } catch {}
[pscustomobject]@{
  app = $proc
  title = $sb.ToString()
} | ConvertTo-Json -Compress
"#;

    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }

    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let app = parsed
        .get("app")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .trim()
        .to_string();
    let title = parsed
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if app.is_empty() && title.is_empty() {
        None
    } else {
        Some((app, title))
    }
}

fn base_envelope(
    source: &str,
    app: &str,
    event_type: &str,
    priority: &str,
    resource: Option<ResourceContext>,
    payload: serde_json::Value,
) -> EventEnvelope {
    EventEnvelope {
        schema_version: "1.0".to_string(),
        event_id: Uuid::new_v4().to_string(),
        ts: Utc::now().to_rfc3339(),
        source: source.to_string(),
        app: app.to_string(),
        event_type: event_type.to_string(),
        priority: priority.to_string(),
        resource,
        payload,
        privacy: None,
        pid: None,
        window_id: None,
        window_title: None,
        browser_url: None,
        raw: None,
    }
}
