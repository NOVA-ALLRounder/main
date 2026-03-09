use super::{focus::resolve_focused_application, *};

#[allow(dead_code)]
pub fn find_element(query: &str) -> Option<String> {
    println!("[MacOS] Find element (Not impl in MVP): {}", query);
    None
}

pub fn get_selected_text() -> Option<String> {
    let script = r#"
        tell application "System Events"
            set frontApp to first application process whose frontmost is true
            set appName to name of frontApp
            
            try
                tell frontApp
                    set focusedElement to value of attribute "AXFocusedUIElement"
                    if focusedElement is not missing value then
                         set selectedText to value of attribute "AXSelectedText" of focusedElement
                         if selectedText is not missing value and selectedText is not "" then
                             return selectedText
                         end if
                    end if
                end tell
            end try
            
            return ""
        end tell
    "#;

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

pub fn find_element_at_pos(x: f32, y: f32) -> Option<(i32, i32)> {
    unsafe {
        let system_wide = AXUIElementCreateSystemWide();
        let _system = AxElement(system_wide);

        let mut element_ref: AXUIElementRef = ptr::null_mut();
        let err = AXUIElementCopyElementAtPosition(system_wide, x, y, &mut element_ref);
        if err != 0 {
            return None;
        }

        let _element = AxElement(element_ref);

        let _pos_val = get_attribute(element_ref, "AXPosition")?;
        let _size_val = get_attribute(element_ref, "AXSize")?;

        None
    }
}

pub fn get_element_center_at(x: i32, y: i32) -> Option<(i32, i32)> {
    let _script = format!(
        r#"
        use framework "CoreGraphics"
        use scripting additions
        
        tell application "System Events"
            set targetList to value of attribute "AXChildren" of (element at {{ {}, {} }})
        end tell
        "#,
        x, y
    );

    unsafe {
        let system_wide = AXUIElementCreateSystemWide();
        let _system = AxElement(system_wide);
        let mut element_ref: AXUIElementRef = ptr::null_mut();
        let err =
            AXUIElementCopyElementAtPosition(system_wide, x as f32, y as f32, &mut element_ref);
        if err == 0 && !element_ref.is_null() {
            let _element = AxElement(element_ref);
            Some((x, y))
        } else {
            None
        }
    }
}

pub fn find_text_on_screen(query: &str) -> Option<String> {
    let query_lower = query.to_lowercase();

    unsafe {
        let system_wide = AXUIElementCreateSystemWide();
        if system_wide.is_null() {
            return None;
        }
        let _system = AxElement(system_wide);

        let focused_app = resolve_focused_application(system_wide)?;
        let _focused_app_guard = AxElement(focused_app);

        fn search_element(element: AXUIElementRef, query: &str, depth: usize) -> Option<String> {
            if depth > 8 {
                return None;
            }

            unsafe {
                let title = get_string_attribute(element, "AXTitle").unwrap_or_default();
                let value = get_string_attribute(element, "AXValue").unwrap_or_default();
                let description =
                    get_string_attribute(element, "AXDescription").unwrap_or_default();

                if title.to_lowercase().contains(query) {
                    return Some(format!("title:{}", title));
                }
                if value.to_lowercase().contains(query) {
                    return Some(format!("value:{}", value));
                }
                if description.to_lowercase().contains(query) {
                    return Some(format!("description:{}", description));
                }

                if let Some(children_ref) = get_attribute(element, "AXChildren") {
                    let children_array = CFArray::<CFTypeRef>::wrap_under_get_rule(
                        children_ref as core_foundation::array::CFArrayRef,
                    );

                    for i in 0..children_array.len() {
                        if let Some(child_ptr) = children_array.get(i) {
                            let child = *child_ptr as AXUIElementRef;
                            if let Some(result) = search_element(child, query, depth + 1) {
                                core_foundation::base::CFRelease(children_ref);
                                return Some(result);
                            }
                        }
                    }
                    core_foundation::base::CFRelease(children_ref);
                }

                None
            }
        }

        search_element(focused_app, &query_lower, 0)
    }
}
