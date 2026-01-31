use serde_json::{json, Value};
use accessibility_sys::{
    AXUIElementCopyAttributeValue, AXUIElementCreateSystemWide, AXUIElementRef,
};
use core_foundation::base::{CFTypeRef, TCFType};
use core_foundation::string::CFString;
use core_foundation::array::CFArray;
use std::ptr;

// Helper to convert foreign AX error to Result
#[allow(dead_code)]
fn check_ax_err(err: i32) -> Result<(), i32> {
    if err == 0 { Ok(()) } else { Err(err) }
}

// Helper to get attribute
fn get_attribute(element: AXUIElementRef, attribute: &str) -> Option<CFTypeRef> {
    unsafe {
        let attr_name = CFString::new(attribute);
        let mut value_ref: CFTypeRef = ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(element, attr_name.as_concrete_TypeRef(), &mut value_ref);
        if err == 0 {
            Some(value_ref)
        } else {
            None
        }
    }
}

// Minimal wrapper for memory safety
struct AxElement(AXUIElementRef);
impl Drop for AxElement {
    fn drop(&mut self) {
        unsafe { core_foundation::base::CFRelease(self.0 as CFTypeRef); }
    }
}

pub fn snapshot(_scope: Option<String>) -> Value {
    println!("[MacOS] Capturing Snapshot (Native)...");

    unsafe {
        // 1. System Wide
        let system_wide = AXUIElementCreateSystemWide();
        let _system_wrapper = AxElement(system_wide); // Auto-release

        // 2. Focused App
        let focused_app_ref = match get_attribute(system_wide, "AXFocusedApplication") {
            Some(r) => r as AXUIElementRef,
            None => return json!({ "error": "No focused application" }),
        };
        // Note: get_attribute returns +1 retain count, so we wrap it
        let _focused_app = AxElement(focused_app_ref);

        // Get App Title
        let app_title = get_string_attribute(focused_app_ref, "AXTitle").unwrap_or_default();

        // 3. Focused Window
        let focused_window_ref = match get_attribute(focused_app_ref, "AXFocusedWindow") {
             Some(r) => r as AXUIElementRef,
             None => return json!({ "role": "AXApplication", "title": app_title, "error": "No focused window" }),
        };
        let _focused_window = AxElement(focused_window_ref);
        
        let window_title = get_string_attribute(focused_window_ref, "AXTitle").unwrap_or_default();
        
        // 4. Traverse Children (Limit depth for MVP)
        // For performance, we only dump the focused window's children.
        let children_json = traverse_children(focused_window_ref, 0, 2);

        json!({
            "role": "AXApplication",
            "title": app_title,
            "focused_window": {
                "role": "AXWindow",
                "title": window_title,
                "children": children_json
            }
        })
    }
}

unsafe fn traverse_children(element: AXUIElementRef, depth: usize, max_depth: usize) -> Vec<Value> {
    if depth > max_depth { return vec![]; }
    
    let mut nodes = Vec::new();
    
    if let Some(children_ref) = get_attribute(element, "AXChildren") {
        let children_array = CFArray::<CFTypeRef>::wrap_under_get_rule(children_ref as core_foundation::array::CFArrayRef);
        
        for i in 0..children_array.len() {
             let Some(child_ptr) = children_array.get(i) else { continue; };
             let child_element = *child_ptr as AXUIElementRef;
             
             let role = get_string_attribute(child_element, "AXRole").unwrap_or_default();
             let title = get_string_attribute(child_element, "AXTitle").unwrap_or_default();
             let value = get_string_attribute(child_element, "AXValue").unwrap_or_default();
             
             // Recursion
             let sub_children = if depth < max_depth {
                 traverse_children(child_element, depth + 1, max_depth)
             } else {
                 vec![]
             };
             
             let mut node = json!({
                 "role": role,
                 "children": sub_children
             });
             
             if !title.is_empty() { node["title"] = json!(title); }
             if !value.is_empty() { node["value"] = json!(value); }
             
             nodes.push(node);
        }
        // Release the array ref
        core_foundation::base::CFRelease(children_ref);
    }
    
    nodes
}

unsafe fn get_string_attribute(element: AXUIElementRef, attr: &str) -> Option<String> {
    if let Some(val_ref) = get_attribute(element, attr) {
        // Assume it's a string
        // Check ID?
        let cf_str = CFString::wrap_under_create_rule(val_ref as core_foundation::string::CFStringRef);
        Some(cf_str.to_string())
    } else {
        None
    }
}

#[allow(dead_code)]
pub fn find_element(query: &str) -> Option<String> {
    println!("[MacOS] Find element (Not impl in MVP): {}", query);
    None
}

/// Get the currently selected text from the frontmost application.
/// Uses AppleScript via `osascript` for maximum compatibility.
pub fn get_selected_text() -> Option<String> {
    // Strategy 1: Try AXSelectedText attribute via System Events (Cleaner)
    // Strategy 2: If fails, we might need Cmd+C simulation (Risky/Intrusive), so let's stick to AX first.
    let script = r#"
        tell application "System Events"
            set frontApp to first application process whose frontmost is true
            set appName to name of frontApp
            
            try
                tell frontApp
                    -- Try focused UI element first
                    set focusedElement to value of attribute "AXFocusedUIElement"
                    if focusedElement is not missing value then
                         set selectedText to value of attribute "AXSelectedText" of focusedElement
                         if selectedText is not missing value and selectedText is not "" then
                             return selectedText
                         end if
                    end if
                end tell
            end try
            
            -- Fallback for some editors (like simple text fields) if AXFocusedUIElement fails
            return ""
        end tell
    "#;

    use std::process::Command;
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() || text == "missing value" {
            None
        } else {
            Some(text)
        }
    } else {
        None
    }
}
