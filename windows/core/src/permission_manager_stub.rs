// Windows Permission Manager — real implementation replacing always-true stubs.
// Uses Windows UI Automation and Win32 API to verify actual system capabilities.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    Accessibility,
    ScreenRecording,
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Capability::Accessibility => write!(f, "Accessibility"),
            Capability::ScreenRecording => write!(f, "Screen Recording"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PermissionStatus {
    pub capability: Capability,
    pub granted: bool,
}

pub struct PermissionManager;

impl PermissionManager {
    /// Check if UI Automation is accessible by attempting to instantiate
    /// the UIA COM object and query the desktop root element.
    pub fn check_accessibility() -> bool {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::System::Com::*;
            use windows::Win32::UI::Accessibility::*;

            unsafe {
                // Ensure COM is initialized
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

                let automation: Result<IUIAutomation, _> = CoCreateInstance(
                    &CUIAutomation,
                    None,
                    CLSCTX_INPROC_SERVER,
                );

                match automation {
                    Ok(auto) => {
                        // Try to get the root (desktop) element
                        match auto.GetRootElement() {
                            Ok(_root) => true,
                            Err(e) => {
                                eprintln!("[PermissionManager] UIA root element failed: {}", e);
                                false
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[PermissionManager] UIA COM creation failed: {}", e);
                        false
                    }
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            true
        }
    }

    /// Request accessibility — on Windows, open the relevant Settings page.
    pub fn request_accessibility() -> bool {
        if Self::check_accessibility() {
            return true;
        }
        Self::open_privacy_settings(Capability::Accessibility);
        // Re-check after a short delay
        std::thread::sleep(std::time::Duration::from_secs(1));
        Self::check_accessibility()
    }

    /// Check screen recording capability by testing BitBlt screen capture.
    pub fn check_screen_recording() -> bool {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::Graphics::Gdi::*;
            use windows::Win32::UI::WindowsAndMessaging::*;

            unsafe {
                let hwnd_desktop = GetDesktopWindow();
                let hdc = GetDC(hwnd_desktop);
                if hdc.is_invalid() {
                    return false;
                }

                let hdc_mem = CreateCompatibleDC(hdc);
                if hdc_mem.is_invalid() {
                    ReleaseDC(hwnd_desktop, hdc);
                    return false;
                }

                // Create a 1x1 bitmap to test capture
                let hbmp = CreateCompatibleBitmap(hdc, 1, 1);
                if hbmp.is_invalid() {
                    let _ = DeleteDC(hdc_mem);
                    ReleaseDC(hwnd_desktop, hdc);
                    return false;
                }

                let old = SelectObject(hdc_mem, hbmp);
                let result = BitBlt(hdc_mem, 0, 0, 1, 1, hdc, 0, 0, SRCCOPY);

                // Cleanup
                SelectObject(hdc_mem, old);
                let _ = DeleteObject(hbmp);
                let _ = DeleteDC(hdc_mem);
                ReleaseDC(hwnd_desktop, hdc);

                result.is_ok()
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            true
        }
    }

    /// Request screen recording — on Windows this is generally always available.
    pub fn request_screen_recording() -> bool {
        if Self::check_screen_recording() {
            return true;
        }
        Self::open_privacy_settings(Capability::ScreenRecording);
        std::thread::sleep(std::time::Duration::from_secs(1));
        Self::check_screen_recording()
    }

    pub fn status_all() -> Vec<PermissionStatus> {
        vec![
            PermissionStatus {
                capability: Capability::Accessibility,
                granted: Self::check_accessibility(),
            },
            PermissionStatus {
                capability: Capability::ScreenRecording,
                granted: Self::check_screen_recording(),
            },
        ]
    }

    pub fn all_granted() -> bool {
        Self::check_accessibility() && Self::check_screen_recording()
    }

    pub fn ensure_all(_interactive: bool) -> Vec<PermissionStatus> {
        let status = Self::status_all();
        if _interactive {
            for s in &status {
                if !s.granted {
                    eprintln!(
                        "⚠️ [Permission] {} is not granted. Opening settings...",
                        s.capability
                    );
                    Self::open_privacy_settings(s.capability);
                }
            }
        }
        status
    }

    pub fn print_status() {
        println!("Permission Status:");
        for status in Self::status_all() {
            println!(
                "  {}: {}",
                status.capability,
                if status.granted { "✅ Granted" } else { "❌ Denied" }
            );
        }
    }

    pub fn open_privacy_settings(capability: Capability) {
        let uri = match capability {
            Capability::Accessibility => "ms-settings:easeofaccess-display",
            Capability::ScreenRecording => "ms-settings:privacy-broadfilesystemaccess",
        };
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", uri])
            .spawn();
    }
}
