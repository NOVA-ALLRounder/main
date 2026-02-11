use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
use core_graphics::event_source::CGEventSource;
use core_graphics::event_source::CGEventSourceStateID;
use core_graphics::geometry::CGPoint;
use std::{thread, time::Duration};
use crate::platform::traits::InputSimulator;
use anyhow::Result;

pub fn click_element(element_id: &str) -> anyhow::Result<()> {
    // Implementing "Click by ID" purely with coords is hard without the Accessibility Object.
    // In native mode, we usually pass coordinates or AXUIElementRef.
    // For MVP, if "element_id" is essentially ignored or we need to look it up.
    // Let's assume for now we cannot click easily without coordinates.
    // But since this is a "Behavior" tool, maybe we just log it?
    // Or we implement a "click at (x,y)" helper.
    
    // For now, let's just fail if we don't have coords, or hardcode a "center" click if valid.
    println!("[MacOS] Click Element '{}' (Not fully resolved in MVP)", element_id);
    // Real implementation requires finding the element first (AXUIElement) then getting its center.
    // We haven't linked find_element yet.
    Ok(())
}

pub fn type_text(text: &str) -> anyhow::Result<()> {
    println!("[MacOS] Typping: {}", text);

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| anyhow::anyhow!("Failed to create event source"))?;

    for c in text.chars() {
        // Very basic mapping. Dealing with keycodes is complex.
        // We'll trust CGEventKeyboardSetUnicodeString which is easier than mapping keycodes manually.

        // 1. Key Down
        if let Ok(event) = CGEvent::new_keyboard_event(source.clone(), 0, true) {
            event.set_string(&c.to_string());
            event.post(CGEventTapLocation::HID);
        }

        thread::sleep(Duration::from_millis(10));

        // 2. Key Up
        if let Ok(event) = CGEvent::new_keyboard_event(source.clone(), 0, false) {
             event.set_string(&c.to_string());
             event.post(CGEventTapLocation::HID);
        }

        thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}

/// macOS Input Simulator implementation
pub struct MacOSInputSimulator;

impl InputSimulator for MacOSInputSimulator {
    fn click_at(&self, x: i32, y: i32) -> Result<()> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| anyhow::anyhow!("Failed to create event source"))?;

        let point = CGPoint::new(x as f64, y as f64);

        // Mouse down
        let mouse_down = CGEvent::new_mouse_event(
            source.clone(),
            CGEventType::LeftMouseDown,
            point,
            CGMouseButton::Left,
        )
        .map_err(|_| anyhow::anyhow!("Failed to create mouse down event"))?;
        mouse_down.post(CGEventTapLocation::HID);

        thread::sleep(Duration::from_millis(50));

        // Mouse up
        let mouse_up = CGEvent::new_mouse_event(
            source,
            CGEventType::LeftMouseUp,
            point,
            CGMouseButton::Left,
        )
        .map_err(|_| anyhow::anyhow!("Failed to create mouse up event"))?;
        mouse_up.post(CGEventTapLocation::HID);

        Ok(())
    }

    fn type_text(&self, text: &str) -> Result<()> {
        type_text(text)
    }

    fn press_key(&self, key_code: u32) -> Result<()> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| anyhow::anyhow!("Failed to create event source"))?;

        // Key down
        let key_down = CGEvent::new_keyboard_event(source.clone(), key_code as u16, true)
            .map_err(|_| anyhow::anyhow!("Failed to create key down event"))?;
        key_down.post(CGEventTapLocation::HID);

        thread::sleep(Duration::from_millis(10));

        // Key up
        let key_up = CGEvent::new_keyboard_event(source, key_code as u16, false)
            .map_err(|_| anyhow::anyhow!("Failed to create key up event"))?;
        key_up.post(CGEventTapLocation::HID);

        Ok(())
    }

    fn scroll(&self, direction: &str, amount: i32) -> Result<()> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| anyhow::anyhow!("Failed to create event source"))?;

        let (wheel1, wheel2) = match direction {
            "up" => (amount, 0),
            "down" => (-amount, 0),
            "left" => (0, amount),
            "right" => (0, -amount),
            _ => return Err(anyhow::anyhow!("Invalid scroll direction: {}", direction)),
        };

        let scroll_event = CGEvent::new_scroll_event(source, 0, 2, wheel1, wheel2, 0)
            .map_err(|_| anyhow::anyhow!("Failed to create scroll event"))?;
        scroll_event.post(CGEventTapLocation::HID);

        Ok(())
    }
}

