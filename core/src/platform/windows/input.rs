use anyhow::{anyhow, Result};
use std::io::Write;
use std::process::{Command, Stdio};

use super::{run_cmd_start, run_powershell};

fn escape_send_keys_text(text: &str) -> String {
    text.replace('\'', "''")
        .replace('{', "{{}")
        .replace('}', "{}}")
}

fn shortcut_key_fragment(key: &str) -> String {
    match key.trim().to_ascii_lowercase().as_str() {
        "escape" | "esc" => "{ESC}".to_string(),
        "enter" | "return" => "{ENTER}".to_string(),
        "tab" => "{TAB}".to_string(),
        "backspace" => "{BACKSPACE}".to_string(),
        "delete" | "del" => "{DELETE}".to_string(),
        other if other.len() == 1 => escape_send_keys_text(other),
        other => escape_send_keys_text(other),
    }
}

fn shortcut_modifier_prefix(modifiers: &[String]) -> String {
    let mut prefix = String::new();
    for modifier in modifiers {
        match modifier.trim().to_ascii_lowercase().as_str() {
            "control" | "ctrl" | "command" | "cmd" | "meta" => prefix.push('^'),
            "alt" | "option" => prefix.push('%'),
            "shift" => prefix.push('+'),
            _ => {}
        }
    }
    prefix
}

pub(super) fn activate_app_by_name(app_name: &str) -> Result<()> {
    let trimmed = app_name.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("frontmost") {
        return Ok(());
    }
    run_cmd_start(trimmed)
}

pub(super) fn set_clipboard_text(text: &str) -> Result<()> {
    let mut child = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "Set-Clipboard -Value ([Console]::In.ReadToEnd())",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!(e.to_string()))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| anyhow!(e.to_string()))?;
    }

    let output = child.wait_with_output().map_err(|e| anyhow!(e.to_string()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "powershell Set-Clipboard failed(status={}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

pub(super) fn get_clipboard_text() -> Result<String> {
    run_powershell("Get-Clipboard -Raw")
}

pub(super) fn browser_hover_at(x: i32, y: i32) -> Result<()> {
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point({}, {})",
        x, y
    );
    run_powershell(&script).map(|_| ())
}

pub(super) fn browser_click_at(x: i32, y: i32, double_click: bool) -> Result<()> {
    let repeat = if double_click { 2 } else { 1 };
    let script = format!(
        r#"
Add-Type -AssemblyName System.Windows.Forms;
Add-Type -AssemblyName System.Drawing;
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class WinMouse {{
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int X, int Y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint dwFlags, uint dx, uint dy, uint dwData, UIntPtr dwExtraInfo);
}}
"@;
[WinMouse]::SetCursorPos({x}, {y}) | Out-Null;
for ($i = 0; $i -lt {repeat}; $i++) {{
  [WinMouse]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero);
  [WinMouse]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero);
  if ({repeat} -gt 1) {{ Start-Sleep -Milliseconds 120 }}
}}
"#
    );
    run_powershell(&script).map(|_| ())
}

pub(super) fn browser_type_text(text: &str, delay_ms: u64) -> Result<()> {
    let escaped = escape_send_keys_text(text);
    let script = if delay_ms == 0 {
        format!(
            "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('{}')",
            escaped
        )
    } else {
        let chars: Vec<String> = text
            .chars()
            .map(|ch| {
                let frag = ch.to_string().replace('\'', "''");
                format!(
                    "[System.Windows.Forms.SendKeys]::SendWait('{}'); Start-Sleep -Milliseconds {}",
                    frag, delay_ms
                )
            })
            .collect();
        format!(
            "Add-Type -AssemblyName System.Windows.Forms; {}",
            chars.join("; ")
        )
    };
    run_powershell(&script).map(|_| ())
}

pub(super) fn browser_navigate(url: &str, browser_hint: Option<&str>) -> Result<()> {
    if let Some(browser) = browser_hint.filter(|value| !value.trim().is_empty()) {
        Command::new("cmd")
            .args(["/C", "start", "", browser, url])
            .status()
            .map_err(|e| anyhow!(e.to_string()))
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(anyhow!("cmd start failed for browser {} {}", browser, url))
                }
            })
    } else {
        run_cmd_start(url)
    }
}

pub(super) fn browser_scroll(pixels: i32) -> Result<()> {
    if pixels == 0 {
        return Ok(());
    }

    let wheel_delta = if pixels > 0 { -120 } else { 120 };
    let repeat = ((pixels.abs() + 119) / 120).max(1);
    let script = format!(
        r#"
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class WinMouse {{
  [DllImport("user32.dll")]
  public static extern void mouse_event(uint dwFlags, uint dx, uint dy, int dwData, UIntPtr dwExtraInfo);
}}
"@;
for ($i = 0; $i -lt {repeat}; $i++) {{
  [WinMouse]::mouse_event(0x0800, 0, 0, {wheel_delta}, [UIntPtr]::Zero);
  Start-Sleep -Milliseconds 40
}}
"#
    );
    run_powershell(&script).map(|_| ())
}

pub(super) fn keyboard_shortcut(key: &str, modifiers: &[String]) -> Result<()> {
    let sequence = format!(
        "{}{}",
        shortcut_modifier_prefix(modifiers),
        shortcut_key_fragment(key)
    );
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('{}')",
        sequence
    );
    run_powershell(&script).map(|_| ())
}
