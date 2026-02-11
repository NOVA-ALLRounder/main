/// Windows application control using PowerShell and Win32 APIs.

use crate::platform::traits::AppController;
use anyhow::Result;
use std::process::Command;
use windows::core::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Windows App Controller implementation.
pub struct WindowsAppController;

/// Map user-friendly app names to Windows executable names.
fn map_app_name_to_executable(app_name: &str) -> &str {
    let lower = app_name.to_lowercase();
    match lower.as_str() {
        // Common app aliases
        "notepad" => "notepad",
        "calculator" | "calc" => "calc",
        "paint" | "mspaint" => "mspaint",
        "wordpad" => "write",
        "excel" => "excel",
        "word" => "winword",
        "powerpoint" | "ppt" => "powerpnt",
        "notion" => "notion",
        "chrome" => "chrome",
        "firefox" => "firefox",
        "edge" => "msedge",
        "vscode" | "vs code" | "code" | "visual studio code" => "code",
        "cmd" | "command prompt" => "cmd",
        "powershell" => "powershell",
        "terminal" => "wt", // Windows Terminal
        "explorer" => "explorer",
        "control panel" => "control",

        // Default: use input as-is
        _ => app_name,
    }
}

impl AppController for WindowsAppController {
    fn launch_app(&self, app_name: &str) -> Result<()> {
        let executable = map_app_name_to_executable(app_name);

        Command::new("cmd")
            .args(["/C", "start", "", executable])
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to launch '{}': {}", app_name, e))?;
        Ok(())
    }

    fn activate_app(&self, app_name: &str) -> Result<()> {
        unsafe {
            let title_wide: Vec<u16> = app_name.encode_utf16().chain(Some(0)).collect();
            let hwnd = FindWindowW(None, PCWSTR::from_raw(title_wide.as_ptr()));

            if hwnd.0 != 0 {
                if IsIconic(hwnd).as_bool() {
                    ShowWindow(hwnd, SW_RESTORE);
                }
                SetForegroundWindow(hwnd);
                Ok(())
            } else {
                Err(anyhow::anyhow!("Window not found: {}", app_name))
            }
        }
    }

    fn get_active_app(&self) -> Result<(String, String)> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0 == 0 {
                return Err(anyhow::anyhow!("No foreground window"));
            }

            let mut title = vec![0u16; 512];
            let len = GetWindowTextW(hwnd, &mut title);
            let window_title = if len > 0 {
                String::from_utf16_lossy(&title[..len as usize])
            } else {
                String::new()
            };

            let app_name = window_title.clone();
            Ok((app_name, window_title))
        }
    }

    fn open_url(&self, url: &str) -> Result<()> {
        Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to open URL: {}", e))?;
        Ok(())
    }

    fn run_script(&self, script: &str) -> Result<String> {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to run PowerShell script: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(anyhow::anyhow!(
                "PowerShell error: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }
}
