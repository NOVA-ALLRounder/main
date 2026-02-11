/// Windows UI Automation implementation

use crate::platform::traits::AccessibilityProvider;
use anyhow::Result;
use serde_json::{json, Value};
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Windows Accessibility Provider using UI Automation
pub struct WindowsAccessibility {
    automation: IUIAutomation,
}

// SAFETY: IUIAutomation is thread-safe when used with COM apartment threading
unsafe impl Send for WindowsAccessibility {}
unsafe impl Sync for WindowsAccessibility {}

impl WindowsAccessibility {
    pub fn new() -> Result<Self> {
        unsafe {
            let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)
                .map_err(|e| anyhow::anyhow!("Failed to create UI Automation: {:?}", e))?;

            Ok(Self { automation })
        }
    }
}

impl AccessibilityProvider for WindowsAccessibility {
    fn snapshot(&self, _scope: Option<String>) -> Result<Value> {
        unsafe {
            // Get foreground window element
            let hwnd = GetForegroundWindow();
            if hwnd.0 == 0 {
                return Err(anyhow::anyhow!("No foreground window"));
            }

            let element = self.automation.ElementFromHandle(hwnd)
                .map_err(|e| anyhow::anyhow!("Failed to get element from window: {:?}", e))?;

            // Get window title
            let name_bstr = element.CurrentName()
                .map_err(|e| anyhow::anyhow!("Failed to get window name: {:?}", e))?;
            let window_title = name_bstr.to_string();

            // Traverse children (limit depth for performance)
            let children = traverse_children(&element, 0, 2)?;

            Ok(json!({
                "role": "Window",
                "title": window_title,
                "children": children
            }))
        }
    }

    fn get_selected_text(&self) -> Result<Option<String>> {
        unsafe {
            let focused = self.automation.GetFocusedElement()
                .map_err(|e| anyhow::anyhow!("Failed to get focused element: {:?}", e))?;

            // Try to get text pattern
            let pattern_result: windows::core::Result<IUIAutomationTextPattern> = focused.GetCurrentPatternAs(UIA_TextPatternId);

            match pattern_result {
                Ok(pattern) => {
                    let selections = pattern.GetSelection()
                        .map_err(|e| anyhow::anyhow!("Failed to get selection: {:?}", e))?;

                    let length = selections.Length()
                        .map_err(|e| anyhow::anyhow!("Failed to get selection length: {:?}", e))?;

                    if length > 0 {
                        let range = selections.GetElement(0)
                            .map_err(|e| anyhow::anyhow!("Failed to get selection range: {:?}", e))?;

                        let text = range.GetText(-1)
                            .map_err(|e| anyhow::anyhow!("Failed to get text: {:?}", e))?;

                        Ok(Some(text.to_string()))
                    } else {
                        Ok(None)
                    }
                }
                Err(_) => Ok(None),
            }
        }
    }

    fn element_exists_at(&self, x: i32, y: i32) -> Result<bool> {
        unsafe {
            let point = POINT { x, y };
            let result = self.automation.ElementFromPoint(point);
            Ok(result.is_ok())
        }
    }

    fn get_element_info(&self, x: i32, y: i32) -> Result<Option<Value>> {
        unsafe {
            let point = POINT { x, y };
            let element = match self.automation.ElementFromPoint(point) {
                Ok(elem) => elem,
                Err(_) => return Ok(None),
            };

            let name = element.CurrentName().ok().map(|b| b.to_string()).unwrap_or_default();
            let control_type = element.CurrentControlType().ok().unwrap_or(UIA_CustomControlTypeId);
            let value = match element.GetCurrentPropertyValue(UIA_ValueValuePropertyId) {
                Ok(variant) => {
                    let bstr = &variant.Anonymous.Anonymous.Anonymous.bstrVal;
                    if !bstr.is_empty() {
                        bstr.to_string()
                    } else {
                        String::new()
                    }
                }
                Err(_) => String::new(),
            };

            let role = control_type_to_string(control_type.0);

            let mut info = json!({
                "role": role,
            });

            if !name.is_empty() {
                info["title"] = json!(name);
            }
            if !value.is_empty() {
                info["value"] = json!(value);
            }

            Ok(Some(info))
        }
    }
}

/// Traverse UI element children recursively
unsafe fn traverse_children(element: &IUIAutomationElement, depth: usize, max_depth: usize) -> Result<Vec<Value>> {
    if depth > max_depth {
        return Ok(vec![]);
    }

    let mut nodes = Vec::new();

    // Get tree walker
    let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)?;
    let walker = automation.ControlViewWalker()?;

    // Get first child
    let mut child_result = walker.GetFirstChildElement(element);
    while let Ok(child) = child_result {
        let name = child.CurrentName().ok().map(|b| b.to_string()).unwrap_or_default();
        let control_type = child.CurrentControlType().ok().unwrap_or(UIA_CustomControlTypeId);
        let value = match child.GetCurrentPropertyValue(UIA_ValueValuePropertyId) {
            Ok(variant) => {
                let bstr = &variant.Anonymous.Anonymous.Anonymous.bstrVal;
                if !bstr.is_empty() {
                    bstr.to_string()
                } else {
                    String::new()
                }
            }
            Err(_) => String::new(),
        };

        let role = control_type_to_string(control_type.0);

        // Recursively get children
        let sub_children = if depth < max_depth {
            traverse_children(&child, depth + 1, max_depth).unwrap_or_default()
        } else {
            vec![]
        };

        let mut node = json!({
            "role": role,
            "children": sub_children
        });

        if !name.is_empty() {
            node["title"] = json!(name);
        }
        if !value.is_empty() {
            node["value"] = json!(value);
        }

        nodes.push(node);

        // Get next sibling
        child_result = walker.GetNextSiblingElement(&child);
    }

    Ok(nodes)
}

/// Convert UI Automation control type to string
fn control_type_to_string(control_type: u32) -> String {
    match control_type {
        v if v == UIA_ButtonControlTypeId.0 => "Button",
        v if v == UIA_TextControlTypeId.0 => "Text",
        v if v == UIA_EditControlTypeId.0 => "Edit",
        v if v == UIA_WindowControlTypeId.0 => "Window",
        v if v == UIA_PaneControlTypeId.0 => "Pane",
        v if v == UIA_MenuControlTypeId.0 => "Menu",
        v if v == UIA_MenuItemControlTypeId.0 => "MenuItem",
        v if v == UIA_ListControlTypeId.0 => "List",
        v if v == UIA_ListItemControlTypeId.0 => "ListItem",
        v if v == UIA_CheckBoxControlTypeId.0 => "CheckBox",
        v if v == UIA_RadioButtonControlTypeId.0 => "RadioButton",
        v if v == UIA_ComboBoxControlTypeId.0 => "ComboBox",
        v if v == UIA_ScrollBarControlTypeId.0 => "ScrollBar",
        v if v == UIA_ImageControlTypeId.0 => "Image",
        v if v == UIA_HyperlinkControlTypeId.0 => "Hyperlink",
        v if v == UIA_TabControlTypeId.0 => "Tab",
        v if v == UIA_TabItemControlTypeId.0 => "TabItem",
        v if v == UIA_ToolBarControlTypeId.0 => "ToolBar",
        _ => "Unknown",
    }
    .to_string()
}

/// Stub accessibility provider (fallback if initialization fails)
pub struct StubAccessibility;

impl AccessibilityProvider for StubAccessibility {
    fn snapshot(&self, _scope: Option<String>) -> Result<Value> {
        Ok(json!({"error": "UI Automation not available"}))
    }

    fn get_selected_text(&self) -> Result<Option<String>> {
        Ok(None)
    }

    fn element_exists_at(&self, _x: i32, _y: i32) -> Result<bool> {
        Ok(false)
    }

    fn get_element_info(&self, _x: i32, _y: i32) -> Result<Option<Value>> {
        Ok(None)
    }
}
