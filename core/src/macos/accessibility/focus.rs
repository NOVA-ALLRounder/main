use super::*;

pub(super) fn frontmost_app_name_via_osascript() -> Option<String> {
    frontmost_app_info_via_osascript().and_then(|(name, _)| {
        if name.trim().is_empty() {
            None
        } else {
            Some(name)
        }
    })
}

pub(super) fn frontmost_app_info_via_osascript() -> Option<(String, Option<i32>)> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to set p to first application process whose frontmost is true")
        .arg("-e")
        .arg("tell application \"System Events\" to return (name of p as text) & tab & (unix id of p as text)")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }
    let mut parts = raw.split('\t');
    let name = parts.next().unwrap_or_default().trim().to_string();
    let pid = parts.next().and_then(|v| v.trim().parse::<i32>().ok());
    Some((name, pid))
}

pub(super) fn bring_process_frontmost(pid: i32) {
    let script = format!(
        "tell application \"System Events\" to set frontmost of first application process whose unix id is {} to true",
        pid
    );
    let _ = Command::new("osascript").arg("-e").arg(script).output();
}

pub(super) fn activate_application_by_name(name: &str) {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return;
    }
    let escaped = trimmed.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!("tell application \"{}\" to activate", escaped);
    let _ = Command::new("osascript").arg("-e").arg(script).output();
}

pub(super) unsafe fn resolve_focused_application(
    system_wide: AXUIElementRef,
) -> Option<AXUIElementRef> {
    for _ in 0..4 {
        if let Some(r) = get_attribute(system_wide, "AXFocusedApplication") {
            return Some(r as AXUIElementRef);
        }
        thread::sleep(Duration::from_millis(60));
    }

    if let Some((_name, Some(pid))) = frontmost_app_info_via_osascript() {
        bring_process_frontmost(pid);
        thread::sleep(Duration::from_millis(90));
        for _ in 0..3 {
            if let Some(r) = get_attribute(system_wide, "AXFocusedApplication") {
                return Some(r as AXUIElementRef);
            }
            thread::sleep(Duration::from_millis(50));
        }
        let fallback_app = AXUIElementCreateApplication(pid);
        if !fallback_app.is_null() {
            return Some(fallback_app);
        }
    }
    None
}

pub(super) unsafe fn resolve_focused_window(
    focused_app_ref: AXUIElementRef,
) -> Option<AXUIElementRef> {
    if let Some(r) = get_attribute(focused_app_ref, "AXFocusedWindow") {
        return Some(r as AXUIElementRef);
    }
    if let Some(r) = get_attribute(focused_app_ref, "AXMainWindow") {
        return Some(r as AXUIElementRef);
    }
    if let Some(windows_ref) = get_attribute(focused_app_ref, "AXWindows") {
        let windows_array = CFArray::<CFTypeRef>::wrap_under_get_rule(
            windows_ref as core_foundation::array::CFArrayRef,
        );
        let first = windows_array.get(0).map(|ptr_ref| {
            let window = *ptr_ref as AXUIElementRef;
            core_foundation::base::CFRetain(window as CFTypeRef);
            window
        });
        core_foundation::base::CFRelease(windows_ref);
        if let Some(window) = first {
            return Some(window);
        }
    }
    None
}
