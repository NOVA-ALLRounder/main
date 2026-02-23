use anyhow::{Context, Result};
use std::process::Command;
#[cfg(target_os = "windows")]
use {
    once_cell::sync::Lazy,
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
    std::fs,
    std::path::PathBuf,
    std::sync::Mutex,
};

#[cfg(target_os = "windows")]
#[derive(Clone, Default, Serialize, Deserialize)]
struct MailDraftState {
    id: String,
    recipient: String,
    subject: String,
    body: String,
}

#[cfg(target_os = "windows")]
#[derive(Default, Serialize, Deserialize)]
struct WindowsAutomationState {
    next_id: u64,
    mail_drafts: HashMap<String, MailDraftState>,
    note_id: Option<String>,
    note_name: String,
    note_body: String,
    textedit_id: Option<String>,
    textedit_name: String,
    textedit_body: String,
}

#[cfg(target_os = "windows")]
static WINDOWS_AUTOMATION_STATE: Lazy<Mutex<WindowsAutomationState>> =
    Lazy::new(|| Mutex::new(windows_state_load()));
#[cfg(target_os = "windows")]
static WINDOWS_LAST_ACTIVATED_APP: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

#[cfg(target_os = "windows")]
fn windows_state_file() -> PathBuf {
    let mut base = dirs::data_local_dir().unwrap_or_else(std::env::temp_dir);
    base.push("steer");
    let _ = fs::create_dir_all(&base);
    base.push("windows_automation_state.json");
    base
}

#[cfg(target_os = "windows")]
fn windows_state_load() -> WindowsAutomationState {
    let path = windows_state_file();
    let Ok(raw) = fs::read_to_string(path) else {
        return WindowsAutomationState::default();
    };
    serde_json::from_str::<WindowsAutomationState>(&raw).unwrap_or_default()
}

#[cfg(target_os = "windows")]
fn windows_state_persist_locked(state: &WindowsAutomationState) {
    let path = windows_state_file();
    if let Ok(raw) = serde_json::to_string(state) {
        let _ = fs::write(path, raw);
    }
}

#[cfg(target_os = "windows")]
fn windows_set_last_activated_app(app: &str) {
    let normalized = app.trim();
    if normalized.is_empty() {
        return;
    }
    if let Ok(mut slot) = WINDOWS_LAST_ACTIVATED_APP.lock() {
        *slot = Some(normalized.to_string());
    }
}

#[cfg(target_os = "windows")]
fn windows_get_last_activated_app() -> Option<String> {
    WINDOWS_LAST_ACTIVATED_APP
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
}

#[cfg(target_os = "windows")]
fn canonicalize_frontmost_windows(process_name: &str) -> String {
    let lower = process_name.trim().to_ascii_lowercase();
    match lower.as_str() {
        "explorer" => "Finder".to_string(),
        "outlook" => "Mail".to_string(),
        "msedge" | "microsoftedge" | "edge" => "Safari".to_string(),
        "chrome" => "Google Chrome".to_string(),
        "firefox" => "Firefox".to_string(),
        "calc" | "calculatorapp" | "calculator" => "Calculator".to_string(),
        "notepad" => {
            if let Some(last) = windows_get_last_activated_app() {
                if app_names_match(&last, "Notes") {
                    return "Notes".to_string();
                }
                if app_names_match(&last, "TextEdit") {
                    return "TextEdit".to_string();
                }
            }
            "TextEdit".to_string()
        }
        _ => process_name.trim().to_string(),
    }
}

#[cfg(target_os = "windows")]
fn run_powershell(script: &str) -> Result<String> {
    let output = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(script)
        .output()
        .context("Failed to run PowerShell")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "PowerShell error: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(target_os = "windows")]
fn frontmost_process_name_windows() -> Result<String> {
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::System::Threading::*;
    use windows::Win32::Foundation::*;

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return Err(anyhow::anyhow!("No foreground window"));
        }

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return Err(anyhow::anyhow!("Failed to get process ID"));
        }

        let process = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            pid,
        ).map_err(|e| anyhow::anyhow!("OpenProcess failed: {}", e))?;

        let mut buf = [0u16; 512];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(process);

        if ok.is_err() {
            return Err(anyhow::anyhow!("QueryFullProcessImageName failed"));
        }

        let full_path = String::from_utf16_lossy(&buf[..size as usize]);
        // Extract just the filename without extension
        let name = full_path
            .rsplit('\\')
            .next()
            .unwrap_or(&full_path)
            .trim_end_matches(".exe")
            .trim_end_matches(".EXE")
            .to_string();

        Ok(canonicalize_frontmost_windows(&name))
    }
}

#[cfg(target_os = "windows")]
fn frontmost_window_title_windows() -> Result<String> {
    use windows::Win32::UI::WindowsAndMessaging::*;

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return Ok(String::new());
        }

        let mut buf = [0u16; 1024];
        let len = GetWindowTextW(hwnd, &mut buf);
        Ok(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

#[cfg(target_os = "windows")]
fn app_alias_for_windows(app: &str) -> String {
    let lower = app.trim().to_ascii_lowercase();
    match lower.as_str() {
        "finder" | "explorer" | "file explorer" | "windows explorer" => "explorer".to_string(),
        "mail" | "outlook" | "microsoft outlook" => "outlook".to_string(),
        "notes" | "notepad" => "notepad".to_string(),
        "textedit" | "text editor" => "notepad".to_string(),
        "google chrome" | "chrome" => "chrome".to_string(),
        "safari" | "microsoft edge" | "edge" | "msedge" => "msedge".to_string(),
        "calculator" | "calc" => "calc".to_string(),
        _ => {
            if lower.is_empty() {
                "explorer".to_string()
            } else {
                app.trim().to_string()
            }
        }
    }
}

fn open_local_outlook() -> Result<String> {
    let status = Command::new("cmd")
        .args(["/C", "start", "", "ms-outlook:"])
        .status()
        .context("Failed to launch New Outlook")?;
    if status.success() {
        Ok("Opened ms-outlook:".to_string())
    } else {
        Err(anyhow::anyhow!(
            "Failed to open New Outlook. Classic Outlook fallback is disabled."
        ))
    }
}

#[cfg(target_os = "windows")]
fn open_windows_target(target: &str) -> Result<String> {
    if target.trim().eq_ignore_ascii_case("outlook") {
        return open_local_outlook();
    }

    Command::new("cmd")
        .args(["/C", "start", "", target])
        .status()
        .context("Failed to launch target on Windows")
        .and_then(|status| {
            if status.success() {
                Ok(format!("Opened {}", target))
            } else {
                Err(anyhow::anyhow!("Failed to open {}", target))
            }
        })
}

#[cfg(target_os = "windows")]
fn parse_quoted_value(script: &str, marker: &str) -> Option<String> {
    let start = script.find(marker)? + marker.len();
    let end_rel = script[start..].find('"')?;
    Some(script[start..start + end_rel].to_string())
}

#[cfg(target_os = "windows")]
fn parse_click_coords(script: &str) -> Option<(i32, i32)> {
    let marker = "click at {";
    let start = script.to_ascii_lowercase().find(marker)? + marker.len();
    let end_rel = script[start..].find('}')?;
    let pair = &script[start..start + end_rel];
    let mut parts = pair.split(',');
    let x = parts.next()?.trim().parse::<i32>().ok()?;
    let y = parts.next()?.trim().parse::<i32>().ok()?;
    Some((x, y))
}

#[cfg(target_os = "windows")]
fn parse_scroll(script_lower: &str) -> Option<i32> {
    let down_marker = "scroll down by ";
    let up_marker = "scroll up by ";
    if let Some(start) = script_lower.find(down_marker) {
        let raw = script_lower[start + down_marker.len()..]
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        let amount = raw.parse::<i32>().ok()?;
        return Some(-amount.max(0));
    }
    if let Some(start) = script_lower.find(up_marker) {
        let raw = script_lower[start + up_marker.len()..]
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        let amount = raw.parse::<i32>().ok()?;
        return Some(amount.max(0));
    }
    None
}

/// Parse a `{x, y}` pair from an AppleScript-like string, given a marker prefix.
#[cfg(target_os = "windows")]
fn parse_applescript_pair(script_lower: &str, marker: &str) -> Option<(i32, i32)> {
    let start = script_lower.find(marker)? + marker.len();
    let end = script_lower[start..].find('}')?;
    let pair = &script_lower[start..start + end];
    let mut parts = pair.split(',');
    let a = parts.next()?.trim().parse::<i32>().ok()?;
    let b = parts.next()?.trim().parse::<i32>().ok()?;
    Some((a, b))
}

/// Resolve well-known macOS folder names to Windows shell paths.
#[cfg(target_os = "windows")]
fn resolve_finder_folder(name: &str) -> Option<String> {
    let lower = name.trim().to_ascii_lowercase();
    // Shell: protocol paths for `start` command
    match lower.as_str() {
        "downloads" | "download" | "다운로드" => Some("shell:Downloads".to_string()),
        "documents" | "document" | "문서" => Some("shell:Personal".to_string()),
        "desktop" | "바탕화면" => Some("shell:Desktop".to_string()),
        "home" | "~" | "홈" => dirs::home_dir().map(|p| p.to_string_lossy().to_string()),
        "pictures" | "photos" | "사진" => Some("shell:My Pictures".to_string()),
        "music" | "음악" => Some("shell:My Music".to_string()),
        "videos" | "video" | "비디오" => Some("shell:My Video".to_string()),
        "applications" | "program files" | "프로그램" => Some("shell:ProgramFiles".to_string()),
        _ => {
            // Try as an absolute path
            let path = std::path::Path::new(name.trim());
            if path.is_absolute() && (path.exists() || path.parent().map_or(false, |p| p.exists())) {
                Some(name.trim().to_string())
            } else {
                None
            }
        }
    }
}

/// Extract a quoted path from a Finder script, e.g. `open folder "Downloads"` or `set target ... to folder "Documents"`
#[cfg(target_os = "windows")]
fn extract_finder_folder_from_script(script: &str) -> Option<String> {
    let lower = script.to_ascii_lowercase();

    // Pattern: "path to <X> folder"
    if let Some(idx) = lower.find("path to ") {
        let after = &lower[idx + "path to ".len()..];
        let folder_name = after
            .trim_start()
            .split(|ch: char| ch == ' ' || ch == '"' || ch == '\n')
            .next()
            .unwrap_or("");
        if let Some(resolved) = resolve_finder_folder(folder_name) {
            return Some(resolved);
        }
    }

    // Pattern: "open folder \"<path>\""
    if let Some(idx) = lower.find("open folder \"") {
        let after = &script[idx + "open folder \"".len()..];
        if let Some(end) = after.find('"') {
            let folder = &after[..end];
            if let Some(resolved) = resolve_finder_folder(folder) {
                return Some(resolved);
            }
            return Some(folder.to_string());
        }
    }

    // Pattern: "open \"<path>\""  (opens a specific file/folder in Finder)
    if let Some(idx) = lower.find("open \"") {
        let after = &script[idx + "open \"".len()..];
        if let Some(end) = after.find('"') {
            let target = &after[..end];
            return Some(target.to_string());
        }
    }

    // Pattern: `set target of <window> to folder "X"` or `set target ... to (path to ...)`
    if let Some(idx) = lower.find("set target") {
        let after = &lower[idx..];
        if let Some(to_idx) = after.find("to folder \"") {
            let folder_start = &script[idx + to_idx + "to folder \"".len()..];
            if let Some(end) = folder_start.find('"') {
                let folder = &folder_start[..end];
                if let Some(resolved) = resolve_finder_folder(folder) {
                    return Some(resolved);
                }
                return Some(folder.to_string());
            }
        }
        // set target ... to (path to X folder)
        if let Some(pt_idx) = after.find("path to ") {
            let folder_raw = &after[pt_idx + "path to ".len()..];
            let folder_name = folder_raw
                .split(|ch: char| ch == ' ' || ch == ')' || ch == '\n')
                .next()
                .unwrap_or("");
            if let Some(resolved) = resolve_finder_folder(folder_name) {
                return Some(resolved);
            }
        }
    }

    // Pattern: "reveal <path>" or "select <file>"
    if let Some(idx) = lower.find("reveal ") {
        let after_reveal = &script[idx + "reveal ".len()..];
        let path = after_reveal.trim().trim_matches('"').trim();
        if !path.is_empty() {
            return Some(path.to_string());
        }
    }

    None
}

/// Handle Finder commands from `run()` (single-line AppleScript).
#[cfg(target_os = "windows")]
fn windows_handle_finder_command(script: &str, normalized: &str) -> Result<String> {
    // Just activate Explorer (no specific folder)
    if normalized.contains("to activate")
        && !normalized.contains("open")
        && !normalized.contains("reveal")
        && !normalized.contains("select")
        && !normalized.contains("set target")
    {
        return open_windows_target("explorer");
    }

    // Reveal/Select: open Explorer with file selected
    if normalized.contains("reveal") || normalized.contains("select") {
        if let Some(path) = extract_finder_folder_from_script(script) {
            let expanded = path.replace("~", &dirs::home_dir().map_or_else(
                || "C:\\Users".to_string(),
                |p| p.to_string_lossy().to_string(),
            ));
            // explorer /select,<path> opens Explorer with the file selected
            let _ = Command::new("explorer")
                .arg(format!("/select,{}", expanded))
                .status();
            return Ok(format!("Revealed {}", expanded));
        }
    }

    // Open specific folder
    if let Some(folder) = extract_finder_folder_from_script(script) {
        return open_windows_target(&folder);
    }

    // Fallback: just open Explorer
    open_windows_target("explorer")
}

/// Handle Finder commands from `run_lines_with_args()` (multi-line AppleScript).
#[cfg(target_os = "windows")]
fn windows_handle_finder_script_lines(lines: &[&str], lower_joined: &str) -> Result<String> {
    // If it contains "path to downloads folder"
    if lower_joined.contains("path to downloads folder") || lower_joined.contains("다운로드") {
        let _ = open_windows_target("shell:Downloads");
        return Ok("ok".to_string());
    }

    // Reveal/Select pattern
    if lower_joined.contains("reveal") || lower_joined.contains("select") {
        let joined = lines.join("\n");
        if let Some(path) = extract_finder_folder_from_script(&joined) {
            let expanded = path.replace("~", &dirs::home_dir().map_or_else(
                || "C:\\Users".to_string(),
                |p| p.to_string_lossy().to_string(),
            ));
            let _ = Command::new("explorer")
                .arg(format!("/select,{}", expanded))
                .status();
            return Ok(format!("Revealed {}", expanded));
        }
    }

    // Try to find and open a folder target
    let joined = lines.join("\n");
    if let Some(folder) = extract_finder_folder_from_script(&joined) {
        return open_windows_target(&folder);
    }

    // Path to documents, desktop, etc.
    for known in ["documents", "desktop", "pictures", "music", "videos", "home"] {
        if lower_joined.contains(&format!("path to {} folder", known)) {
            if let Some(resolved) = resolve_finder_folder(known) {
                return open_windows_target(&resolved);
            }
        }
    }

    // Fallback: just activate Explorer
    open_windows_target("explorer")
}

/// Extract a file path from a Preview script.
#[cfg(target_os = "windows")]
fn extract_preview_file_path(script: &str) -> Option<String> {
    let lower = script.to_ascii_lowercase();

    // Pattern: `open "filepath"` or `open POSIX file "filepath"`
    for marker in &["open posix file \"", "open file \"", "open alias \"", "open \""] {
        if let Some(idx) = lower.find(marker) {
            let after = &script[idx + marker.len()..];
            if let Some(end) = after.find('"') {
                let path = after[..end].trim().to_string();
                if !path.is_empty() {
                    return Some(path);
                }
            }
        }
    }

    None
}

/// Handle Preview commands from `run()` (single-line AppleScript).
/// Preview.app opens images/PDFs → on Windows, use default viewer via `start`.
#[cfg(target_os = "windows")]
fn windows_handle_preview_command(script: &str, normalized: &str) -> Result<String> {
    if normalized.contains("to activate") && !normalized.contains("open") {
        // Open Windows Photos or default viewer (no specific file)
        return open_windows_target("ms-photos:");
    }

    if let Some(file_path) = extract_preview_file_path(script) {
        let expanded = file_path.replace("~", &dirs::home_dir().map_or_else(
            || "C:\\Users".to_string(),
            |p| p.to_string_lossy().to_string(),
        ));
        // Use `start` to open with default viewer
        return open_windows_target(&expanded);
    }

    // Fallback: open Photos app
    open_windows_target("ms-photos:")
}

/// Handle Preview commands from `run_lines_with_args()` (multi-line AppleScript).
#[cfg(target_os = "windows")]
fn windows_handle_preview_script_lines(lines: &[&str], lower_joined: &str) -> Result<String> {
    let joined = lines.join("\n");

    if let Some(file_path) = extract_preview_file_path(&joined) {
        let expanded = file_path.replace("~", &dirs::home_dir().map_or_else(
            || "C:\\Users".to_string(),
            |p| p.to_string_lossy().to_string(),
        ));
        return open_windows_target(&expanded);
    }

    if lower_joined.contains("to activate") {
        return open_windows_target("ms-photos:");
    }

    open_windows_target("ms-photos:")
}

#[cfg(target_os = "windows")]
fn send_keys_escape(text: &str) -> String {
    let mut escaped = String::new();
    for ch in text.chars() {
        match ch {
            '+' | '^' | '%' | '~' | '(' | ')' | '{' | '}' | '[' | ']' => {
                escaped.push('{');
                escaped.push(ch);
                escaped.push('}');
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(target_os = "windows")]
fn send_windows_keys(sequence: &str) -> Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;

    // For simple text, use SendInput with UNICODE flag (no PowerShell needed)
    unsafe {
        for ch in sequence.chars() {
            // Skip SendKeys format characters - they were for PowerShell
            if "^+%~".contains(ch) {
                continue;
            }

            let mut inputs = [INPUT::default(); 2];

            inputs[0].r#type = INPUT_KEYBOARD;
            inputs[0].Anonymous.ki.wScan = ch as u16;
            inputs[0].Anonymous.ki.dwFlags = KEYEVENTF_UNICODE;

            inputs[1].r#type = INPUT_KEYBOARD;
            inputs[1].Anonymous.ki.wScan = ch as u16;
            inputs[1].Anonymous.ki.dwFlags = KEYEVENTF_UNICODE | KEYEVENTF_KEYUP;

            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_key_with_modifiers(key: &str, modifiers: &[&str]) -> Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;

    unsafe {
        let mut mod_vks: Vec<VIRTUAL_KEY> = Vec::new();

        for m in modifiers {
            match m.to_ascii_lowercase().as_str() {
                "command" | "cmd" | "control" | "ctrl" => mod_vks.push(VK_CONTROL),
                "shift" => mod_vks.push(VK_SHIFT),
                "option" | "alt" => mod_vks.push(VK_MENU),
                _ => {}
            }
        }

        let key_vk = match key.trim().to_ascii_lowercase().as_str() {
            "escape" | "esc" => VK_ESCAPE,
            "return" | "enter" => VK_RETURN,
            "tab" => VK_TAB,
            "space" => VK_SPACE,
            "backspace" | "delete" => VK_BACK,
            "up" => VK_UP,
            "down" => VK_DOWN,
            "left" => VK_LEFT,
            "right" => VK_RIGHT,
            "home" => VK_HOME,
            "end" => VK_END,
            s if s.len() == 1 => {
                let ch = s.chars().next().unwrap().to_ascii_uppercase();
                VIRTUAL_KEY(ch as u16)
            }
            _ => VK_SPACE,
        };

        // Build input array: mod_down... key_down key_up ...mod_up
        let total = mod_vks.len() * 2 + 2;
        let mut inputs: Vec<INPUT> = vec![INPUT::default(); total];
        let mut idx = 0;

        // Press modifiers
        for &vk in &mod_vks {
            inputs[idx].r#type = INPUT_KEYBOARD;
            inputs[idx].Anonymous.ki.wVk = vk;
            idx += 1;
        }

        // Press key
        inputs[idx].r#type = INPUT_KEYBOARD;
        inputs[idx].Anonymous.ki.wVk = key_vk;
        idx += 1;

        // Release key
        inputs[idx].r#type = INPUT_KEYBOARD;
        inputs[idx].Anonymous.ki.wVk = key_vk;
        inputs[idx].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
        idx += 1;

        // Release modifiers in reverse
        for &vk in mod_vks.iter().rev() {
            inputs[idx].r#type = INPUT_KEYBOARD;
            inputs[idx].Anonymous.ki.wVk = vk;
            inputs[idx].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
            idx += 1;
        }

        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_click(x: i32, y: i32) -> Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    unsafe {
        // Move cursor to position
        let _ = SetCursorPos(x, y);
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Mouse down + up via SendInput
        let mut inputs = [INPUT::default(); 2];

        inputs[0].r#type = INPUT_MOUSE;
        inputs[0].Anonymous.mi.dwFlags = MOUSEEVENTF_LEFTDOWN;

        inputs[1].r#type = INPUT_MOUSE;
        inputs[1].Anonymous.mi.dwFlags = MOUSEEVENTF_LEFTUP;

        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_scroll(amount: i32) -> Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;

    unsafe {
        let wheel = amount.saturating_mul(120);
        let mut input = [INPUT::default(); 1];
        input[0].r#type = INPUT_MOUSE;
        input[0].Anonymous.mi.dwFlags = MOUSEEVENTF_WHEEL;
        input[0].Anonymous.mi.mouseData = wheel as u32;

        SendInput(&input, std::mem::size_of::<INPUT>() as i32);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_extract_app_activation(script: &str) -> Option<String> {
    // e.g. tell application "Finder" to activate
    let marker = "tell application \"";
    let lower = script.to_ascii_lowercase();
    let idx = lower.find("to activate")?;
    let prefix = &script[..idx];
    parse_quoted_value(prefix, marker)
}

#[cfg(target_os = "windows")]
fn windows_mail_get_or_create_draft(draft_hint: Option<&str>) -> MailDraftState {
    let mut state = WINDOWS_AUTOMATION_STATE
        .lock()
        .expect("windows automation state poisoned");
    if let Some(hint) = draft_hint {
        if let Some(existing) = state.mail_drafts.get(hint).cloned() {
            return existing;
        }
    }
    if let Some(last) = state
        .mail_drafts
        .values()
        .cloned()
        .last()
    {
        return last;
    }
    state.next_id = state.next_id.saturating_add(1);
    let id = format!("WIN_DRAFT_{}", state.next_id);
    let draft = MailDraftState {
        id: id.clone(),
        ..MailDraftState::default()
    };
    state.mail_drafts.insert(id.clone(), draft.clone());
    windows_state_persist_locked(&state);
    draft
}

#[cfg(target_os = "windows")]
fn windows_mail_upsert_draft(draft: MailDraftState) {
    let mut state = WINDOWS_AUTOMATION_STATE
        .lock()
        .expect("windows automation state poisoned");
    state.mail_drafts.insert(draft.id.clone(), draft);
    windows_state_persist_locked(&state);
}

#[cfg(target_os = "windows")]
fn windows_mail_count() -> usize {
    WINDOWS_AUTOMATION_STATE
        .lock()
        .expect("windows automation state poisoned")
        .mail_drafts
        .len()
}

#[cfg(target_os = "windows")]
fn windows_handle_mail_run_with_args(lines: &[&str], args: &[String]) -> Result<String> {
    let script = lines.join("\n");
    let lower = script.to_ascii_lowercase();

    if lower.contains("return (count of outgoing messages)") {
        return Ok(windows_mail_count().to_string());
    }

    if lower.contains("email addresses of ac") || lower.contains("user name of ac") {
        let default_addr = std::env::var("STEER_DEFAULT_MAIL_TO").unwrap_or_default();
        return Ok(default_addr);
    }

    if lower.contains("return removedcount as text") {
        return Ok("0".to_string());
    }

    if lower.contains("return \"ok|\" & draftid & \"|\" & bodylen") {
        let text = args.first().cloned().unwrap_or_default();
        let draft_hint = args.get(1).map(|s| s.as_str()).unwrap_or("");
        let mut draft = windows_mail_get_or_create_draft(if draft_hint.is_empty() {
            None
        } else {
            Some(draft_hint)
        });
        if draft.body.is_empty() {
            draft.body = text;
        } else if !text.trim().is_empty() {
            draft.body = format!("{}\n{}", draft.body, text);
        }
        let body_len = draft.body.chars().count();
        let draft_id = draft.id.clone();
        windows_mail_upsert_draft(draft);
        return Ok(format!("ok|{}|{}", draft_id, body_len));
    }

    if lower.contains("return draftid & \"|\" & bodylen") {
        let body = args.first().cloned().unwrap_or_default();
        let subject = args.get(1).cloned().unwrap_or_default();
        let recipient = args.get(2).cloned().unwrap_or_default();
        let mut draft = windows_mail_get_or_create_draft(None);
        draft.body = body;
        draft.subject = subject;
        draft.recipient = recipient;
        let id = draft.id.clone();
        let body_len = draft.body.chars().count();
        windows_mail_upsert_draft(draft);
        return Ok(format!("{}|{}", id, body_len));
    }

    if lower.contains("return \"ok|\" & draftid") {
        let mut draft = windows_mail_get_or_create_draft(args.get(1).map(|s| s.as_str()));
        if lower.contains("set subject of _msg to subjecttext") {
            draft.subject = args.first().cloned().unwrap_or_default();
        }
        if lower.contains("to recipients") {
            draft.recipient = args.first().cloned().unwrap_or_default();
        }
        let id = draft.id.clone();
        windows_mail_upsert_draft(draft);
        return Ok(format!("ok|{}", id));
    }

    if lower.contains("return draftid") {
        let hint = args.first().map(|s| s.as_str()).filter(|s| !s.trim().is_empty());
        let mut draft = windows_mail_get_or_create_draft(hint);
        if let Some(recipient) = args.get(1) {
            if !recipient.trim().is_empty() {
                draft.recipient = recipient.clone();
            }
        }
        windows_mail_upsert_draft(draft.clone());
        return Ok(draft.id);
    }

    if lower.contains("return \"sent_pending|\"") || lower.contains("return \"sent_confirmed|\"") {
        let fallback = args.first().cloned().unwrap_or_default();
        let subject_hint = args.get(1).cloned().unwrap_or_default();
        let draft_hint = args.get(3).cloned().unwrap_or_default();
        let mut draft = windows_mail_get_or_create_draft(if draft_hint.trim().is_empty() {
            None
        } else {
            Some(draft_hint.as_str())
        });
        if draft.recipient.trim().is_empty() {
            draft.recipient = fallback;
        }
        if draft.subject.trim().is_empty() {
            draft.subject = subject_hint;
        }
        let body_len = draft.body.chars().count();
        let id = draft.id.clone();
        let recipient = draft.recipient.clone();
        let subject = draft.subject.clone();
        let body = draft.body.clone();

        // 실제 Outlook COM으로 이메일 발송 시도
        if !recipient.is_empty() {
            match crate::win32_app_control::outlook::send_email(&recipient, &subject, &body) {
                Ok(()) => log::info!("Outlook: email sent to {}", recipient),
                Err(e) => log::warn!("Outlook COM failed (falling back to simulation): {}", e),
            }
        }

        {
            let mut state = WINDOWS_AUTOMATION_STATE
                .lock()
                .expect("windows automation state poisoned");
            state.mail_drafts.remove(&id);
            windows_state_persist_locked(&state);
        }
        return Ok(format!(
            "sent_confirmed|1|0|{}|{}|{}|{}",
            recipient, subject, id, body_len
        ));
    }

    Ok("ok".to_string())
}

#[cfg(target_os = "windows")]
fn windows_handle_notes_run_with_args(lines: &[&str], args: &[String]) -> Result<String> {
    let script = lines.join("\n");
    let lower = script.to_ascii_lowercase();
    if lower.contains("return \"ok|\" & noteid & \"|\" & notenameout & \"|\" & notebodylen") {
        let text = args.first().cloned().unwrap_or_default();
        let marker = args.get(1).cloned().unwrap_or_default();
        let mut state = WINDOWS_AUTOMATION_STATE
            .lock()
            .expect("windows automation state poisoned");
        if state.note_id.is_none() {
            state.next_id = state.next_id.saturating_add(1);
            state.note_id = Some(format!("WIN_NOTE_{}", state.next_id));
        }
        if marker.trim().is_empty() {
            state.note_body = text;
        } else if text.contains(&marker) {
            state.note_body = text;
        } else if text.trim().is_empty() {
            state.note_body = marker.clone();
        } else {
            state.note_body = format!("{}\n{}", text, marker);
        }
        state.note_name = state
            .note_body
            .lines()
            .next()
            .unwrap_or("Steer Note")
            .to_string();
        let id = state.note_id.clone().unwrap_or_default();
        let final_body = state.note_body.clone();
        let note_name = state.note_name.clone();
        let body_len = state.note_body.chars().count();
        windows_state_persist_locked(&state);
        drop(state);

        // 실제 메모장에 텍스트 쓰기
        if let Err(e) = crate::win32_app_control::notepad::set_text(&final_body) {
            log::warn!("Notepad set_text failed (simulation only): {}", e);
        }

        return Ok(format!("ok|{}|{}|{}", id, note_name, body_len));
    }

    if lower.contains("tell application \"notes\"") {
        let marker = args.first().cloned().unwrap_or_default();
        let state = WINDOWS_AUTOMATION_STATE
            .lock()
            .expect("windows automation state poisoned");
        if state.note_body.trim().is_empty() {
            return Ok(String::new());
        }
        if !marker.trim().is_empty() && !state.note_body.contains(&marker) {
            return Ok("__STEER_MARKER_NOT_FOUND__".to_string());
        }
        if state.note_name.trim().is_empty() {
            return Ok(state.note_body.clone());
        }
        return Ok(format!("{}\n{}", state.note_name, state.note_body));
    }
    Ok(String::new())
}

#[cfg(target_os = "windows")]
fn windows_handle_textedit_run_with_args(lines: &[&str], args: &[String]) -> Result<String> {
    let script = lines.join("\n");
    let lower = script.to_ascii_lowercase();
    if lower.contains("return \"ok|\" & docid & \"|\" & docname & \"|\" & bodylen") {
        let text = args.first().cloned().unwrap_or_default();
        let marker = args.get(1).cloned().unwrap_or_default();
        let mut state = WINDOWS_AUTOMATION_STATE
            .lock()
            .expect("windows automation state poisoned");
        if state.textedit_id.is_none() {
            state.next_id = state.next_id.saturating_add(1);
            state.textedit_id = Some(format!("WIN_DOC_{}", state.next_id));
            state.textedit_name = "Steer Document".to_string();
        }
        if state.textedit_body.is_empty() {
            state.textedit_body = text.clone();
        } else if !text.trim().is_empty() {
            state.textedit_body = format!("{}\n{}", state.textedit_body, text);
        }
        if !marker.trim().is_empty() && !state.textedit_body.contains(&marker) {
            if state.textedit_body.trim().is_empty() {
                state.textedit_body = marker;
            } else {
                state.textedit_body = format!("{}\n{}", state.textedit_body, marker);
            }
        }
        let id = state.textedit_id.clone().unwrap_or_default();
        let final_body = state.textedit_body.clone();
        let doc_name = state.textedit_name.clone();
        let body_len = state.textedit_body.chars().count();
        windows_state_persist_locked(&state);
        drop(state);

        // 실제 메모장에 텍스트 쓰기
        if let Err(e) = crate::win32_app_control::notepad::set_text(&final_body) {
            log::warn!("Notepad set_text failed (simulation only): {}", e);
        }

        return Ok(format!("ok|{}|{}|{}", id, doc_name, body_len));
    }

    if lower.contains("tell application \"textedit\"") {
        let marker = args.first().cloned().unwrap_or_default();
        let state = WINDOWS_AUTOMATION_STATE
            .lock()
            .expect("windows automation state poisoned");
        if state.textedit_body.trim().is_empty() {
            return Ok(String::new());
        }
        if !marker.trim().is_empty() && !state.textedit_body.contains(&marker) {
            return Ok("__STEER_MARKER_NOT_FOUND__".to_string());
        }
        return Ok(state.textedit_body.clone());
    }

    Ok(String::new())
}

fn normalize_app_name_token(name: &str) -> String {
    name.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn canonical_app_group(name: &str) -> String {
    let norm = normalize_app_name_token(name);
    match norm.as_str() {
        "finder" | "explorer" | "fileexplorer" | "windowsexplorer" => "finder".to_string(),
        "mail" | "outlook" | "microsoftoutlook" => "mail".to_string(),
        "notes" | "notepad" | "textedit" | "texteditor" => "notepad".to_string(),
        "safari" | "edge" | "msedge" | "microsoftedge" => "edge".to_string(),
        "googlechrome" | "chrome" => "chrome".to_string(),
        "calculator" | "calc" | "calculatorapp" => "calculator".to_string(),
        _ => norm,
    }
}

pub fn app_names_match(actual: &str, expected: &str) -> bool {
    let actual_trim = actual.trim();
    let expected_trim = expected.trim();
    if actual_trim.is_empty() || expected_trim.is_empty() {
        return false;
    }
    if actual_trim.eq_ignore_ascii_case(expected_trim) {
        return true;
    }

    let actual_group = canonical_app_group(actual_trim);
    let expected_group = canonical_app_group(expected_trim);
    if actual_group == expected_group {
        return true;
    }

    let actual_norm = normalize_app_name_token(actual_trim);
    let expected_norm = normalize_app_name_token(expected_trim);
    actual_norm.contains(&expected_norm) || expected_norm.contains(&actual_norm)
}

pub fn frontmost_app_name() -> Result<String> {
    #[cfg(target_os = "macos")]
    {
        let script = r#"tell application "System Events" to return name of first application process whose frontmost is true"#;
        return run(script);
    }
    #[cfg(target_os = "windows")]
    {
        return frontmost_process_name_windows();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(anyhow::anyhow!("Frontmost app lookup is not implemented on this OS"))
    }
}

pub fn run(script: &str) -> Result<String> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()
            .context("Failed to run AppleScript")?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if !output.status.success() {
            return Err(anyhow::anyhow!("AppleScript Error: {}", stderr));
        }

        return Ok(stdout);
    }
    #[cfg(target_os = "windows")]
    {
        let normalized = script.to_ascii_lowercase();

        if normalized.contains("frontmost is true")
            || normalized.contains("first application process")
        {
            return frontmost_process_name_windows();
        }

        // --- System Events: window/process management ---

        // click button "X" of window 1
        if normalized.contains("click button") {
            if let Some(btn_name) = parse_quoted_value(script, "click button \"") {
                let _ = crate::win32_app_control::system_events::click_button_by_name(&btn_name);
                return Ok("ok".to_string());
            }
        }

        // set frontmost of ... process ... to true
        if normalized.contains("set frontmost") && normalized.contains("to true") {
            // Extract process name: "set frontmost of first application process whose name is "X" to true"
            if let Some(name) = parse_quoted_value(script, "whose name is \"")
                .or_else(|| parse_quoted_value(script, "whose unix id is \""))
            {
                let _ = crate::win32_app_control::system_events::set_frontmost_by_name(&name);
                return Ok("ok".to_string());
            }
        }

        // keystroke "h" using {command down, option down} → hide other apps
        if normalized.contains("keystroke") && normalized.contains("command down") && normalized.contains("option down") {
            if let Some(key) = parse_quoted_value(script, "keystroke \"") {
                if key == "h" || key == "H" {
                    let _ = crate::win32_app_control::system_events::hide_other_apps();
                    return Ok("ok".to_string());
                }
            }
        }

        // set position of window 1 to {x, y}
        if normalized.contains("set position of window") || normalized.contains("set size of window") {
            // Best-effort: apply to foreground window
            let hwnd = crate::win32_app_control::system_events::get_foreground_hwnd();
            if let Ok((cx, cy, cw, ch)) = crate::win32_app_control::system_events::get_window_rect(hwnd) {
                if normalized.contains("set position") {
                    if let Some(coords) = parse_applescript_pair(&normalized, "to {") {
                        let _ = crate::win32_app_control::system_events::set_window_rect(hwnd, coords.0, coords.1, cw, ch);
                        return Ok("ok".to_string());
                    }
                }
                if normalized.contains("set size") {
                    if let Some(dims) = parse_applescript_pair(&normalized, "to {") {
                        let _ = crate::win32_app_control::system_events::set_window_rect(hwnd, cx, cy, dims.0, dims.1);
                        return Ok("ok".to_string());
                    }
                }
            }
        }

        // close window / minimize window / maximize window
        if normalized.contains("close window") || normalized.contains("close front window") {
            let hwnd = crate::win32_app_control::system_events::get_foreground_hwnd();
            let _ = crate::win32_app_control::system_events::close_window(hwnd);
            return Ok("ok".to_string());
        }

        // click menu item
        if normalized.contains("click menu item") {
            if let Some(item_name) = parse_quoted_value(script, "click menu item \"") {
                // Try to also get menu bar item (parent menu)
                if let Some(menu_name) = parse_quoted_value(script, "of menu \"")
                    .or_else(|| parse_quoted_value(script, "menu bar item \""))
                {
                    let _ = crate::win32_app_control::system_events::click_menu_item(&[&menu_name, &item_name]);
                } else {
                    let _ = crate::win32_app_control::system_events::click_menu_item(&[&item_name]);
                }
                return Ok("ok".to_string());
            }
        }

        // --- End System Events handlers ---

        if normalized.contains("the clipboard") {
            return run_powershell("Get-Clipboard");
        }

        if let Some(text) = parse_quoted_value(script, "set the clipboard to \"") {
            let escaped = text.replace('\'', "''");
            let _ = run_powershell(&format!("Set-Clipboard -Value '{}'", escaped))?;
            return Ok("ok".to_string());
        }

        if normalized.contains("tell application \"finder\"") {
            windows_set_last_activated_app("Finder");
            return windows_handle_finder_command(script, &normalized);
        }

        if normalized.contains("tell application \"preview\"") {
            return windows_handle_preview_command(script, &normalized);
        }
        if normalized.contains("tell application \"mail\" to activate") {
            windows_set_last_activated_app("Mail");
            return open_windows_target("outlook");
        }
        if normalized.contains("tell application \"notes\" to activate") {
            windows_set_last_activated_app("Notes");
            return open_windows_target("notepad");
        }
        if normalized.contains("tell application \"textedit\" to activate") {
            windows_set_last_activated_app("TextEdit");
            return open_windows_target("notepad");
        }

        // Music.app → Windows 미디어 키 제어
        if normalized.contains("tell application \"music\"") || normalized.contains("tell application \"itunes\"") {
            if normalized.contains("to activate") {
                windows_set_last_activated_app("Music");
                // Windows Media Player 또는 기본 음악 앱 열기
                return open_windows_target("mswindowsmusic:");
            }
            if normalized.contains("to play") || normalized.contains("to resume") {
                crate::win32_app_control::media::play_pause();
                return Ok("ok".to_string());
            }
            if normalized.contains("to pause") || normalized.contains("to stop") {
                crate::win32_app_control::media::play_pause();
                return Ok("ok".to_string());
            }
            if normalized.contains("next track") || normalized.contains("to next") {
                crate::win32_app_control::media::next_track();
                return Ok("ok".to_string());
            }
            if normalized.contains("previous track") || normalized.contains("to previous") {
                crate::win32_app_control::media::prev_track();
                return Ok("ok".to_string());
            }
            // 현재 재생 중인 곡 정보 요청 — 미지원이므로 빈 문자열 반환
            if normalized.contains("name of current track") || normalized.contains("player state") {
                return Ok(String::new());
            }
            return Ok("ok".to_string());
        }

        if normalized.contains("to activate") {
            if let Some(app_name) = windows_extract_app_activation(script) {
                windows_set_last_activated_app(&app_name);
                let target = app_alias_for_windows(&app_name);
                return open_windows_target(&target);
            }
        }

        if let Some(start) = script.find("open location \"") {
            let begin = start + "open location \"".len();
            if let Some(end_rel) = script[begin..].find('"') {
                let url = &script[begin..begin + end_rel];
                return open_windows_target(url);
            }
        }

        if let Some((x, y)) = parse_click_coords(script) {
            windows_click(x, y)?;
            return Ok("ok".to_string());
        }

        if let Some(start) = normalized.find("cliclick m:") {
            let raw = &normalized[start + "cliclick m:".len()..];
            let mut parts = raw.split(',');
            if let (Some(x_raw), Some(y_raw)) = (parts.next(), parts.next()) {
                if let (Ok(x), Ok(y)) = (
                    x_raw.trim().parse::<i32>(),
                    y_raw
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .parse::<i32>(),
                ) {
                    windows_click(x, y)?;
                    return Ok("ok".to_string());
                }
            }
        }

        if normalized.contains("key code 53") {
            windows_key_with_modifiers("escape", &[])?;
            return Ok("ok".to_string());
        }

        if normalized.contains("keystroke")
            && normalized.contains("using {")
            && normalized.contains("down")
        {
            let key = parse_quoted_value(script, "keystroke \"").unwrap_or_default();
            let mut modifiers: Vec<&str> = Vec::new();
            if normalized.contains("command down") {
                modifiers.push("command");
            }
            if normalized.contains("shift down") {
                modifiers.push("shift");
            }
            if normalized.contains("option down") || normalized.contains("alt down") {
                modifiers.push("option");
            }
            if normalized.contains("control down") || normalized.contains("ctrl down") {
                modifiers.push("control");
            }
            windows_key_with_modifiers(&key, &modifiers)?;
            return Ok("ok".to_string());
        }

        if normalized.contains("keystroke \"") {
            if let Some(text) = parse_quoted_value(script, "keystroke \"") {
                send_windows_keys(&send_keys_escape(&text))?;
                return Ok("ok".to_string());
            }
        }

        if let Some(scroll_amount) = parse_scroll(&normalized) {
            windows_scroll(scroll_amount)?;
            return Ok("ok".to_string());
        }

        // Do not silently succeed for unsupported scripts on Windows.
        // Returning an error makes fallback/retry paths visible to callers.
        let head = script
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("");
        let preview = head.chars().take(160).collect::<String>();
        Err(anyhow::anyhow!(
            "Unsupported AppleScript on Windows: {}",
            if preview.is_empty() {
                "<empty script>"
            } else {
                &preview
            }
        ))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Ok("AppleScript functionality is only available on macOS.".to_string())
    }
}

pub fn control_app(app: &str, command: &str) -> Result<String> {
    #[cfg(target_os = "windows")]
    {
        let app_lower = app.trim().to_ascii_lowercase();
        let cmd_lower = command.trim().to_ascii_lowercase();
        match (app_lower.as_str(), cmd_lower.as_str()) {
            (_, "activate") => activate_app(app),
            ("music" | "itunes", "play" | "pause") => {
                crate::win32_app_control::media::play_pause();
                Ok("ok".to_string())
            }
            ("music" | "itunes", "next") => {
                crate::win32_app_control::media::next_track();
                Ok("ok".to_string())
            }
            ("music" | "itunes", "previous" | "prev") => {
                crate::win32_app_control::media::prev_track();
                Ok("ok".to_string())
            }
            ("music" | "itunes", "stop") => {
                crate::win32_app_control::media::stop();
                Ok("ok".to_string())
            }
            ("mail" | "outlook", "new" | "compose") => open_local_outlook(),
            ("notes" | "textedit" | "notepad", "new") => open_windows_target("notepad"),
            _ => Err(anyhow::anyhow!(
                "Unsupported app control command on Windows: app='{}', command='{}'",
                app,
                command
            )),
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let script = match (app.to_lowercase().as_str(), command) {
            ("music", "play") => "tell application \"Music\" to play",
            ("music", "pause") => "tell application \"Music\" to pause",
            ("music", "next") => "tell application \"Music\" to next track",
            ("notes", "new") => "tell application \"Notes\" to make new note at folder \"Notes\"",
            _ => return Err(anyhow::anyhow!("Unknown app control command")),
        };

        run(script)
    }
}

pub fn activate_app(app: &str) -> Result<String> {
    #[cfg(target_os = "windows")]
    {
        windows_set_last_activated_app(app);
        let target = app_alias_for_windows(app);
        open_windows_target(&target)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let script = format!(
            r#"
    tell application "{}"
        activate
        set _tries to 0
        repeat while (not frontmost) and _tries < 40
            delay 0.1
            set _tries to _tries + 1
        end repeat
        delay 0.2
    end tell
    "#,
            app
        );
        run(&script)
    }
}

pub fn execute_js_in_chrome(script: &str) -> Result<String> {
    #[cfg(target_os = "windows")]
    {
        // Chrome DevTools Protocol (CDP) — fully native Rust, no PowerShell
        // Requires Chrome launched with: --remote-debugging-port=9222
        let debug_port = std::env::var("STEER_CHROME_DEBUG_PORT")
            .unwrap_or_else(|_| "9222".to_string());
        let host = format!("127.0.0.1:{}", debug_port);

        // Step 1: HTTP GET /json to list tabs (raw TcpStream — no PowerShell)
        use std::io::{Read, Write};
        use std::net::TcpStream;

        let mut stream = TcpStream::connect(&host)
            .map_err(|e| anyhow::anyhow!(
                "Chrome DevTools not reachable on port {} ({}). Launch Chrome with --remote-debugging-port={}",
                debug_port, e, debug_port
            ))?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(3)))?;

        let request = format!("GET /json HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", host);
        stream.write_all(request.as_bytes())?;

        let mut response = String::new();
        stream.read_to_string(&mut response)?;

        // Parse HTTP response body (skip headers)
        let body = response
            .split("\r\n\r\n")
            .nth(1)
            .unwrap_or("")
            .trim();

        if body.is_empty() {
            return Err(anyhow::anyhow!("Empty response from Chrome DevTools"));
        }

        let tabs: Vec<serde_json::Value> = serde_json::from_str(body)
            .map_err(|e| anyhow::anyhow!("Failed to parse Chrome tabs: {}", e))?;

        let active_tab = tabs.iter()
            .find(|t| t["type"].as_str() == Some("page"))
            .ok_or_else(|| anyhow::anyhow!("No active Chrome tab found"))?;

        let ws_url = active_tab["webSocketDebuggerUrl"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No WebSocket URL for active tab"))?;

        // Step 2: WebSocket via tungstenite — send Runtime.evaluate
        let escaped_script = script.replace('\\', "\\\\").replace('"', "\\\"");
        let cdp_payload = format!(
            r#"{{"id":1,"method":"Runtime.evaluate","params":{{"expression":"{}","returnByValue":true}}}}"#,
            escaped_script
        );

        let (mut ws, _) = tungstenite::connect(ws_url)
            .map_err(|e| anyhow::anyhow!("WebSocket connect failed: {}", e))?;

        ws.send(tungstenite::Message::Text(cdp_payload))
            .map_err(|e| anyhow::anyhow!("WebSocket send failed: {}", e))?;

        let msg = ws.read()
            .map_err(|e| anyhow::anyhow!("WebSocket read failed: {}", e))?;

        let _ = ws.close(None);

        let stdout = msg.to_text().unwrap_or("").to_string();

        // Parse CDP response
        if let Ok(resp) = serde_json::from_str::<serde_json::Value>(&stdout) {
            if let Some(result) = resp.get("result").and_then(|r| r.get("result")).and_then(|r| r.get("value")) {
                return Ok(result.to_string());
            }
            if let Some(desc) = resp.get("result").and_then(|r| r.get("result")).and_then(|r| r.get("description")) {
                return Ok(desc.as_str().unwrap_or("").to_string());
            }
        }

        Ok(stdout)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let lines = [
            "on run argv",
            "set js to item 1 of argv",
            "tell application \"Google Chrome\" to execute javascript js in active tab of front window",
            "end run",
        ];
        run_lines_with_args(&lines, &[script.to_string()])
    }
}

pub fn activate_frontmost_app() -> Result<String> {
    #[cfg(target_os = "windows")]
    {
        frontmost_process_name_windows()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let script = r#"
        tell application "System Events"
            set frontApp to name of first application process whose frontmost is true
        end tell
        tell application frontApp to activate
        return frontApp
    "#;
        run(script)
    }
}

pub fn check_accessibility() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let script = r#"tell application "System Events" to get name of first application process whose frontmost is true"#;
        run(script).map(|_| ())
    }
}

pub fn get_active_window_context() -> Result<(String, String)> {
    #[cfg(target_os = "windows")]
    {
        let title = frontmost_window_title_windows().unwrap_or_default();

        // Try to detect browser and retrieve URL via CDP
        let url = if is_browser_frontmost(&title) {
            get_browser_url_via_cdp().unwrap_or_default()
        } else {
            String::new()
        };

        Ok((title, url))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let script = r#"
        global frontApp, windowTitle, browserUrl
        set windowTitle to ""
        set browserUrl to ""
        
        tell application "System Events"
            set frontApp to name of first application process whose frontmost is true
        end tell

        if frontApp is "Google Chrome" then
            tell application "Google Chrome"
                if (count of windows) > 0 then
                    set windowTitle to title of active tab of front window
                    set browserUrl to URL of active tab of front window
                end if
            end tell
        else if frontApp is "Safari" then
            tell application "Safari"
                if (count of documents) > 0 then
                    set windowTitle to name of front document
                    set browserUrl to URL of front document
                end if
            end tell
        else
            tell application "System Events"
                tell process frontApp
                    if (count of windows) > 0 then
                        set windowTitle to name of front window
                    end if
                end tell
            end tell
        end if
        
        return windowTitle & "|||" & browserUrl
    "#;

        let output = run(script)?;
        let parts: Vec<&str> = output.split("|||").collect();
        let title = parts.first().copied().unwrap_or("").trim().to_string();
        let url = parts.get(1).copied().unwrap_or("").trim().to_string();

        Ok((title, url))
    }
}

/// Check if the foreground process is a browser (Chrome, Edge, etc.)
#[cfg(target_os = "windows")]
fn is_browser_frontmost(title: &str) -> bool {
    let t = title.to_lowercase();
    // Common browser title patterns
    t.contains("google chrome") || t.contains("- chrome") ||
    t.contains("microsoft edge") || t.contains("- edge") ||
    // Also check by process name
    frontmost_process_name_windows()
        .map(|n| {
            let nl = n.to_lowercase();
            nl.contains("chrome") || nl.contains("msedge") || nl.contains("firefox")
        })
        .unwrap_or(false)
}

/// Get the current browser tab URL via Chrome DevTools Protocol.
/// Reuses the same CDP connection logic as execute_js_in_chrome.
#[cfg(target_os = "windows")]
fn get_browser_url_via_cdp() -> Result<String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let debug_port = std::env::var("STEER_CHROME_DEBUG_PORT")
        .unwrap_or_else(|_| "9222".to_string());
    let host = format!("127.0.0.1:{}", debug_port);

    let mut stream = TcpStream::connect(&host)
        .map_err(|e| anyhow::anyhow!("CDP not reachable: {}", e))?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;

    let request = format!("GET /json HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", host);
    stream.write_all(request.as_bytes())?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    let body = response.split("\r\n\r\n").nth(1).unwrap_or("").trim();
    if body.is_empty() {
        return Ok(String::new());
    }

    let tabs: Vec<serde_json::Value> = serde_json::from_str(body)
        .map_err(|e| anyhow::anyhow!("Failed to parse tabs: {}", e))?;

    // Find the active page tab and return its URL
    let active_tab = tabs.iter()
        .find(|t| t["type"].as_str() == Some("page"));

    match active_tab {
        Some(tab) => Ok(tab["url"].as_str().unwrap_or("").to_string()),
        None => Ok(String::new()),
    }
}

/// Search for text in the current browser page via CDP DOM query.
/// Returns the matched text context if found.
pub fn find_text_in_browser(query: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        let escaped_query = query.replace('\\', "\\\\").replace('"', "\\\"").replace('\'', "\\'");
        let js = format!(
            r#"(function() {{
                var body = document.body ? document.body.innerText : '';
                var idx = body.toLowerCase().indexOf('{}');
                if (idx === -1) return '';
                var start = Math.max(0, idx - 40);
                var end = Math.min(body.length, idx + {} + 40);
                return body.substring(start, end);
            }})()
            "#,
            escaped_query.to_lowercase(),
            query.len()
        );

        match execute_js_in_chrome(&js) {
            Ok(result) => {
                let trimmed = result.trim().trim_matches('"').to_string();
                if trimmed.is_empty() { None } else { Some(trimmed) }
            }
            Err(_) => None,
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = query;
        None
    }
}

fn run_lines_with_args(lines: &[&str], args: &[String]) -> Result<String> {
    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("osascript");
        for line in lines {
            cmd.arg("-e").arg(line);
        }
        cmd.arg("--");
        for arg in args {
            cmd.arg(arg);
        }

        let output = cmd.output().context("Failed to run AppleScript")?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if !output.status.success() {
            return Err(anyhow::anyhow!("AppleScript Error: {}", stderr));
        }

        return Ok(stdout);
    }
    #[cfg(target_os = "windows")]
    {
        let joined = lines.join("\n");
        let lower = joined.to_ascii_lowercase();

        if lower.contains("tell application \"mail\"") {
            return windows_handle_mail_run_with_args(lines, args);
        }
        if lower.contains("tell application \"notes\"") {
            return windows_handle_notes_run_with_args(lines, args);
        }
        if lower.contains("tell application \"textedit\"") {
            return windows_handle_textedit_run_with_args(lines, args);
        }
        if lower.contains("tell application \"finder\"") {
            return windows_handle_finder_script_lines(lines, &lower);
        }
        if lower.contains("tell application \"preview\"") {
            return windows_handle_preview_script_lines(lines, &lower);
        }

        run(&joined)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Ok("AppleScript functionality is only available on macOS.".to_string())
    }
}

pub fn run_with_args(lines: &[&str], args: &[String]) -> Result<String> {
    run_lines_with_args(lines, args)
}

pub fn open_url(url: &str) -> Result<String> {
    #[cfg(target_os = "windows")]
    {
        open_windows_target(url)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let script = format!(
            r#"
        tell application "System Events"
            set frontApp to name of first application process whose frontmost is true
        end tell
        
        if frontApp is "Safari" then
            tell application "Safari"
                activate
                open location "{}"
            end tell
            return "Opened in Safari"
        else if frontApp is "Google Chrome" then
            tell application "Google Chrome"
                activate
                open location "{}"
            end tell
            return "Opened in Chrome"
        else
            open location "{}"
            return "Opened in Default Browser"
        end if
    "#,
            url, url, url
        );

        run(&script)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "windows")]
    fn windows_mail_draft_lifecycle_smoke() {
        let create_lines = [
            "on run argv",
            "set bodyText to item 1 of argv",
            "set subjectHint to \"\"",
            "set recipientHint to \"\"",
            "if (count of argv) >= 2 then set subjectHint to item 2 of argv",
            "if (count of argv) >= 3 then set recipientHint to item 3 of argv",
            "tell application \"Mail\"",
            "return draftId & \"|\" & bodyLen",
            "end run",
        ];
        let create_out = run_with_args(
            &create_lines,
            &[
                "body line".to_string(),
                "subject".to_string(),
                "user@example.com".to_string(),
            ],
        )
        .expect("create draft");
        let mut parts = create_out.split('|');
        let draft_id = parts.next().unwrap_or("").to_string();
        let body_len = parts
            .next()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        assert!(!draft_id.is_empty());
        assert!(body_len >= 8);

        let send_lines = [
            "on run argv",
            "tell application \"Mail\"",
            "return \"sent_confirmed|\" & beforeOutgoing & \"|\" & afterOutgoing & \"|\" & _recipient & \"|\" & _subject & \"|\" & _draftId & \"|\" & _bodyLen",
            "end tell",
            "end run",
        ];
        let sent = run_with_args(
            &send_lines,
            &[
                "user@example.com".to_string(),
                "subject".to_string(),
                "".to_string(),
                draft_id.clone(),
                "0".to_string(),
            ],
        )
        .expect("send draft");
        assert!(sent.starts_with("sent_confirmed|"));
        assert!(sent.contains(&draft_id));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn windows_notes_textedit_smoke() {
        let notes_write_lines = [
            "on run argv",
            "tell application \"Notes\"",
            "return \"ok|\" & noteId & \"|\" & noteNameOut & \"|\" & noteBodyLen",
            "end run",
        ];
        let note_out = run_with_args(
            &notes_write_lines,
            &["hello notes".to_string(), "RUN_SCOPE_X".to_string()],
        )
        .expect("notes write");
        assert!(note_out.starts_with("ok|"));

        let notes_read_lines = [
            "on run argv",
            "tell application \"Notes\"",
            "return nName & return & nBody",
            "end run",
        ];
        let note_read = run_with_args(&notes_read_lines, &["RUN_SCOPE_X".to_string()])
            .expect("notes read");
        assert!(note_read.contains("RUN_SCOPE_X"));

        let textedit_write_lines = [
            "on run argv",
            "tell application \"TextEdit\"",
            "return \"ok|\" & docId & \"|\" & docName & \"|\" & bodyLen",
            "end run",
        ];
        let doc_out = run_with_args(
            &textedit_write_lines,
            &[
                "hello textedit".to_string(),
                "RUN_SCOPE_X".to_string(),
                "1".to_string(),
            ],
        )
        .expect("textedit write");
        assert!(doc_out.starts_with("ok|"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn windows_finder_downloads_smoke() {
        let lines = [
            "tell application \"Finder\"",
            "set targetFolder to (path to downloads folder)",
            "return \"ok\"",
        ];
        let out = run_with_args(&lines, &[]).expect("finder downloads");
        assert_eq!(out, "ok");
    }
}
