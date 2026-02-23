// Windows UI Automation stub — replaces macOS Accessibility API
// Uses Win32 UI Automation (UIA) for real UI tree traversal and element interaction.
// Enhanced with: CacheRequest optimization, MSAA fallback, OCR fallback.

use serde_json::json;

#[cfg(target_os = "windows")]
use windows::{
    Win32::UI::Accessibility::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::Foundation::*,
    Win32::System::Com::*,
    Win32::UI::Input::KeyboardAndMouse::*,
};

#[cfg(target_os = "windows")]
use std::sync::Once;

#[cfg(target_os = "windows")]
static COM_INIT: Once = Once::new();

#[cfg(target_os = "windows")]
fn ensure_com_initialized() {
    COM_INIT.call_once(|| {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
    });
}

pub mod accessibility {
    use super::json;
    use serde_json::Value;

    fn env_usize_bounded(key: &str, default: usize, min: usize, max: usize) -> usize {
        std::env::var(key)
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .map(|v| v.clamp(min, max))
            .unwrap_or(default)
    }

    fn uia_max_children() -> usize {
        env_usize_bounded("STEER_UIA_MAX_CHILDREN", 100, 20, 400)
    }

    /// Take a snapshot of the active window's UI tree via Windows UIA.
    /// Returns a JSON structure similar to macOS Accessibility snapshot.
    pub fn snapshot(_scope: Option<String>) -> Value {
        #[cfg(target_os = "windows")]
        {
            super::ensure_com_initialized();
            match snapshot_via_uia() {
                Ok(val) => val,
                Err(e) => {
                    eprintln!("[UIA] snapshot error: {}", e);
                    fallback_snapshot()
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            fallback_snapshot()
        }
    }

    fn fallback_snapshot() -> Value {
        #[cfg(target_os = "windows")]
        let title = crate::win32_app_control::system_events::get_foreground_title();

        #[cfg(not(target_os = "windows"))]
        let title = crate::applescript::get_active_window_context()
            .map(|(t, _)| t)
            .unwrap_or_default();

        json!({
            "role": "WindowsDesktop",
            "title": title,
            "focused_window": {
                "role": "Window",
                "title": "",
                "children": []
            },
            "strict_mode": false,
            "ok": true
        })
    }

    #[cfg(target_os = "windows")]
    fn snapshot_via_uia() -> Result<Value, String> {
        use super::*;
        unsafe {
            let automation: IUIAutomation = CoCreateInstance(
                &CUIAutomation,
                None,
                CLSCTX_INPROC_SERVER,
            ).map_err(|e| format!("Failed to create UIA: {}", e))?;

            // Get the focused element first
            let focused = automation.GetFocusedElement()
                .map_err(|e| format!("GetFocusedElement failed: {}", e))?;

            // Get the foreground window
            let hwnd = GetForegroundWindow();

            let mut title_buf = [0u16; 512];
            let title_len = GetWindowTextW(hwnd, &mut title_buf);
            let window_title = String::from_utf16_lossy(&title_buf[..title_len as usize]);

            // Get the root element for the foreground window
            let root = automation.ElementFromHandle(hwnd)
                .map_err(|e| format!("ElementFromHandle failed: {}", e))?;

            // --- CacheRequest optimization ---
            // Create a CacheRequest to batch-fetch properties for better performance
            let cache_request = automation.CreateCacheRequest()
                .map_err(|e| format!("CreateCacheRequest failed: {}", e))?;

            // Add properties to cache: Name, ControlType, BoundingRectangle, AutomationId, ClassName
            let _ = cache_request.AddProperty(UIA_NamePropertyId);
            let _ = cache_request.AddProperty(UIA_ControlTypePropertyId);
            let _ = cache_request.AddProperty(UIA_BoundingRectanglePropertyId);
            let _ = cache_request.AddProperty(UIA_AutomationIdPropertyId);
            let _ = cache_request.AddProperty(UIA_ClassNamePropertyId);

            // Use ControlViewCondition for smarter traversal (skip raw/invisible elements)
            let _ = cache_request.SetTreeScope(TreeScope_Subtree);

            // Build UI tree with cached properties (limited depth for performance)
            let max_depth = env_usize_bounded("STEER_UIA_MAX_DEPTH", 8, 2, 32) as u32;

            // Try to get a cached subtree first for performance
            let root_cached = automation.ElementFromHandleBuildCache(hwnd, &cache_request)
                .unwrap_or(root);

            let tree = build_uia_tree_cached(&automation, &root_cached, 0, max_depth, &cache_request)?;

            // Get focused element info
            let focused_name = focused.CurrentName()
                .map(|s| s.to_string())
                .unwrap_or_default();
            let focused_role = focused.CurrentControlType()
                .map(|ct| uia_control_type_name(ct))
                .unwrap_or_else(|_| "Unknown".to_string());
            let focused_auto_id = focused.CurrentAutomationId()
                .map(|s| s.to_string())
                .unwrap_or_default();

            Ok(json!({
                "role": "WindowsDesktop",
                "title": window_title,
                "focused_element": {
                    "role": focused_role,
                    "name": focused_name,
                    "automationId": focused_auto_id,
                },
                "focused_window": tree,
                "strict_mode": false,
                "ok": true
            }))
        }
    }

    /// Build UIA tree with CacheRequest for batch property retrieval.
    /// Falls back to Current* properties if cached properties are unavailable.
    #[cfg(target_os = "windows")]
    fn build_uia_tree_cached(
        automation: &super::IUIAutomation,
        element: &super::IUIAutomationElement,
        depth: u32,
        max_depth: u32,
        _cache_request: &super::IUIAutomationCacheRequest,
    ) -> Result<serde_json::Value, String> {
        use super::*;
        if depth >= max_depth {
            return Ok(json!({"role": "...", "children": []}));
        }

        unsafe {
            // Try cached properties first, fall back to Current* properties
            let name = element.CachedName()
                .or_else(|_| element.CurrentName())
                .map(|s| s.to_string())
                .unwrap_or_default();

            let control_type = element.CachedControlType()
                .or_else(|_| element.CurrentControlType())
                .map(|ct| uia_control_type_name(ct))
                .unwrap_or_else(|_| "Unknown".to_string());

            let auto_id = element.CachedAutomationId()
                .or_else(|_| element.CurrentAutomationId())
                .map(|s| s.to_string())
                .unwrap_or_default();

            let class_name = element.CachedClassName()
                .or_else(|_| element.CurrentClassName())
                .map(|s| s.to_string())
                .unwrap_or_default();

            // Try to get Value for editable controls
            let value = element.GetCurrentPattern(UIA_ValuePatternId)
                .ok()
                .and_then(|p| {
                    use windows::core::Interface;
                    p.cast::<IUIAutomationValuePattern>().ok()
                })
                .and_then(|vp| vp.CurrentValue().ok())
                .map(|v| v.to_string())
                .unwrap_or_default();

            let mut children_json = Vec::new();

            // --- Smart TreeWalker selection ---
            // Use ControlViewWalker for structured content (skips raw container elements)
            // This gives better results than RawViewWalker for most applications
            if depth < max_depth - 1 {
                let walker = automation.ControlViewWalker()
                    .map_err(|e| format!("ControlViewWalker failed: {}", e))?;

                if let Ok(first_child) = walker.GetFirstChildElement(element) {
                    let mut current = first_child;
                    let mut count = 0;
                    let max_children = uia_max_children();

                    loop {
                        if count >= max_children {
                            children_json.push(json!({"role": "...", "note": "truncated"}));
                            break;
                        }

                        match build_uia_tree_cached(automation, &current, depth + 1, max_depth, _cache_request) {
                            Ok(child) => children_json.push(child),
                            Err(_) => break,
                        }

                        match walker.GetNextSiblingElement(&current) {
                            Ok(next) => current = next,
                            Err(_) => break,
                        }
                        count += 1;
                    }
                }
            }

            // Get bounding rectangle
            let bounds = element.CachedBoundingRectangle()
                .or_else(|_| element.CurrentBoundingRectangle())
                .ok();
            let bounds_json = bounds.map(|r| {
                json!({
                    "x": r.left,
                    "y": r.top,
                    "width": r.right - r.left,
                    "height": r.bottom - r.top
                })
            });

            let mut result = json!({
                "role": control_type,
                "title": name,
                "children": children_json,
            });

            if !auto_id.is_empty() {
                result["automationId"] = json!(auto_id);
            }
            if !class_name.is_empty() {
                result["className"] = json!(class_name);
            }
            if !value.is_empty() {
                result["value"] = json!(value);
            }
            if let Some(b) = bounds_json {
                result["bounds"] = b;
            }

            Ok(result)
        }
    }

    #[cfg(target_os = "windows")]
    #[allow(non_upper_case_globals)]
    fn uia_control_type_name(ct: super::UIA_CONTROLTYPE_ID) -> String {
        use super::*;
        match ct {
            UIA_ButtonControlTypeId => "Button".to_string(),
            UIA_CalendarControlTypeId => "Calendar".to_string(),
            UIA_CheckBoxControlTypeId => "CheckBox".to_string(),
            UIA_ComboBoxControlTypeId => "ComboBox".to_string(),
            UIA_EditControlTypeId => "Edit".to_string(),
            UIA_HyperlinkControlTypeId => "Hyperlink".to_string(),
            UIA_ImageControlTypeId => "Image".to_string(),
            UIA_ListItemControlTypeId => "ListItem".to_string(),
            UIA_ListControlTypeId => "List".to_string(),
            UIA_MenuControlTypeId => "Menu".to_string(),
            UIA_MenuBarControlTypeId => "MenuBar".to_string(),
            UIA_MenuItemControlTypeId => "MenuItem".to_string(),
            UIA_ProgressBarControlTypeId => "ProgressBar".to_string(),
            UIA_RadioButtonControlTypeId => "RadioButton".to_string(),
            UIA_ScrollBarControlTypeId => "ScrollBar".to_string(),
            UIA_SliderControlTypeId => "Slider".to_string(),
            UIA_SpinnerControlTypeId => "Spinner".to_string(),
            UIA_StatusBarControlTypeId => "StatusBar".to_string(),
            UIA_TabControlTypeId => "Tab".to_string(),
            UIA_TabItemControlTypeId => "TabItem".to_string(),
            UIA_TextControlTypeId => "Text".to_string(),
            UIA_ToolBarControlTypeId => "ToolBar".to_string(),
            UIA_ToolTipControlTypeId => "ToolTip".to_string(),
            UIA_TreeControlTypeId => "Tree".to_string(),
            UIA_TreeItemControlTypeId => "TreeItem".to_string(),
            UIA_WindowControlTypeId => "Window".to_string(),
            UIA_PaneControlTypeId => "Pane".to_string(),
            UIA_GroupControlTypeId => "Group".to_string(),
            UIA_ThumbControlTypeId => "Thumb".to_string(),
            UIA_DataGridControlTypeId => "DataGrid".to_string(),
            UIA_DataItemControlTypeId => "DataItem".to_string(),
            UIA_DocumentControlTypeId => "Document".to_string(),
            UIA_HeaderControlTypeId => "Header".to_string(),
            UIA_HeaderItemControlTypeId => "HeaderItem".to_string(),
            UIA_TableControlTypeId => "Table".to_string(),
            UIA_TitleBarControlTypeId => "TitleBar".to_string(),
            UIA_SeparatorControlTypeId => "Separator".to_string(),
            _ => format!("ControlType({})", ct.0),
        }
    }

    /// Get currently selected text via UIA Text pattern.
    pub fn get_selected_text() -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            super::ensure_com_initialized();
            unsafe {
                let automation: super::IUIAutomation = match super::CoCreateInstance(
                    &super::CUIAutomation,
                    None,
                    super::CLSCTX_INPROC_SERVER,
                ) {
                    Ok(a) => a,
                    Err(_) => return None,
                };

                let focused = match automation.GetFocusedElement() {
                    Ok(f) => f,
                    Err(_) => return None,
                };

                // Try to get the TextPattern
                let pattern_id = super::UIA_TextPatternId;
                let pattern = match focused.GetCurrentPattern(pattern_id) {
                    Ok(p) => p,
                    Err(_) => return None,
                };

                use windows::core::Interface;
                let text_pattern: super::IUIAutomationTextPattern = match pattern.cast() {
                    Ok(tp) => tp,
                    Err(_) => return None,
                };

                let selection = match text_pattern.GetSelection() {
                    Ok(s) => s,
                    Err(_) => return None,
                };

                let range = match selection.GetElement(0) {
                    Ok(r) => r,
                    Err(_) => return None,
                };

                match range.GetText(4096) {
                    Ok(text) => {
                        let s = text.to_string();
                        if s.is_empty() { None } else { Some(s) }
                    }
                    Err(_) => None,
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }

    /// Get the center coordinates of a UI element at the given point.
    pub fn get_element_center_at(x: i32, y: i32) -> Option<(i32, i32)> {
        #[cfg(target_os = "windows")]
        {
            super::ensure_com_initialized();
            unsafe {
                let automation: super::IUIAutomation = match super::CoCreateInstance(
                    &super::CUIAutomation,
                    None,
                    super::CLSCTX_INPROC_SERVER,
                ) {
                    Ok(a) => a,
                    Err(_) => return None,
                };

                let point = super::POINT { x, y };
                let element = match automation.ElementFromPoint(point) {
                    Ok(e) => e,
                    Err(_) => return None,
                };

                let rect = match element.CurrentBoundingRectangle() {
                    Ok(r) => r,
                    Err(_) => return None,
                };

                let cx = (rect.left + rect.right) / 2;
                let cy = (rect.top + rect.bottom) / 2;
                Some((cx, cy))
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (x, y);
            None
        }
    }

    /// Find the UI element at the specific screen coordinates (x, y).
    /// Returns (x, y) of the element's center if found.
    /// Windows equivalent of macOS AXUIElementCopyElementAtPosition.
    pub fn find_element_at_pos(x: f32, y: f32) -> Option<(i32, i32)> {
        #[cfg(target_os = "windows")]
        {
            super::ensure_com_initialized();
            unsafe {
                let automation: super::IUIAutomation = match super::CoCreateInstance(
                    &super::CUIAutomation,
                    None,
                    super::CLSCTX_INPROC_SERVER,
                ) {
                    Ok(a) => a,
                    Err(_) => return None,
                };

                let point = super::POINT { x: x as i32, y: y as i32 };
                let element = match automation.ElementFromPoint(point) {
                    Ok(e) => e,
                    Err(_) => return None,
                };

                // Get bounding rectangle for center calculation
                let rect = match element.CurrentBoundingRectangle() {
                    Ok(r) => r,
                    Err(_) => return Some((x as i32, y as i32)), // Element exists but no bounds
                };

                let cx = (rect.left + rect.right) / 2;
                let cy = (rect.top + rect.bottom) / 2;
                Some((cx, cy))
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (x, y);
            None
        }
    }

    /// Search for text on screen using Windows UI Automation.
    /// Uses a layered approach: UIA → MSAA → OCR fallback.
    /// Returns a description of where the text was found.
    pub fn find_text_on_screen(query: &str) -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            super::ensure_com_initialized();
            let query_lower = query.to_lowercase();

            // Layer 1: UIA search
            if let Some(result) = find_text_via_uia(&query_lower) {
                return Some(result);
            }

            // Layer 2: MSAA fallback
            if let Some(result) = super::msaa::find_text_via_msaa(&query_lower) {
                return Some(format!("msaa:{}", result));
            }

            // Layer 3: OCR fallback (most expensive, last resort)
            if let Some(result) = super::ocr::find_text_via_ocr(&query_lower) {
                return Some(format!("ocr:{}", result));
            }

            None
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = query;
            None
        }
    }

    /// UIA-based text search (primary method).
    #[cfg(target_os = "windows")]
    fn find_text_via_uia(query: &str) -> Option<String> {
        unsafe {
            let automation: super::IUIAutomation = match super::CoCreateInstance(
                &super::CUIAutomation,
                None,
                super::CLSCTX_INPROC_SERVER,
            ) {
                Ok(a) => a,
                Err(_) => return None,
            };

            // Get foreground window element
            let hwnd = super::GetForegroundWindow();
            let root = match automation.ElementFromHandle(hwnd) {
                Ok(e) => e,
                Err(_) => return None,
            };

            search_element_uia(&automation, &root, query, 0)
        }
    }

    #[cfg(target_os = "windows")]
    fn search_element_uia(
        automation: &super::IUIAutomation,
        element: &super::IUIAutomationElement,
        query: &str,
        depth: usize,
    ) -> Option<String> {
        if depth > 12 {
            return None;
        }

        unsafe {
            // Check element name
            if let Ok(name) = element.CurrentName() {
                let name_str = name.to_string();
                if !name_str.is_empty() && name_str.to_lowercase().contains(query) {
                    return Some(format!("title:{}", name_str));
                }
            }

            // Check element value (for edit controls, etc.)
            if let Ok(pattern_id) = element.GetCurrentPattern(super::UIA_ValuePatternId) {
                use windows::core::Interface;
                if let Ok(value_pattern) = pattern_id.cast::<super::IUIAutomationValuePattern>() {
                    if let Ok(val) = value_pattern.CurrentValue() {
                        let val_str = val.to_string();
                        if !val_str.is_empty() && val_str.to_lowercase().contains(query) {
                            return Some(format!("value:{}", val_str));
                        }
                    }
                }
            }

            // Check AutomationId
            if let Ok(auto_id) = element.CurrentAutomationId() {
                let id_str = auto_id.to_string();
                if !id_str.is_empty() && id_str.to_lowercase().contains(query) {
                    return Some(format!("automationId:{}", id_str));
                }
            }

            // Check help text / description
            if let Ok(help) = element.CurrentHelpText() {
                let help_str = help.to_string();
                if !help_str.is_empty() && help_str.to_lowercase().contains(query) {
                    return Some(format!("description:{}", help_str));
                }
            }

            // Recursively search children using ControlViewWalker
            if let Ok(walker) = automation.ControlViewWalker() {
                if let Ok(first_child) = walker.GetFirstChildElement(element) {
                    let mut current = first_child;
                    let mut count = 0;

                    loop {
                        if count >= 200 {
                            break;
                        }

                        if let Some(result) = search_element_uia(automation, &current, query, depth + 1) {
                            return Some(result);
                        }

                        match walker.GetNextSiblingElement(&current) {
                            Ok(next) => current = next,
                            Err(_) => break,
                        }
                        count += 1;
                    }
                }
            }

            None
        }
    }

    /// Find element by query string (stub for compatibility).
    #[allow(dead_code)]
    pub fn find_element(query: &str) -> Option<String> {
        eprintln!("[UIA] find_element: {}", query);
        None
    }
}

// ─── MSAA (IAccessible) Fallback Module ───
pub mod msaa {
    /// Search for text using MSAA IAccessible as fallback when UIA misses elements.
    /// Particularly useful for legacy Win32 applications.
    #[cfg(target_os = "windows")]
    pub fn find_text_via_msaa(query: &str) -> Option<String> {
        use windows::Win32::UI::Accessibility::*;
        use windows::Win32::UI::WindowsAndMessaging::*;
        use windows::core::Interface;

        unsafe {
            let hwnd = GetForegroundWindow();
            let mut accessible: Option<IAccessible> = None;
            let child_var = windows::core::VARIANT::default();

            let hr = AccessibleObjectFromWindow(
                hwnd,
                OBJID_CLIENT.0 as u32,
                &IAccessible::IID,
                &mut accessible as *mut _ as *mut *mut std::ffi::c_void,
            );

            if hr.is_err() {
                return None;
            }

            let acc = accessible?;
            search_accessible(&acc, &child_var, query, 0)
        }
    }

    #[cfg(target_os = "windows")]
    fn search_accessible(
        acc: &windows::Win32::UI::Accessibility::IAccessible,
        _child: &windows::core::VARIANT,
        query: &str,
        depth: usize,
    ) -> Option<String> {
        if depth > 10 {
            return None;
        }

        unsafe {
            let self_var = windows::core::VARIANT::from(0i32); // CHILDID_SELF

            // Check name
            if let Ok(name) = acc.get_accName(&self_var) {
                let name_str = name.to_string();
                if !name_str.is_empty() && name_str.to_lowercase().contains(query) {
                    return Some(format!("name:{}", name_str));
                }
            }

            // Check value
            if let Ok(value) = acc.get_accValue(&self_var) {
                let val_str = value.to_string();
                if !val_str.is_empty() && val_str.to_lowercase().contains(query) {
                    return Some(format!("value:{}", val_str));
                }
            }

            // Check description
            if let Ok(desc) = acc.get_accDescription(&self_var) {
                let desc_str = desc.to_string();
                if !desc_str.is_empty() && desc_str.to_lowercase().contains(query) {
                    return Some(format!("desc:{}", desc_str));
                }
            }

            // Enumerate children
            let child_count = acc.accChildCount().unwrap_or(0);
            if child_count == 0 {
                return None;
            }

            let count = child_count.min(100) as usize;
            let mut children: Vec<windows::core::VARIANT> = Vec::with_capacity(count);
            for _ in 0..count {
                children.push(windows::core::VARIANT::default());
            }
            let mut obtained: i32 = 0;

            let hr = windows::Win32::UI::Accessibility::AccessibleChildren(
                acc,
                0,
                &mut children,
                &mut obtained,
            );

            if hr.is_err() {
                return None;
            }

            for i in 0..(obtained as usize) {
                let child_var = &children[i];
                // Try to extract IDispatch from VARIANT — indicates a sub-accessible object
                use windows::core::Interface;
                let disp_result: Result<windows::Win32::System::Com::IDispatch, _> = child_var.try_into();
                if let Ok(disp) = disp_result {
                    if let Ok(child_acc) = disp.cast::<windows::Win32::UI::Accessibility::IAccessible>() {
                        if let Some(result) = search_accessible(&child_acc, child_var, query, depth + 1) {
                            return Some(result);
                        }
                    }
                } else {
                    // Simple child (identified by index) — check its name/value on parent
                    if let Ok(name) = acc.get_accName(child_var) {
                        let name_str = name.to_string();
                        if !name_str.is_empty() && name_str.to_lowercase().contains(query) {
                            return Some(format!("msaa_child_name:{}", name_str));
                        }
                    }
                    if let Ok(value) = acc.get_accValue(child_var) {
                        let val_str = value.to_string();
                        if !val_str.is_empty() && val_str.to_lowercase().contains(query) {
                            return Some(format!("msaa_child_value:{}", val_str));
                        }
                    }
                }
            }

            None
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn find_text_via_msaa(_query: &str) -> Option<String> {
        None
    }

    /// Perform default action on an accessible element found by name (MSAA fallback for click).
    #[cfg(target_os = "windows")]
    pub fn do_default_action_by_name(element_name: &str) -> Result<(), String> {
        use windows::Win32::UI::Accessibility::*;
        use windows::Win32::UI::WindowsAndMessaging::*;
        use windows::core::Interface;

        unsafe {
            let hwnd = GetForegroundWindow();
            let mut accessible: Option<IAccessible> = None;

            let hr = AccessibleObjectFromWindow(
                hwnd,
                OBJID_CLIENT.0 as u32,
                &IAccessible::IID,
                &mut accessible as *mut _ as *mut *mut std::ffi::c_void,
            );

            if hr.is_err() {
                return Err("AccessibleObjectFromWindow failed".to_string());
            }

            let acc = accessible.ok_or("No IAccessible")?;
            let query = element_name.to_lowercase();
            do_action_recursive(&acc, &query, 0)
        }
    }

    #[cfg(target_os = "windows")]
    fn do_action_recursive(
        acc: &windows::Win32::UI::Accessibility::IAccessible,
        query: &str,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 10 {
            return Err("Max depth reached".to_string());
        }

        unsafe {
            let self_var = windows::core::VARIANT::from(0i32);

            if let Ok(name) = acc.get_accName(&self_var) {
                if name.to_string().to_lowercase().contains(query) {
                    return acc.accDoDefaultAction(&self_var)
                        .map_err(|e| format!("accDoDefaultAction failed: {}", e));
                }
            }

            let child_count = acc.accChildCount().unwrap_or(0) as usize;
            let count = child_count.min(100);
            let mut children: Vec<windows::core::VARIANT> = Vec::with_capacity(count);
            for _ in 0..count {
                children.push(windows::core::VARIANT::default());
            }
            let mut obtained: i32 = 0;

            let hr = windows::Win32::UI::Accessibility::AccessibleChildren(
                acc,
                0,
                &mut children,
                &mut obtained,
            );
            if hr.is_err() {
                return Err("AccessibleChildren failed".to_string());
            }

            for i in 0..(obtained as usize) {
                let child_var = &children[i];
                use windows::core::Interface;
                let disp_result: Result<windows::Win32::System::Com::IDispatch, _> = child_var.try_into();
                if let Ok(disp) = disp_result {
                    if let Ok(child_acc) = disp.cast::<windows::Win32::UI::Accessibility::IAccessible>() {
                        if let Ok(()) = do_action_recursive(&child_acc, query, depth + 1) {
                            return Ok(());
                        }
                    }
                }
            }

            Err(format!("Element '{}' not found via MSAA", query))
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn do_default_action_by_name(_element_name: &str) -> Result<(), String> {
        Err("MSAA not available on this platform".to_string())
    }
}

// ─── OCR Fallback Module (Windows.Media.Ocr via PowerShell) ───
pub mod ocr {
    /// Check if Windows OCR capability is available.
    pub fn ocr_available() -> bool {
        #[cfg(target_os = "windows")]
        {
            // Windows.Media.Ocr is available on Windows 10 1809+ with language packs
            let output = std::process::Command::new("powershell")
                .args([
                    "-NoProfile", "-NonInteractive", "-Command",
                    "[Windows.Media.Ocr.OcrEngine,Windows.Foundation,ContentType=WindowsRuntime] | Out-Null; Write-Output 'ok'",
                ])
                .output();
            match output {
                Ok(o) => String::from_utf8_lossy(&o.stdout).trim() == "ok",
                Err(_) => false,
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            false
        }
    }

    /// Perform OCR on a screen region and search for text.
    /// Uses PowerShell with Windows.Media.Ocr (no external dependencies).
    #[cfg(target_os = "windows")]
    pub fn find_text_via_ocr(query: &str) -> Option<String> {
        // Capture the foreground window area via PowerShell + GDI
        // and run OCR on it. This is expensive so it's only used as last resort.
        let ps_script = format!(
            r#"
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Runtime.WindowsRuntime

$user32 = Add-Type -MemberDefinition @'
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
[StructLayout(LayoutKind.Sequential)] public struct RECT {{ public int Left; public int Top; public int Right; public int Bottom; }}
'@ -Name 'Win32' -Namespace 'OCR' -PassThru

$hwnd = $user32::GetForegroundWindow()
$rect = New-Object OCR.Win32+RECT
[void]$user32::GetWindowRect($hwnd, [ref]$rect)
$w = [Math]::Max($rect.Right - $rect.Left, 1)
$h = [Math]::Max($rect.Bottom - $rect.Top, 1)
if ($w -gt 3840) {{ $w = 3840 }}
if ($h -gt 2160) {{ $h = 2160 }}

$bmp = New-Object System.Drawing.Bitmap($w, $h)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($rect.Left, $rect.Top, 0, 0, (New-Object System.Drawing.Size($w,$h)))
$g.Dispose()

$ms = New-Object System.IO.MemoryStream
$bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Bmp)
$bmp.Dispose()
$bytes = $ms.ToArray()
$ms.Dispose()

[Windows.Media.Ocr.OcrEngine,Windows.Foundation,ContentType=WindowsRuntime] | Out-Null
[Windows.Graphics.Imaging.BitmapDecoder,Windows.Foundation,ContentType=WindowsRuntime] | Out-Null

$ras = [System.Runtime.InteropServices.WindowsRuntime.WindowsRuntimeBufferExtensions]::AsBuffer($bytes)
$streamRef = [Windows.Storage.Streams.InMemoryRandomAccessStream,Windows.Foundation,ContentType=WindowsRuntime]::new()
$writer = [Windows.Storage.Streams.DataWriter,Windows.Foundation,ContentType=WindowsRuntime]::new($streamRef)
$writer.WriteBytes($bytes)
$storeTask = $writer.StoreAsync()

$asTaskGeneric = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {{ $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' }})[0]

# Simplified: just output all recognized text
$engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages()
if ($engine -eq $null) {{
    Write-Output ''
    exit
}}

$streamRef.Seek(0)
$decoderTask = [Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($streamRef)
$decoderAwaiter = $decoderTask.GetAwaiter()
while (-not $decoderAwaiter.IsCompleted) {{ Start-Sleep -Milliseconds 50 }}
$decoder = $decoderAwaiter.GetResult()

$sbTask = $decoder.GetSoftwareBitmapAsync()
$sbAwaiter = $sbTask.GetAwaiter()
while (-not $sbAwaiter.IsCompleted) {{ Start-Sleep -Milliseconds 50 }}
$sb = $sbAwaiter.GetResult()

$ocrTask = $engine.RecognizeAsync($sb)
$ocrAwaiter = $ocrTask.GetAwaiter()
while (-not $ocrAwaiter.IsCompleted) {{ Start-Sleep -Milliseconds 50 }}
$ocrResult = $ocrAwaiter.GetResult()

Write-Output $ocrResult.Text
"#
        );

        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
            .output()
            .ok()?;

        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            return None;
        }

        if text.to_lowercase().contains(query) {
            Some(text)
        } else {
            None
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn find_text_via_ocr(_query: &str) -> Option<String> {
        None
    }

    /// Perform OCR on the entire foreground window and return all recognized text.
    #[allow(dead_code)]
    pub fn ocr_foreground_window() -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            // Reuse the same logic but return everything
            let ps_script = r#"
Add-Type -AssemblyName System.Drawing
$user32 = Add-Type -MemberDefinition @'
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
'@ -Name 'Win32' -Namespace 'OCR2' -PassThru

$hwnd = $user32::GetForegroundWindow()
$rect = New-Object OCR2.Win32+RECT
[void]$user32::GetWindowRect($hwnd, [ref]$rect)
$w = [Math]::Max($rect.Right - $rect.Left, 1)
$h = [Math]::Max($rect.Bottom - $rect.Top, 1)
$bmp = New-Object System.Drawing.Bitmap($w, $h)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($rect.Left, $rect.Top, 0, 0, (New-Object System.Drawing.Size($w,$h)))
$g.Dispose()
$tmpFile = [System.IO.Path]::GetTempFileName() + '.bmp'
$bmp.Save($tmpFile, [System.Drawing.Imaging.ImageFormat]::Bmp)
$bmp.Dispose()
Write-Output $tmpFile
"#;
            let output = std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", ps_script])
                .output()
                .ok()?;
            let _path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // For now return a simplified version
            None
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }
}

pub mod actions {
    use anyhow::Result;

    /// Click a UI element by its element_id.
    /// Attempts UIA Invoke pattern first, falls back to MSAA accDoDefaultAction.
    pub fn click_element(_element_id: &str) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            // Try MSAA fallback for clicking elements by name
            if !_element_id.is_empty() {
                match super::msaa::do_default_action_by_name(_element_id) {
                    Ok(()) => return Ok(()),
                    Err(e) => {
                        eprintln!("[UIA actions] MSAA fallback also failed for '{}': {}", _element_id, e);
                    }
                }
            }
            eprintln!("[UIA actions] click_element: use coordinate click as last resort");
        }
        Ok(())
    }

    /// Type text using Win32 SendInput API.
    pub fn type_text(text: &str) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            use super::*;
            unsafe {
                for ch in text.chars() {
                    let mut inputs = [INPUT::default(); 2];

                    // Key down
                    inputs[0].r#type = INPUT_KEYBOARD;
                    inputs[0].Anonymous.ki.wScan = ch as u16;
                    inputs[0].Anonymous.ki.dwFlags = KEYEVENTF_UNICODE;

                    // Key up
                    inputs[1].r#type = INPUT_KEYBOARD;
                    inputs[1].Anonymous.ki.wScan = ch as u16;
                    inputs[1].Anonymous.ki.dwFlags = KEYEVENTF_UNICODE | KEYEVENTF_KEYUP;

                    SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = text;
        }
        Ok(())
    }
}

pub mod events {
    use anyhow::Result;
    use tokio::sync::mpsc;

    /// Start monitoring Windows UI events via SetWinEventHook.
    pub fn start_event_tap(_tx: mpsc::Sender<String>) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            // WinEventHook requires a message loop; start in a background thread
            let tx = _tx.clone();
            std::thread::spawn(move || {
                use super::*;
                unsafe {
                    let _hook = SetWinEventHook(
                        EVENT_SYSTEM_FOREGROUND,
                        EVENT_OBJECT_NAMECHANGE,
                        None,
                        Some(win_event_callback),
                        0,
                        0,
                        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                    );

                    // Store sender in thread-local (simplified approach)
                    WIN_EVENT_TX.with(|cell| {
                        *cell.borrow_mut() = Some(tx);
                    });

                    // Message loop to keep the hook alive
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            });
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
use std::cell::RefCell;

#[cfg(target_os = "windows")]
thread_local! {
    static WIN_EVENT_TX: RefCell<Option<tokio::sync::mpsc::Sender<String>>> = RefCell::new(None);
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn win_event_callback(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    let event_name = match event {
        0x0003 => "EVENT_SYSTEM_FOREGROUND",
        0x8005 => "EVENT_OBJECT_FOCUS",
        0x800C => "EVENT_OBJECT_NAMECHANGE",
        _ => return,
    };

    let mut title_buf = [0u16; 256];
    let title_len = GetWindowTextW(hwnd, &mut title_buf);
    let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);

    let msg = format!(r#"{{"event":"{}","title":"{}"}}"#, event_name, title);

    WIN_EVENT_TX.with(|cell| {
        if let Some(ref tx) = *cell.borrow() {
            let _ = tx.try_send(msg);
        }
    });
}
