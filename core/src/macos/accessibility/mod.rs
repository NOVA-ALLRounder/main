use accessibility_sys::{
    AXUIElementCopyAttributeValue, AXUIElementCopyElementAtPosition, AXUIElementCreateApplication,
    AXUIElementCreateSystemWide, AXUIElementRef,
};
use core_foundation::array::CFArray;
use core_foundation::base::{CFTypeRef, TCFType};
use core_foundation::string::CFString;
use serde_json::Value;
use std::process::Command;
use std::ptr;
use std::thread;
use std::time::Duration;

mod config;
mod focus;
mod search;
mod snapshot;

pub use search::{
    find_element, find_element_at_pos, find_text_on_screen, get_element_center_at,
    get_selected_text,
};
pub use snapshot::snapshot;

#[allow(dead_code)]
fn check_ax_err(err: i32) -> Result<(), i32> {
    if err == 0 {
        Ok(())
    } else {
        Err(err)
    }
}

pub(super) fn get_attribute(element: AXUIElementRef, attribute: &str) -> Option<CFTypeRef> {
    unsafe {
        let attr_name = CFString::new(attribute);
        let mut value_ref: CFTypeRef = ptr::null_mut();
        let err =
            AXUIElementCopyAttributeValue(element, attr_name.as_concrete_TypeRef(), &mut value_ref);
        if err == 0 {
            Some(value_ref)
        } else {
            None
        }
    }
}

pub(super) struct AxElement(pub(super) AXUIElementRef);
impl Drop for AxElement {
    fn drop(&mut self) {
        unsafe {
            core_foundation::base::CFRelease(self.0 as CFTypeRef);
        }
    }
}

pub(super) unsafe fn get_string_attribute(element: AXUIElementRef, attr: &str) -> Option<String> {
    if let Some(val_ref) = get_attribute(element, attr) {
        let cf_str =
            CFString::wrap_under_create_rule(val_ref as core_foundation::string::CFStringRef);
        Some(cf_str.to_string())
    } else {
        None
    }
}
