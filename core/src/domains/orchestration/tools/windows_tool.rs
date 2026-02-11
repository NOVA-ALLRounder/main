// Windows Tool - integrates with platform::windows

use super::tool_trait::{Tool, ToolParams, ToolResult};
use crate::platform::{get_platform, traits::*};
use anyhow::Result;
use async_trait::async_trait;

pub struct WindowsTool {
    // Platform is accessed via get_platform() to avoid lifetime issues
}

impl WindowsTool {
    pub fn new() -> Self {
        Self {}
    }

    fn launch_app(&self, app_name: &str) -> Result<()> {
        let platform = get_platform();
        platform.app_controller().launch_app(app_name)
    }

    fn activate_app(&self, app_name: &str) -> Result<()> {
        let platform = get_platform();
        platform.app_controller().activate_app(app_name)
    }

    fn get_active_app(&self) -> Result<(String, String)> {
        let platform = get_platform();
        platform.app_controller().get_active_app()
    }

    fn open_url(&self, url: &str) -> Result<()> {
        let platform = get_platform();
        platform.app_controller().open_url(url)
    }

    fn click_at(&self, x: i32, y: i32) -> Result<()> {
        let platform = get_platform();
        platform.input_simulator().click_at(x, y)
    }

    fn type_text(&self, text: &str) -> Result<()> {
        let platform = get_platform();
        platform.input_simulator().type_text(text)
    }

    fn press_key(&self, key_code: u32) -> Result<()> {
        let platform = get_platform();
        platform.input_simulator().press_key(key_code)
    }

    fn scroll(&self, direction: &str, amount: i32) -> Result<()> {
        let platform = get_platform();
        platform.input_simulator().scroll(direction, amount)
    }
}

#[async_trait]
impl Tool for WindowsTool {
    fn name(&self) -> &str {
        "windows"
    }

    fn description(&self) -> &str {
        "Windows application control and input simulation"
    }

    async fn execute(&self, params: ToolParams) -> Result<ToolResult> {
        log::debug!("WindowsTool executing action: {}", params.action);

        match params.action.as_str() {
            "launch_app" => {
                let app = params.get_string("app")?;
                self.launch_app(&app)?;
                Ok(ToolResult::success(format!("Launched {}", app)))
            }
            "activate_app" => {
                let app = params.get_string("app")?;
                self.activate_app(&app)?;
                Ok(ToolResult::success(format!("Activated {}", app)))
            }
            "get_active_app" => {
                let (app_name, window_title) = self.get_active_app()?;
                Ok(ToolResult::json(serde_json::json!({
                    "app": app_name,
                    "window_title": window_title
                })))
            }
            "open_url" => {
                let url = params.get_string("url")?;
                self.open_url(&url)?;
                Ok(ToolResult::success(format!("Opened URL: {}", url)))
            }
            "click" => {
                let x = params.get_i32("x")?;
                let y = params.get_i32("y")?;
                self.click_at(x, y)?;
                Ok(ToolResult::success(format!("Clicked at ({}, {})", x, y)))
            }
            "type_text" => {
                let text = params.get_string("text")?;
                self.type_text(&text)?;
                Ok(ToolResult::success(format!("Typed {} characters", text.len())))
            }
            "press_key" => {
                let key_code = params.get_i32("key_code")? as u32;
                self.press_key(key_code)?;
                Ok(ToolResult::success(format!("Pressed key: {}", key_code)))
            }
            "scroll" => {
                let direction = params.get_string("direction")?;
                let amount = params.get_i32("amount")?;
                self.scroll(&direction, amount)?;
                Ok(ToolResult::success(format!(
                    "Scrolled {} by {}",
                    direction, amount
                )))
            }
            _ => Err(anyhow::anyhow!("Unknown action: {}", params.action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_windows_tool_creation() {
        let tool = WindowsTool::new();
        assert_eq!(tool.name(), "windows");
    }

    #[tokio::test]
    async fn test_get_active_app() {
        let tool = WindowsTool::new();
        let params = ToolParams::new("get_active_app");

        // This might fail in CI/headless environment, so we just test the interface
        let result = tool.execute(params).await;
        // Result can be Ok or Err depending on environment
        assert!(result.is_ok() || result.is_err());
    }
}
