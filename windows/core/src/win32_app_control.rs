// Windows Real App Control ??Outlook COM, Notepad Win32, Media Keys
// Replaces in-memory simulations with actual OS interactions.

#[cfg(target_os = "windows")]
pub mod outlook {
    //! New Outlook automation helpers for composing/sending mail on Windows.

    use anyhow::{Context, Result};
    use std::process::Command;
    use std::time::Duration;

    fn outlook_mode() -> String {
        std::env::var("STEER_WINDOWS_OUTLOOK_MODE")
            .unwrap_or_else(|_| "new".to_string())
            .trim()
            .to_ascii_lowercase()
    }

    fn mode_forces_new(mode: &str) -> bool {
        matches!(mode, "new" | "new_outlook" | "monarch")
    }

    fn ps_quote_single(s: &str) -> String {
        s.replace('\'', "''")
    }

    fn launch_new_outlook_compose(recipient: &str, subject: &str, body: &str) -> Result<()> {
        let uri = format!(
            "ms-outlook://compose?to={}&subject={}&body={}",
            urlencoding::encode(recipient),
            urlencoding::encode(subject),
            urlencoding::encode(body)
        );
        let ps = format!("Start-Process '{}'", ps_quote_single(&uri));
        let status = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps])
            .status()
            .context("Failed to launch New Outlook compose")?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("New Outlook compose launch failed"))
        }
    }

    fn click_send_in_foreground_window() -> Result<()> {
        std::thread::sleep(Duration::from_millis(1200));

        for label in ["Send"] {
            if crate::win32_app_control::system_events::click_button_by_name(label).is_ok() {
                return Ok(());
            }
        }

        let ps = "$ws = New-Object -ComObject WScript.Shell; Start-Sleep -Milliseconds 200; $ws.SendKeys('%s')";
        let status = Command::new("powershell")
            .args(["-NoProfile", "-Command", ps])
            .status()
            .context("Failed to send Alt+S hotkey for Outlook send")?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Could not trigger send action in New Outlook"))
        }
    }

    fn send_email_via_new_outlook(recipient: &str, subject: &str, body: &str) -> Result<()> {
        launch_new_outlook_compose(recipient, subject, body)?;
        click_send_in_foreground_window()?;
        Ok(())
    }

    fn create_draft_via_new_outlook(recipient: &str, subject: &str, body: &str) -> Result<()> {
        launch_new_outlook_compose(recipient, subject, body)
    }
    /// Create and send an email via New Outlook only.
    pub fn send_email(recipient: &str, subject: &str, body: &str) -> Result<()> {
        let mode = outlook_mode();
        if !mode_forces_new(&mode) && mode != "auto" {
            log::warn!(
                "Unknown STEER_WINDOWS_OUTLOOK_MODE='{}', forcing New Outlook path",
                mode
            );
        }
        send_email_via_new_outlook(recipient, subject, body)
    }

    /// Create a draft email in New Outlook only.
    pub fn create_draft(recipient: &str, subject: &str, body: &str) -> Result<()> {
        let mode = outlook_mode();
        if !mode_forces_new(&mode) && mode != "auto" {
            log::warn!(
                "Unknown STEER_WINDOWS_OUTLOOK_MODE='{}', forcing New Outlook path",
                mode
            );
        }
        create_draft_via_new_outlook(recipient, subject, body)
    }
}

#[cfg(target_os = "windows")]
pub mod notepad {
    //! Notepad Win32 control ??open, find, set text.

    use anyhow::Result;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::Foundation::*;
    use windows::core::PCWSTR;

    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    /// Find an existing Notepad window, or open a new one.
    pub fn open_or_find() -> Result<HWND> {
        let class = to_wide("Notepad");
        unsafe {
            if let Ok(hwnd) = FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null()) {
                if !hwnd.is_invalid() {
                    let _ = SetForegroundWindow(hwnd);
                    return Ok(hwnd);
                }
            }
        }

        // Open notepad
        std::process::Command::new("notepad.exe").spawn()?;
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Find again
        unsafe {
            let hwnd = FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null())
                .map_err(|e| anyhow::anyhow!("Could not find Notepad window: {}", e))?;
            if hwnd.is_invalid() {
                return Err(anyhow::anyhow!("Notepad window handle is invalid after opening"));
            }
            let _ = SetForegroundWindow(hwnd);
            Ok(hwnd)
        }
    }

    /// Get the edit control inside Notepad.
    fn get_edit_control(hwnd: HWND) -> Result<HWND> {
        unsafe {
            // Try classic "Edit" control (Win10 Notepad)
            let edit_class = to_wide("Edit");
            if let Ok(edit) = FindWindowExW(hwnd, HWND::default(), PCWSTR(edit_class.as_ptr()), PCWSTR::null()) {
                if !edit.is_invalid() {
                    return Ok(edit);
                }
            }
            // Try RichEditD2DPT (Windows 11 new Notepad)
            let rich_class = to_wide("RichEditD2DPT");
            if let Ok(rich) = FindWindowExW(hwnd, HWND::default(), PCWSTR(rich_class.as_ptr()), PCWSTR::null()) {
                if !rich.is_invalid() {
                    return Ok(rich);
                }
            }
            Err(anyhow::anyhow!("Could not find edit control in Notepad"))
        }
    }

    /// Set text in Notepad window.
    pub fn set_text(text: &str) -> Result<()> {
        let hwnd = open_or_find()?;
        let edit = get_edit_control(hwnd)?;
        let wide_text = to_wide(text);

        unsafe {
            SendMessageW(edit, WM_SETTEXT, WPARAM(0), LPARAM(wide_text.as_ptr() as isize));
        }
        Ok(())
    }

    /// Append text to existing Notepad content.
    pub fn append_text(text: &str) -> Result<()> {
        let hwnd = open_or_find()?;
        let edit = get_edit_control(hwnd)?;

        unsafe {
            let len = SendMessageW(edit, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0));

            // EM_SETSEL: move cursor to end
            let em_setsel: u32 = 0x00B1;
            SendMessageW(edit, em_setsel, WPARAM(len.0 as usize), LPARAM(len.0));

            // EM_REPLACESEL: insert text at cursor
            let em_replacesel: u32 = 0x00C2;
            let wide = to_wide(&format!("\r\n{}", text));
            SendMessageW(edit, em_replacesel, WPARAM(1), LPARAM(wide.as_ptr() as isize));
        }
        Ok(())
    }

    /// Get current text from Notepad.
    pub fn get_text() -> Result<String> {
        let hwnd = open_or_find()?;
        let edit = get_edit_control(hwnd)?;

        unsafe {
            let len = SendMessageW(edit, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0));
            if len.0 <= 0 {
                return Ok(String::new());
            }
            let mut buf = vec![0u16; (len.0 as usize) + 1];
            SendMessageW(edit, WM_GETTEXT, WPARAM(buf.len()), LPARAM(buf.as_mut_ptr() as isize));
            Ok(String::from_utf16_lossy(&buf).trim_end_matches('\0').to_string())
        }
    }
}

#[cfg(target_os = "windows")]
pub mod media {
    //! Media key control ??play/pause, next, previous, volume.

    use windows::Win32::UI::Input::KeyboardAndMouse::*;

    /// Send a single media key press.
    unsafe fn send_media_key(vk: VIRTUAL_KEY) {
        let mut inputs = [INPUT::default(); 2];

        inputs[0].r#type = INPUT_KEYBOARD;
        inputs[0].Anonymous.ki.wVk = vk;

        inputs[1].r#type = INPUT_KEYBOARD;
        inputs[1].Anonymous.ki.wVk = vk;
        inputs[1].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;

        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }

    pub fn play_pause() {
        unsafe { send_media_key(VK_MEDIA_PLAY_PAUSE); }
    }

    pub fn next_track() {
        unsafe { send_media_key(VK_MEDIA_NEXT_TRACK); }
    }

    pub fn prev_track() {
        unsafe { send_media_key(VK_MEDIA_PREV_TRACK); }
    }

    pub fn stop() {
        unsafe { send_media_key(VK_MEDIA_STOP); }
    }

    pub fn volume_up() {
        unsafe { send_media_key(VK_VOLUME_UP); }
    }

    pub fn volume_down() {
        unsafe { send_media_key(VK_VOLUME_DOWN); }
    }

    pub fn volume_mute() {
        unsafe { send_media_key(VK_VOLUME_MUTE); }
    }
}

#[cfg(target_os = "windows")]
pub mod system_events {
    //! System Events equivalent ??process/window management via Win32 API.
    //! Replaces macOS `tell application "System Events"` patterns.

    use anyhow::{Context, Result};
    use windows::Win32::Foundation::*;
    use windows::Win32::System::Threading::*;
    use windows::Win32::System::Diagnostics::ToolHelp::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    /// Information about a running process.
    #[derive(Debug, Clone)]
    pub struct ProcessInfo {
        pub pid: u32,
        pub name: String,
    }

    /// Information about a visible window.
    #[derive(Debug, Clone)]
    pub struct WindowInfo {
        pub hwnd: isize,
        pub title: String,
        pub pid: u32,
        pub process_name: String,
        pub x: i32,
        pub y: i32,
        pub width: i32,
        pub height: i32,
        pub visible: bool,
        pub minimized: bool,
    }

    /// List all running processes.
    pub fn list_processes() -> Result<Vec<ProcessInfo>> {
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
                .context("CreateToolhelp32Snapshot failed")?;
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };

            let mut result = Vec::new();
            if Process32FirstW(snap, &mut entry).is_ok() {
                loop {
                    let name_len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                    let name = String::from_utf16_lossy(&entry.szExeFile[..name_len]);
                    result.push(ProcessInfo {
                        pid: entry.th32ProcessID,
                        name,
                    });
                    if Process32NextW(snap, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snap);
            Ok(result)
        }
    }

    /// Get process name by PID.
    fn process_name_by_pid(pid: u32) -> String {
        if let Ok(procs) = list_processes() {
            for p in procs {
                if p.pid == pid {
                    return p.name;
                }
            }
        }
        String::new()
    }

    /// List all visible top-level windows.
    pub fn list_windows() -> Result<Vec<WindowInfo>> {
        unsafe {
            let mut windows: Vec<WindowInfo> = Vec::new();
            let windows_ptr: *mut Vec<WindowInfo> = &mut windows;

            let _ = EnumWindows(
                Some(enum_windows_callback),
                LPARAM(windows_ptr as isize),
            );
            Ok(windows)
        }
    }

    unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        if !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1); // continue
        }

        let mut title_buf = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, &mut title_buf);
        if title_len == 0 {
            return BOOL(1); // skip windows with no title
        }
        let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        let process_name = process_name_by_pid(pid);

        let mut rect = RECT::default();
        let _ = GetWindowRect(hwnd, &mut rect);

        let minimized = IsIconic(hwnd).as_bool();

        let windows = &mut *(lparam.0 as *mut Vec<WindowInfo>);
        windows.push(WindowInfo {
            hwnd: hwnd.0 as isize,
            title,
            pid,
            process_name,
            x: rect.left,
            y: rect.top,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
            visible: true,
            minimized,
        });

        BOOL(1) // continue enumeration
    }

    /// Get position and size of a window.
    pub fn get_window_rect(hwnd: isize) -> Result<(i32, i32, i32, i32)> {
        unsafe {
            let h = HWND(hwnd as *mut _);
            let mut rect = RECT::default();
            GetWindowRect(h, &mut rect)
                .context("GetWindowRect failed")?;
            Ok((rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top))
        }
    }

    /// Set position and size of a window.
    pub fn set_window_rect(hwnd: isize, x: i32, y: i32, width: i32, height: i32) -> Result<()> {
        unsafe {
            let h = HWND(hwnd as *mut _);
            // Restore if minimized first
            if IsIconic(h).as_bool() {
                let _ = ShowWindow(h, SW_RESTORE);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            SetWindowPos(
                h,
                None,
                x, y, width, height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            ).context("SetWindowPos failed")?;
            Ok(())
        }
    }

    /// Minimize a window.
    pub fn minimize_window(hwnd: isize) -> Result<()> {
        unsafe {
            let _ = ShowWindow(HWND(hwnd as *mut _), SW_MINIMIZE);
            Ok(())
        }
    }

    /// Maximize a window.
    pub fn maximize_window(hwnd: isize) -> Result<()> {
        unsafe {
            let _ = ShowWindow(HWND(hwnd as *mut _), SW_MAXIMIZE);
            Ok(())
        }
    }

    /// Restore a window from minimized/maximized state.
    pub fn restore_window(hwnd: isize) -> Result<()> {
        unsafe {
            let _ = ShowWindow(HWND(hwnd as *mut _), SW_RESTORE);
            Ok(())
        }
    }

    /// Close a window via WM_CLOSE.
    pub fn close_window(hwnd: isize) -> Result<()> {
        unsafe {
            PostMessageW(HWND(hwnd as *mut _), WM_CLOSE, WPARAM(0), LPARAM(0))
                .context("PostMessage WM_CLOSE failed")?;
            Ok(())
        }
    }

    /// Bring a window to the foreground.
    pub fn set_foreground(hwnd: isize) -> Result<()> {
        unsafe {
            let h = HWND(hwnd as *mut _);
            if IsIconic(h).as_bool() {
                let _ = ShowWindow(h, SW_RESTORE);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            let _ = SetForegroundWindow(h);
            Ok(())
        }
    }

    /// Set an application as frontmost by process name.
    pub fn set_frontmost_by_name(process_name: &str) -> Result<()> {
        let windows = list_windows()?;
        let lower = process_name.to_ascii_lowercase();
        for w in &windows {
            let pn = w.process_name.to_ascii_lowercase();
            if pn.contains(&lower) || pn.trim_end_matches(".exe") == lower {
                return set_foreground(w.hwnd);
            }
        }
        Err(anyhow::anyhow!("No window found for process: {}", process_name))
    }

    /// Force-quit a process by PID.
    pub fn kill_process(pid: u32) -> Result<()> {
        unsafe {
            let process = OpenProcess(PROCESS_TERMINATE, false, pid)
                .context("OpenProcess failed")?;
            TerminateProcess(process, 1)
                .context("TerminateProcess failed")?;
            let _ = CloseHandle(process);
            Ok(())
        }
    }

    /// Force-quit a process by name.
    pub fn kill_by_name(name: &str) -> Result<()> {
        let procs = list_processes()?;
        let lower = name.to_ascii_lowercase();
        let mut killed = false;
        for p in &procs {
            let pn = p.name.to_ascii_lowercase();
            if pn == lower || pn == format!("{}.exe", lower) {
                let _ = kill_process(p.pid);
                killed = true;
            }
        }
        if killed {
            Ok(())
        } else {
            Err(anyhow::anyhow!("No process found: {}", name))
        }
    }

    /// Hide all windows except the foreground window (macOS Cmd+Opt+H equivalent).
    pub fn hide_other_apps() -> Result<()> {
        unsafe {
            let fg = GetForegroundWindow();
            let mut fg_pid: u32 = 0;
            GetWindowThreadProcessId(fg, Some(&mut fg_pid));

            let windows = list_windows()?;
            for w in &windows {
                if w.pid != fg_pid && !w.minimized {
                    let _ = ShowWindow(HWND(w.hwnd as *mut _), SW_MINIMIZE);
                }
            }
            Ok(())
        }
    }

    /// Click a named button in the current foreground window using UIA.
    pub fn click_button_by_name(button_name: &str) -> Result<()> {
        use windows::Win32::UI::Accessibility::*;
        use windows::Win32::System::Com::*;
        use windows::core::Interface;

        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let automation: IUIAutomation = CoCreateInstance(
                &CUIAutomation,
                None,
                CLSCTX_INPROC_SERVER,
            ).context("Failed to create UIA instance")?;

            let hwnd = GetForegroundWindow();
            let root = automation.ElementFromHandle(hwnd)
                .context("ElementFromHandle failed")?;

            // Create condition: ControlType == Button AND Name == button_name
            let name_cond = automation.CreatePropertyCondition(
                UIA_NamePropertyId,
                &windows::core::VARIANT::from(button_name),
            ).context("CreatePropertyCondition Name failed")?;

            let type_cond = automation.CreatePropertyCondition(
                UIA_ControlTypePropertyId,
                &windows::core::VARIANT::from(UIA_ButtonControlTypeId.0),
            ).context("CreatePropertyCondition ControlType failed")?;

            let combined = automation.CreateAndCondition(&name_cond, &type_cond)
                .context("CreateAndCondition failed")?;

            let element = root.FindFirst(TreeScope_Subtree, &combined)
                .context(format!("Button '{}' not found", button_name))?;

            // Try InvokePattern
            if let Ok(pattern) = element.GetCurrentPattern(UIA_InvokePatternId) {
                if let Ok(invoke) = pattern.cast::<IUIAutomationInvokePattern>() {
                    invoke.Invoke().context("Invoke failed")?;
                    return Ok(());
                }
            }

            Err(anyhow::anyhow!("Button '{}' found but not invocable", button_name))
        }
    }

    /// Click a menu item by path, e.g. ["File", "Save As..."]
    pub fn click_menu_item(menu_path: &[&str]) -> Result<()> {
        use windows::Win32::UI::Accessibility::*;
        use windows::Win32::System::Com::*;
        use windows::core::Interface;

        if menu_path.is_empty() {
            return Err(anyhow::anyhow!("Empty menu path"));
        }

        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let automation: IUIAutomation = CoCreateInstance(
                &CUIAutomation,
                None,
                CLSCTX_INPROC_SERVER,
            ).context("Failed to create UIA instance")?;

            let hwnd = GetForegroundWindow();
            let root = automation.ElementFromHandle(hwnd)
                .context("ElementFromHandle failed")?;

            // Find the menu bar
            let menubar_cond = automation.CreatePropertyCondition(
                UIA_ControlTypePropertyId,
                &windows::core::VARIANT::from(UIA_MenuBarControlTypeId.0),
            )?;

            let menubar = root.FindFirst(TreeScope_Children, &menubar_cond)
                .context("MenuBar not found")?;

            let mut current = menubar;

            for (i, menu_name) in menu_path.iter().enumerate() {
                let name_cond = automation.CreatePropertyCondition(
                    UIA_NamePropertyId,
                    &windows::core::VARIANT::from(*menu_name),
                )?;

                let menu_item_cond = automation.CreatePropertyCondition(
                    UIA_ControlTypePropertyId,
                    &windows::core::VARIANT::from(UIA_MenuItemControlTypeId.0),
                )?;

                let combined = automation.CreateAndCondition(&name_cond, &menu_item_cond)?;

                let scope = if i == 0 { TreeScope_Children } else { TreeScope_Subtree };
                let element = current.FindFirst(scope, &combined)
                    .context(format!("Menu item '{}' not found", menu_name))?;

                // Expand if not last item (opens submenu)
                if i < menu_path.len() - 1 {
                    if let Ok(pattern) = element.GetCurrentPattern(UIA_ExpandCollapsePatternId) {
                        if let Ok(expand) = pattern.cast::<IUIAutomationExpandCollapsePattern>() {
                            let _ = expand.Expand();
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                    } else {
                        // Try invoke to open submenu
                        if let Ok(pattern) = element.GetCurrentPattern(UIA_InvokePatternId) {
                            if let Ok(invoke) = pattern.cast::<IUIAutomationInvokePattern>() {
                                let _ = invoke.Invoke();
                                std::thread::sleep(std::time::Duration::from_millis(100));
                            }
                        }
                    }
                } else {
                    // Last item ??invoke
                    if let Ok(pattern) = element.GetCurrentPattern(UIA_InvokePatternId) {
                        if let Ok(invoke) = pattern.cast::<IUIAutomationInvokePattern>() {
                            invoke.Invoke().context("Menu invoke failed")?;
                            return Ok(());
                        }
                    }
                }

                current = element;
            }

            Ok(())
        }
    }

    /// Set value of a UI element by name using UIA ValuePattern.
    pub fn set_element_value(element_name: &str, value: &str) -> Result<()> {
        use windows::Win32::UI::Accessibility::*;
        use windows::Win32::System::Com::*;
        use windows::core::Interface;

        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let automation: IUIAutomation = CoCreateInstance(
                &CUIAutomation,
                None,
                CLSCTX_INPROC_SERVER,
            ).context("Failed to create UIA instance")?;

            let hwnd = GetForegroundWindow();
            let root = automation.ElementFromHandle(hwnd)
                .context("ElementFromHandle failed")?;

            let name_cond = automation.CreatePropertyCondition(
                UIA_NamePropertyId,
                &windows::core::VARIANT::from(element_name),
            )?;

            let element = root.FindFirst(TreeScope_Subtree, &name_cond)
                .context(format!("Element '{}' not found", element_name))?;

            // Try ValuePattern
            if let Ok(pattern) = element.GetCurrentPattern(UIA_ValuePatternId) {
                if let Ok(value_pattern) = pattern.cast::<IUIAutomationValuePattern>() {
                    let bstr = windows::core::BSTR::from(value);
                    value_pattern.SetValue(&bstr)
                        .context("SetValue failed")?;
                    return Ok(());
                }
            }

            Err(anyhow::anyhow!("Element '{}' does not support ValuePattern", element_name))
        }
    }

    /// Toggle a UI element (checkbox, toggle button) by name.
    pub fn toggle_element(element_name: &str) -> Result<()> {
        use windows::Win32::UI::Accessibility::*;
        use windows::Win32::System::Com::*;
        use windows::core::Interface;

        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let automation: IUIAutomation = CoCreateInstance(
                &CUIAutomation,
                None,
                CLSCTX_INPROC_SERVER,
            ).context("Failed to create UIA instance")?;

            let hwnd = GetForegroundWindow();
            let root = automation.ElementFromHandle(hwnd)
                .context("ElementFromHandle failed")?;

            let name_cond = automation.CreatePropertyCondition(
                UIA_NamePropertyId,
                &windows::core::VARIANT::from(element_name),
            )?;

            let element = root.FindFirst(TreeScope_Subtree, &name_cond)
                .context(format!("Element '{}' not found", element_name))?;

            // Try TogglePattern
            if let Ok(pattern) = element.GetCurrentPattern(UIA_TogglePatternId) {
                if let Ok(toggle) = pattern.cast::<IUIAutomationTogglePattern>() {
                    toggle.Toggle().context("Toggle failed")?;
                    return Ok(());
                }
            }

            // Fallback: InvokePattern
            if let Ok(pattern) = element.GetCurrentPattern(UIA_InvokePatternId) {
                if let Ok(invoke) = pattern.cast::<IUIAutomationInvokePattern>() {
                    invoke.Invoke().context("Invoke failed")?;
                    return Ok(());
                }
            }

            Err(anyhow::anyhow!("Element '{}' does not support Toggle/Invoke", element_name))
        }
    }

    /// Get the window title of the foreground window.
    pub fn get_foreground_title() -> String {
        unsafe {
            let hwnd = GetForegroundWindow();
            let mut buf = [0u16; 1024];
            let len = GetWindowTextW(hwnd, &mut buf);
            String::from_utf16_lossy(&buf[..len as usize])
        }
    }

    /// Get the foreground window handle.
    pub fn get_foreground_hwnd() -> isize {
        unsafe { GetForegroundWindow().0 as isize }
    }
}

#[cfg(target_os = "windows")]
pub mod win32_msg {
    //! Direct Win32 message-based text retrieval for legacy controls.
    //! These functions bypass UIA and read directly from Win32 controls
    //! using window messages, providing faster and more reliable text
    //! retrieval for standard Win32 controls.

    use anyhow::Result;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::Foundation::*;

    /// Get text content from any Win32 control via WM_GETTEXT.
    /// Works on Edit, Static, Button, and most standard controls.
    pub fn get_text_from_control(hwnd: isize) -> Result<String> {
        unsafe {
            let h = HWND(hwnd as *mut _);
            let len = SendMessageW(h, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0));
            if len.0 <= 0 {
                return Ok(String::new());
            }
            let mut buf = vec![0u16; (len.0 as usize) + 1];
            SendMessageW(h, WM_GETTEXT, WPARAM(buf.len()), LPARAM(buf.as_mut_ptr() as isize));
            Ok(String::from_utf16_lossy(&buf).trim_end_matches('\0').to_string())
        }
    }

    /// Get all items from a ListBox control via LB_GETCOUNT + LB_GETTEXT.
    pub fn get_listbox_items(hwnd: isize) -> Result<Vec<String>> {
        const LB_GETCOUNT: u32 = 0x018B;
        const LB_GETTEXT: u32 = 0x0189;
        const LB_GETTEXTLEN: u32 = 0x018A;

        unsafe {
            let h = HWND(hwnd as *mut _);
            let count = SendMessageW(h, LB_GETCOUNT, WPARAM(0), LPARAM(0));
            if count.0 <= 0 {
                return Ok(Vec::new());
            }

            let mut items = Vec::new();
            for i in 0..(count.0 as usize) {
                let text_len = SendMessageW(h, LB_GETTEXTLEN, WPARAM(i), LPARAM(0));
                if text_len.0 <= 0 {
                    continue;
                }
                let mut buf = vec![0u16; (text_len.0 as usize) + 1];
                SendMessageW(h, LB_GETTEXT, WPARAM(i), LPARAM(buf.as_mut_ptr() as isize));
                let s = String::from_utf16_lossy(&buf).trim_end_matches('\0').to_string();
                if !s.is_empty() {
                    items.push(s);
                }
            }
            Ok(items)
        }
    }

    /// Get text from a ComboBox control via CB_GETLBTEXT.
    pub fn get_combobox_text(hwnd: isize) -> Result<String> {
        const CB_GETCURSEL: u32 = 0x0147;
        const CB_GETLBTEXTLEN: u32 = 0x0149;
        const CB_GETLBTEXT: u32 = 0x0148;

        unsafe {
            let h = HWND(hwnd as *mut _);

            // Get currently selected item index
            let cur_sel = SendMessageW(h, CB_GETCURSEL, WPARAM(0), LPARAM(0));
            if cur_sel.0 < 0 {
                // No selection ??try WM_GETTEXT for edit portion
                return get_text_from_control(hwnd);
            }

            let text_len = SendMessageW(h, CB_GETLBTEXTLEN, WPARAM(cur_sel.0 as usize), LPARAM(0));
            if text_len.0 <= 0 {
                return Ok(String::new());
            }

            let mut buf = vec![0u16; (text_len.0 as usize) + 1];
            SendMessageW(h, CB_GETLBTEXT, WPARAM(cur_sel.0 as usize), LPARAM(buf.as_mut_ptr() as isize));
            Ok(String::from_utf16_lossy(&buf).trim_end_matches('\0').to_string())
        }
    }

    /// Get all items from a ComboBox control's dropdown list.
    pub fn get_combobox_items(hwnd: isize) -> Result<Vec<String>> {
        const CB_GETCOUNT: u32 = 0x0146;
        const CB_GETLBTEXTLEN: u32 = 0x0149;
        const CB_GETLBTEXT: u32 = 0x0148;

        unsafe {
            let h = HWND(hwnd as *mut _);
            let count = SendMessageW(h, CB_GETCOUNT, WPARAM(0), LPARAM(0));
            if count.0 <= 0 {
                return Ok(Vec::new());
            }

            let mut items = Vec::new();
            for i in 0..(count.0 as usize) {
                let text_len = SendMessageW(h, CB_GETLBTEXTLEN, WPARAM(i), LPARAM(0));
                if text_len.0 <= 0 {
                    continue;
                }
                let mut buf = vec![0u16; (text_len.0 as usize) + 1];
                SendMessageW(h, CB_GETLBTEXT, WPARAM(i), LPARAM(buf.as_mut_ptr() as isize));
                let s = String::from_utf16_lossy(&buf).trim_end_matches('\0').to_string();
                if !s.is_empty() {
                    items.push(s);
                }
            }
            Ok(items)
        }
    }

    /// Enumerate all child windows of a given parent and collect their text.
    /// Useful for debugging or gathering all visible text from a dialog.
    pub fn get_all_child_text(parent_hwnd: isize) -> Result<Vec<String>> {
        unsafe {
            let mut texts: Vec<String> = Vec::new();
            let texts_ptr: *mut Vec<String> = &mut texts;

            let _ = EnumChildWindows(
                HWND(parent_hwnd as *mut _),
                Some(enum_child_text_callback),
                LPARAM(texts_ptr as isize),
            );

            Ok(texts)
        }
    }

    unsafe extern "system" fn enum_child_text_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let len = SendMessageW(hwnd, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0));
        if len.0 > 0 {
            let mut buf = vec![0u16; (len.0 as usize) + 1];
            SendMessageW(hwnd, WM_GETTEXT, WPARAM(buf.len()), LPARAM(buf.as_mut_ptr() as isize));
            let s = String::from_utf16_lossy(&buf).trim_end_matches('\0').to_string();
            if !s.trim().is_empty() {
                let texts = &mut *(lparam.0 as *mut Vec<String>);
                texts.push(s);
            }
        }
        BOOL(1) // continue enumeration
    }
}

