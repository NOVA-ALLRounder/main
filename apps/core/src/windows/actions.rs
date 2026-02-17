use anyhow::{anyhow, Result};
use std::process::Command;

pub fn click_element(element_id: &str) -> Result<()> {
    // Compatibility mode: accept "x,y" as coordinates.
    let mut parts = element_id.split(',');
    let x = parts
        .next()
        .ok_or_else(|| anyhow!("Usage: click <x,y> on Windows"))?
        .trim()
        .parse::<i32>()
        .map_err(|_| anyhow!("Invalid X coordinate"))?;
    let y = parts
        .next()
        .ok_or_else(|| anyhow!("Usage: click <x,y> on Windows"))?
        .trim()
        .parse::<i32>()
        .map_err(|_| anyhow!("Invalid Y coordinate"))?;

    let script = format!(
        r#"
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class Win32 {{
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int X, int Y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint dwFlags, uint dx, uint dy, uint dwData, UIntPtr dwExtraInfo);
}}
"@;
[Win32]::SetCursorPos({x}, {y}) | Out-Null;
[System.Threading.Thread]::Sleep(30);
[Win32]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero);
[Win32]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero);
"#
    );

    let mut last_err = String::new();
    for _ in 0..3 {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output()?;
        if output.status.success() {
            return Ok(());
        }
        last_err = String::from_utf8_lossy(&output.stderr).to_string();
        std::thread::sleep(std::time::Duration::from_millis(120));
    }

    Err(anyhow!("Windows click failed: {}", last_err))
}

pub fn type_text(text: &str) -> Result<()> {
    // Stable mode: clipboard paste via Ctrl+V (supports multiline/special chars).
    let escaped = text.replace('\'', "''");
    let script = format!(
        r#"
Add-Type -AssemblyName System.Windows.Forms;
$old = $null
try {{ $old = Get-Clipboard -Raw -ErrorAction Stop }} catch {{}}
$tmp = @'
{escaped}
'@;
Set-Clipboard -Value $tmp;
$wshell = New-Object -ComObject WScript.Shell;
$wshell.SendKeys('^v');
[System.Threading.Thread]::Sleep(120);
if ($null -ne $old) {{ try {{ Set-Clipboard -Value $old }} catch {{}} }}
"#
    );

    let mut last_err = String::new();
    for _ in 0..2 {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output()?;
        if output.status.success() {
            return Ok(());
        }
        last_err = String::from_utf8_lossy(&output.stderr).to_string();
        std::thread::sleep(std::time::Duration::from_millis(150));
    }

    Err(anyhow!("Windows type failed: {}", last_err))
}

pub fn launch_app(app_name: &str) -> Result<()> {
    let script = format!(
        r#"
$wshell = New-Object -ComObject WScript.Shell;
$wshell.Run("{}", 1, $false);
"#,
        app_name
    );
    // Simple Run command on Windows often works for registered apps like 'calc', 'notepad'
    // Fallback to searching Start Menu is harder without specific paths.
    // Let's assume standard run commands for now.
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()?;
    
    if output.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow!("Failed to launch app: {}", err))
    }
}

pub fn send_keys(keys: &str) -> Result<()> {
    // Maps simplified keystrokes to SendKeys format if needed
    // e.g. "enter" -> "{ENTER}", "command+c" -> "^c" (Windows uses Ctrl for Command)
    let mapped = match keys.to_lowercase().as_str() {
        "enter" | "return" => "{ENTER}".to_string(),
        "tab" => "{TAB}".to_string(),
        "esc" | "escape" => "{ESC}".to_string(),
        "space" => " ".to_string(),
        k if k.contains("command+") || k.contains("ctrl+") => {
            // Replace command/ctrl with ^
            let parts: Vec<&str> = k.split('+').collect();
            if let Some(last) = parts.last() {
                format!("^{}", last)
            } else {
                k.to_string()
            }
        },
        k => k.to_string(), // Pass through normal chars
    };

    let script = format!(
        r#"
$wshell = New-Object -ComObject WScript.Shell;
$wshell.SendKeys('{}');
"#,
        mapped
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow!("Failed to send keys: {}", err))
    }
}
