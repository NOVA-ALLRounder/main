use serde_json::{json, Value};
use std::process::Command;

pub fn snapshot(scope: Option<String>) -> Value {
    json!({
        "platform": "windows",
        "scope": scope,
        "status": "limited",
        "message": "Windows accessibility snapshot is in compatibility mode"
    })
}

pub fn get_selected_text() -> Option<String> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", "Get-Clipboard -Raw"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

