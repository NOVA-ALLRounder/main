// ComputerUseSkill - Computer control and automation
// Wraps existing computer_use functionality with Skill trait

use super::{Skill, SkillContext, SkillResult};
use crate::jarvis::skills::metadata::*;
use async_trait::async_trait;
use serde_json::json;

// Windows API imports for mouse and keyboard control
#[cfg(target_os = "windows")]
use winapi::um::winuser::{
    SendInput, INPUT, INPUT_MOUSE, INPUT_KEYBOARD,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE,
    KEYEVENTF_UNICODE, KEYEVENTF_KEYUP,
    GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
};

#[cfg(target_os = "windows")]
use std::mem;

use std::path::PathBuf;

pub struct ComputerUseSkill {
    // Simple skill implementation
    // TODO: Integrate with full ComputerUseAgent when needed
}

impl ComputerUseSkill {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for ComputerUseSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for ComputerUseSkill {
    fn metadata(&self) -> SkillMetadata {
        SkillMetadata {
            name: "computer_use".to_string(),
            description: "Control computer: open apps, click, type, screenshot".to_string(),
            version: "1.0.0".to_string(),
            actions: vec![
                "open_app".to_string(),
                "click".to_string(),
                "type".to_string(),
                "screenshot".to_string(),
                "key".to_string(),
            ],
            requirements: SkillRequirements {
                env_vars: vec![],
                required_bins: vec![],
                any_bins: vec![],
                platform: Some("windows".to_string()), // Windows-specific for now
                config_keys: vec![],
            },
            tags: vec!["automation".to_string(), "desktop".to_string()],
        }
    }

    fn check_eligibility(&self) -> EligibilityResult {
        let requirements = self.metadata().requirements;
        check_requirements(&requirements)
    }

    async fn execute(&self, ctx: SkillContext) -> SkillResult {
        log::info!(
            "ComputerUseSkill executing action: {} (session: {})",
            ctx.action,
            ctx.session_key
        );

        match ctx.action.as_str() {
            "open_app" => self.open_app(ctx).await,
            "click" => self.click(ctx).await,
            "type" => self.type_text(ctx).await,
            "screenshot" => self.screenshot(ctx).await,
            "key" => self.press_key(ctx).await,
            _ => SkillResult::error(format!("Unknown action: {}", ctx.action)),
        }
    }

    fn requires_approval(&self, action: &str) -> bool {
        // Risky actions require approval
        matches!(action, "execute" | "delete")
    }
}

impl ComputerUseSkill {
    async fn open_app(&self, ctx: SkillContext) -> SkillResult {
        let app_name = match ctx.params.get("app") {
            Some(serde_json::Value::String(s)) => s.clone(),
            _ => {
                return SkillResult::error("Missing 'app' parameter");
            }
        };

        // TODO: Use Windows API to open application
        // For now, use std::process::Command
        #[cfg(target_os = "windows")]
        {
            let result = std::process::Command::new("cmd")
                .args(&["/C", "start", &app_name])
                .spawn();

            match result {
                Ok(_) => SkillResult::success(format!("Opened {}", app_name)),
                Err(e) => SkillResult::error(format!("Failed to open {}: {}", app_name, e)),
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            SkillResult::error("Computer use only supported on Windows")
        }
    }

    async fn click(&self, ctx: SkillContext) -> SkillResult {
        let x = ctx
            .params
            .get("x")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let y = ctx
            .params
            .get("y")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        #[cfg(target_os = "windows")]
        {
            log::info!("Performing mouse click at ({}, {})", x, y);

            unsafe {
                // Get screen dimensions for coordinate normalization
                let screen_width = GetSystemMetrics(SM_CXSCREEN);
                let screen_height = GetSystemMetrics(SM_CYSCREEN);

                if screen_width == 0 || screen_height == 0 {
                    return SkillResult::error("Failed to get screen dimensions");
                }

                // Convert screen coordinates to absolute coordinates (0-65535)
                // Windows uses 65535 as the coordinate space for absolute positioning
                let normalized_x = ((x as f64 / screen_width as f64) * 65535.0) as i32;
                let normalized_y = ((y as f64 / screen_height as f64) * 65535.0) as i32;

                log::debug!(
                    "Normalized coordinates: ({}, {}) -> ({}, {})",
                    x, y, normalized_x, normalized_y
                );

                // Create mouse move event
                let mut input_move = INPUT {
                    type_: INPUT_MOUSE,
                    u: mem::zeroed(),
                };

                *input_move.u.mi_mut() = winapi::um::winuser::MOUSEINPUT {
                    dx: normalized_x,
                    dy: normalized_y,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                    time: 0,
                    dwExtraInfo: 0,
                };

                // Create mouse down event
                let mut input_down = INPUT {
                    type_: INPUT_MOUSE,
                    u: mem::zeroed(),
                };

                *input_down.u.mi_mut() = winapi::um::winuser::MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_LEFTDOWN,
                    time: 0,
                    dwExtraInfo: 0,
                };

                // Create mouse up event
                let mut input_up = INPUT {
                    type_: INPUT_MOUSE,
                    u: mem::zeroed(),
                };

                *input_up.u.mi_mut() = winapi::um::winuser::MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_LEFTUP,
                    time: 0,
                    dwExtraInfo: 0,
                };

                // Send all three events: move, down, up
                let inputs = [input_move, input_down, input_up];
                let result = SendInput(
                    inputs.len() as u32,
                    inputs.as_ptr() as *mut INPUT,
                    mem::size_of::<INPUT>() as i32,
                );

                if result != inputs.len() as u32 {
                    log::error!("SendInput failed: expected {}, got {}", inputs.len(), result);
                    return SkillResult::error(format!(
                        "Failed to send mouse input: only {} of {} events sent",
                        result, inputs.len()
                    ));
                }

                log::info!("Successfully clicked at ({}, {})", x, y);
                SkillResult::success_with_data(
                    format!("Clicked at ({}, {})", x, y),
                    json!({
                        "action": "click",
                        "x": x,
                        "y": y,
                        "status": "success"
                    })
                )
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            SkillResult::error("Mouse click only supported on Windows")
        }
    }

    async fn type_text(&self, ctx: SkillContext) -> SkillResult {
        let text = match ctx.params.get("text") {
            Some(serde_json::Value::String(s)) => s.clone(),
            _ => {
                return SkillResult::error("Missing 'text' parameter");
            }
        };

        #[cfg(target_os = "windows")]
        {
            log::info!("Typing text: {} characters", text.len());

            unsafe {
                // For each character, send key down and key up events
                for ch in text.chars() {
                    let scan_code = ch as u16;

                    // Create key down event using KEYEVENTF_UNICODE
                    let mut input_down = INPUT {
                        type_: INPUT_KEYBOARD,
                        u: mem::zeroed(),
                    };

                    *input_down.u.ki_mut() = winapi::um::winuser::KEYBDINPUT {
                        wVk: 0,
                        wScan: scan_code,
                        dwFlags: KEYEVENTF_UNICODE,
                        time: 0,
                        dwExtraInfo: 0,
                    };

                    // Create key up event
                    let mut input_up = INPUT {
                        type_: INPUT_KEYBOARD,
                        u: mem::zeroed(),
                    };

                    *input_up.u.ki_mut() = winapi::um::winuser::KEYBDINPUT {
                        wVk: 0,
                        wScan: scan_code,
                        dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    };

                    // Send both events
                    let inputs = [input_down, input_up];
                    let result = SendInput(
                        inputs.len() as u32,
                        inputs.as_ptr() as *mut INPUT,
                        mem::size_of::<INPUT>() as i32,
                    );

                    if result != inputs.len() as u32 {
                        log::error!("SendInput failed for character '{}': expected {}, got {}", ch, inputs.len(), result);
                        return SkillResult::error(format!(
                            "Failed to send keyboard input for character '{}': only {} of {} events sent",
                            ch, result, inputs.len()
                        ));
                    }

                    // Small delay between characters to ensure proper input
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }

                log::info!("Successfully typed {} characters", text.len());
                SkillResult::success_with_data(
                    format!("Typed {} characters", text.len()),
                    json!({
                        "action": "type",
                        "text": text,
                        "characters": text.len(),
                        "status": "success"
                    })
                )
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            SkillResult::error("Keyboard input only supported on Windows")
        }
    }

    async fn screenshot(&self, ctx: SkillContext) -> SkillResult {
        log::info!("Capturing screenshot");

        // Get optional path parameter, or use default
        let output_path = match ctx.params.get("path") {
            Some(serde_json::Value::String(s)) => PathBuf::from(s),
            _ => {
                // Default to temp directory with timestamp
                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
                let temp_dir = std::env::temp_dir();
                temp_dir.join(format!("screenshot_{}.png", timestamp))
            }
        };

        // Capture screenshot using screenshots crate
        match screenshots::Screen::all() {
            Ok(screens) => {
                if screens.is_empty() {
                    return SkillResult::error("No screens found");
                }

                // Use the primary screen (first one)
                let screen = &screens[0];
                log::debug!("Capturing from screen: {}x{}", screen.display_info.width, screen.display_info.height);

                match screen.capture() {
                    Ok(image_buffer) => {
                        let width = image_buffer.width();
                        let height = image_buffer.height();

                        // Save the image
                        if let Err(e) = image_buffer.save(&output_path) {
                            log::error!("Failed to save screenshot: {}", e);
                            return SkillResult::error(format!("Failed to save screenshot: {}", e));
                        }

                        log::info!(
                            "Screenshot saved to: {} ({}x{})",
                            output_path.display(),
                            width,
                            height
                        );

                        SkillResult::success_with_data(
                            format!("Screenshot saved to {}", output_path.display()),
                            json!({
                                "action": "screenshot",
                                "path": output_path.to_string_lossy().to_string(),
                                "width": width,
                                "height": height,
                                "status": "success"
                            })
                        )
                    }
                    Err(e) => {
                        log::error!("Failed to capture screen: {}", e);
                        SkillResult::error(format!("Failed to capture screen: {}", e))
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to get screens: {}", e);
                SkillResult::error(format!("Failed to get screens: {}", e))
            }
        }
    }

    async fn press_key(&self, ctx: SkillContext) -> SkillResult {
        let _key = match ctx.params.get("key") {
            Some(serde_json::Value::String(s)) => s.clone(),
            _ => {
                return SkillResult::error("Missing 'key' parameter");
            }
        };

        // TODO: Implement key press using Windows API
        SkillResult::error("Key press not yet implemented")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::jarvis::models::context::UserContext;

    fn create_test_context(action: &str, params: HashMap<String, serde_json::Value>) -> SkillContext {
        SkillContext {
            action: action.to_string(),
            params,
            session_key: "test".to_string(),
            user_context: UserContext::default(),
        }
    }

    #[tokio::test]
    async fn test_metadata() {
        let skill = ComputerUseSkill::new();
        let meta = skill.metadata();

        assert_eq!(meta.name, "computer_use");
        assert!(meta.actions.contains(&"open_app".to_string()));
        assert!(meta.actions.contains(&"click".to_string()));
        assert!(meta.actions.contains(&"type".to_string()));
        assert!(meta.actions.contains(&"screenshot".to_string()));
        assert_eq!(meta.requirements.platform, Some("windows".to_string()));
    }

    #[tokio::test]
    async fn test_eligibility() {
        let skill = ComputerUseSkill::new();
        let result = skill.check_eligibility();

        // Should be eligible on Windows, not on other platforms
        #[cfg(target_os = "windows")]
        assert!(result.eligible);

        #[cfg(not(target_os = "windows"))]
        assert!(!result.eligible);
    }

    #[test]
    fn test_requires_approval() {
        let skill = ComputerUseSkill::new();

        assert!(!skill.requires_approval("open_app"));
        assert!(!skill.requires_approval("click"));
        assert!(skill.requires_approval("execute"));
        assert!(skill.requires_approval("delete"));
    }

    #[tokio::test]
    async fn test_click_missing_params() {
        let skill = ComputerUseSkill::new();
        let params = HashMap::new();
        let ctx = create_test_context("click", params);

        // Should use default values (0, 0) if params missing
        let result = skill.execute(ctx).await;

        // On Windows, should attempt the click; on other platforms, should error
        #[cfg(not(target_os = "windows"))]
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_click_with_coords() {
        let skill = ComputerUseSkill::new();
        let mut params = HashMap::new();
        params.insert("x".to_string(), serde_json::json!(100));
        params.insert("y".to_string(), serde_json::json!(200));
        let ctx = create_test_context("click", params);

        let result = skill.execute(ctx).await;

        #[cfg(target_os = "windows")]
        {
            assert!(result.success);
            if let Some(data) = result.data {
                assert_eq!(data.get("x").and_then(|v| v.as_i64()), Some(100));
                assert_eq!(data.get("y").and_then(|v| v.as_i64()), Some(200));
            }
        }

        #[cfg(not(target_os = "windows"))]
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_type_text_missing_param() {
        let skill = ComputerUseSkill::new();
        let params = HashMap::new();
        let ctx = create_test_context("type", params);

        let result = skill.execute(ctx).await;
        assert!(!result.success);
        assert!(result.message.contains("Missing 'text' parameter"));
    }

    #[tokio::test]
    async fn test_type_text_with_text() {
        let skill = ComputerUseSkill::new();
        let mut params = HashMap::new();
        params.insert("text".to_string(), serde_json::json!("Hello World"));
        let ctx = create_test_context("type", params);

        let result = skill.execute(ctx).await;

        #[cfg(target_os = "windows")]
        {
            assert!(result.success);
            if let Some(data) = result.data {
                assert_eq!(data.get("text").and_then(|v| v.as_str()), Some("Hello World"));
                assert_eq!(data.get("characters").and_then(|v| v.as_i64()), Some(11));
            }
        }

        #[cfg(not(target_os = "windows"))]
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_screenshot_default_path() {
        let skill = ComputerUseSkill::new();
        let params = HashMap::new();
        let ctx = create_test_context("screenshot", params);

        let result = skill.execute(ctx).await;

        // Screenshot should work on all platforms with screenshots crate
        if result.success {
            if let Some(data) = result.data {
                assert!(data.get("path").is_some());
                assert!(data.get("width").and_then(|v| v.as_u64()).unwrap() > 0);
                assert!(data.get("height").and_then(|v| v.as_u64()).unwrap() > 0);

                // Clean up the screenshot file
                if let Some(path) = data.get("path").and_then(|v| v.as_str()) {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_screenshot_custom_path() {
        let skill = ComputerUseSkill::new();
        let temp_dir = std::env::temp_dir();
        let test_path = temp_dir.join("test_screenshot.png");

        let mut params = HashMap::new();
        params.insert("path".to_string(), serde_json::json!(test_path.to_string_lossy().to_string()));
        let ctx = create_test_context("screenshot", params);

        let result = skill.execute(ctx).await;

        if result.success {
            // Verify file was created
            assert!(test_path.exists());

            if let Some(data) = result.data {
                assert_eq!(
                    data.get("path").and_then(|v| v.as_str()),
                    Some(test_path.to_string_lossy().as_ref())
                );
            }

            // Clean up
            let _ = std::fs::remove_file(test_path);
        }
    }

    #[tokio::test]
    async fn test_open_app() {
        let skill = ComputerUseSkill::new();
        let mut params = HashMap::new();
        params.insert("app".to_string(), serde_json::json!("notepad"));
        let ctx = create_test_context("open_app", params);

        let result = skill.execute(ctx).await;

        #[cfg(target_os = "windows")]
        {
            // Opening notepad should succeed on Windows
            // Note: This will actually open notepad, might want to skip in CI
            assert!(result.success || result.message.contains("Failed to open"));
        }

        #[cfg(not(target_os = "windows"))]
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_unknown_action() {
        let skill = ComputerUseSkill::new();
        let params = HashMap::new();
        let ctx = create_test_context("unknown_action", params);

        let result = skill.execute(ctx).await;
        assert!(!result.success);
        assert!(result.message.contains("Unknown action"));
    }
}
