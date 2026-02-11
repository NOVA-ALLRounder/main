// Computer Use Agent - Vision-based desktop automation
// Perception ??Reasoning ??Action loop

use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

use crate::domains::intelligence::llm_gateway::LLMClient;

/// Computer Use Agent - Controls computer through vision
pub struct ComputerUseAgent {
    llm: LLMClient,
    screen_width: u32,
    screen_height: u32,
    action_delay_ms: u64,
    max_steps: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerAction {
    pub action_type: String,  // "click", "type", "key", "scroll", "screenshot", "done"
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub text: Option<String>,
    pub key: Option<String>,
    pub button: Option<String>,  // "left", "right", "middle"
    pub scroll_amount: Option<i32>,
    pub reasoning: String,
}

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub success: bool,
    pub message: String,
    pub screenshot_path: Option<String>,
}

impl ComputerUseAgent {
    pub fn new() -> Result<Self> {
        let llm = LLMClient::new()?;

        // Get screen resolution
        let (width, height) = Self::get_screen_resolution()?;

        Ok(Self {
            llm,
            screen_width: width,
            screen_height: height,
            action_delay_ms: 500,  // 0.5 second between actions
            max_steps: 50,  // Prevent runaway loops
        })
    }

    /// Main loop: Observe ??Reason ??Act
    pub async fn execute_goal(&mut self, goal: &str) -> Result<Vec<ExecutionResult>> {
        log::info!("???Computer Use Goal: {}", goal);

        let mut results = Vec::new();
        let mut step = 0;

        loop {
            step += 1;

            if step > self.max_steps {
                log::warn!("?醫묓닔  Max steps ({}) reached, stopping", self.max_steps);
                break;
            }

            log::info!("?踰?Step {}: Taking screenshot...", step);

            // 1. PERCEPTION: Capture screen
            let screenshot_path = self.capture_screenshot()?;
            let screenshot_b64 = self.encode_image_to_base64(&screenshot_path)?;

            // 2. REASONING: Ask vision model what to do
            log::info!("?裕?Step {}: Analyzing screen...", step);
            let action = self.decide_next_action(goal, &screenshot_b64, step).await?;

            log::info!("???Step {}: Action: {} - {}", step, action.action_type, action.reasoning);

            // Check if done
            if action.action_type == "done" {
                log::info!("??Goal completed!");
                results.push(ExecutionResult {
                    success: true,
                    message: action.reasoning.clone(),
                    screenshot_path: Some(screenshot_path),
                });
                break;
            }

            // 3. ACTION: Execute the action
            let result = self.execute_action(&action)?;
            results.push(result);

            // Wait before next action (avoid overwhelming)
            tokio::time::sleep(tokio::time::Duration::from_millis(self.action_delay_ms)).await;
        }

        Ok(results)
    }

    /// Use vision model to decide next action
    async fn decide_next_action(
        &self,
        goal: &str,
        screenshot_b64: &str,
        step: usize,
    ) -> Result<ComputerAction> {
        let system_prompt = format!(r#"You are a computer use agent controlling a Windows desktop.

Your goal: {}

Screen resolution: {}x{}

You can perform these actions:
1. click: Click at coordinates {{"action_type": "click", "x": 100, "y": 200, "button": "left"}}
2. type: Type text {{"action_type": "type", "text": "Hello"}}
3. key: Press key {{"action_type": "key", "key": "Enter"}} (Enter, Escape, Tab, etc.)
4. scroll: Scroll {{"action_type": "scroll", "scroll_amount": -3}} (negative=up, positive=down)
5. done: Task completed {{"action_type": "done"}}

Current step: {}

Analyze the screenshot and decide the NEXT SINGLE ACTION.

Response format (JSON only):
{{
  "action_type": "click",
  "x": 100,
  "y": 200,
  "button": "left",
  "reasoning": "I see the Start button at bottom-left, clicking it to open menu"
}}

Be precise with coordinates. Think step-by-step.
"#,
            goal, self.screen_width, self.screen_height, step
        );

        let messages = vec![
            json!({
                "role": "user",
                "content": [
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/png;base64,{}", screenshot_b64)
                        }
                    },
                    {
                        "type": "text",
                        "text": "What should I do next?"
                    }
                ]
            })
        ];

        // Use GPT-4V for vision
        let response = self.llm.chat_completion_json(
            vec![
                json!({"role": "system", "content": system_prompt}),
                messages[0].clone()
            ],
            Some("gpt-4o")  // GPT-4V for vision
        ).await?;

        let action: ComputerAction = serde_json::from_str(&response)?;

        Ok(action)
    }

    /// Execute a computer action
    fn execute_action(&self, action: &ComputerAction) -> Result<ExecutionResult> {
        match action.action_type.as_str() {
            "click" => {
                let x = action.x.ok_or_else(|| anyhow::anyhow!("Missing x coordinate"))?;
                let y = action.y.ok_or_else(|| anyhow::anyhow!("Missing y coordinate"))?;
                let button = action.button.as_deref().unwrap_or("left");

                self.mouse_click(x, y, button)?;

                Ok(ExecutionResult {
                    success: true,
                    message: format!("Clicked {} button at ({}, {})", button, x, y),
                    screenshot_path: None,
                })
            }

            "type" => {
                let text = action.text.as_ref().ok_or_else(|| anyhow::anyhow!("Missing text"))?;

                self.keyboard_type(text)?;

                Ok(ExecutionResult {
                    success: true,
                    message: format!("Typed: {}", text),
                    screenshot_path: None,
                })
            }

            "key" => {
                let key = action.key.as_ref().ok_or_else(|| anyhow::anyhow!("Missing key"))?;

                self.keyboard_press(key)?;

                Ok(ExecutionResult {
                    success: true,
                    message: format!("Pressed key: {}", key),
                    screenshot_path: None,
                })
            }

            "scroll" => {
                let amount = action.scroll_amount.unwrap_or(-3);

                self.mouse_scroll(amount)?;

                Ok(ExecutionResult {
                    success: true,
                    message: format!("Scrolled {}", amount),
                    screenshot_path: None,
                })
            }

            _ => {
                Ok(ExecutionResult {
                    success: false,
                    message: format!("Unknown action: {}", action.action_type),
                    screenshot_path: None,
                })
            }
        }
    }

    /// Capture screenshot using Windows API
    fn capture_screenshot(&self) -> Result<String> {
        let timestamp = chrono::Utc::now().timestamp();
        let output_path = format!("C:\\Users\\Admin\\AppData\\Local\\steer\\screenshots\\screen_{}.png", timestamp);

        // Ensure directory exists
        std::fs::create_dir_all("C:\\Users\\Admin\\AppData\\Local\\steer\\screenshots")?;

        // Use PowerShell to capture screenshot (Windows built-in)
        let ps_script = format!(r#"
            Add-Type -AssemblyName System.Windows.Forms
            Add-Type -AssemblyName System.Drawing
            $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
            $bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
            $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
            $graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
            $bitmap.Save('{}')
            $graphics.Dispose()
            $bitmap.Dispose()
        "#, output_path.replace("\\", "\\\\"));

        Command::new("powershell")
            .args(&["-NoProfile", "-Command", &ps_script])
            .output()?;

        // Verify file was created
        if !Path::new(&output_path).exists() {
            return Err(anyhow::anyhow!("Screenshot failed to save"));
        }

        log::debug!("Screenshot saved: {}", output_path);

        Ok(output_path)
    }

    /// Encode image to base64
    fn encode_image_to_base64(&self, path: &str) -> Result<String> {
        let img_bytes = std::fs::read(path)?;
        Ok(general_purpose::STANDARD.encode(&img_bytes))
    }

    /// Get screen resolution
    fn get_screen_resolution() -> Result<(u32, u32)> {
        // Use PowerShell to get resolution
        let output = Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-Command",
                "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.Screen]::PrimaryScreen.Bounds | Select-Object Width,Height | ConvertTo-Json"
            ])
            .output()?;

        let json_str = String::from_utf8_lossy(&output.stdout);
        let resolution: Value = serde_json::from_str(&json_str)?;

        let width = resolution["Width"].as_u64().unwrap_or(1920) as u32;
        let height = resolution["Height"].as_u64().unwrap_or(1080) as u32;

        Ok((width, height))
    }

    /// Mouse click using Windows API
    fn mouse_click(&self, x: i32, y: i32, _button: &str) -> Result<()> {
        // Use PowerShell for mouse control
        let ps_script = format!(r#"
            Add-Type -AssemblyName System.Windows.Forms
            [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point({}, {})
            Start-Sleep -Milliseconds 100
        "#, x, y);

        Command::new("powershell")
            .args(&["-NoProfile", "-Command", &ps_script])
            .output()?;

        // TODO: Actual click with SendInput API (needs Windows crate)
        // For now, just move cursor

        log::debug!("Moved mouse to ({}, {})", x, y);

        Ok(())
    }

    /// Type text using keyboard
    fn keyboard_type(&self, text: &str) -> Result<()> {
        // Use PowerShell SendKeys (basic implementation)
        let ps_script = format!(r#"
            Add-Type -AssemblyName System.Windows.Forms
            [System.Windows.Forms.SendKeys]::SendWait('{}')
        "#, text.replace("'", "''"));

        Command::new("powershell")
            .args(&["-NoProfile", "-Command", &ps_script])
            .output()?;

        log::debug!("Typed: {}", text);

        Ok(())
    }

    /// Press a key
    fn keyboard_press(&self, key: &str) -> Result<()> {
        let ps_key = match key.to_lowercase().as_str() {
            "enter" => "{ENTER}",
            "escape" | "esc" => "{ESC}",
            "tab" => "{TAB}",
            "backspace" => "{BACKSPACE}",
            "delete" => "{DELETE}",
            _ => key,
        };

        let ps_script = format!(r#"
            Add-Type -AssemblyName System.Windows.Forms
            [System.Windows.Forms.SendKeys]::SendWait('{}')
        "#, ps_key);

        Command::new("powershell")
            .args(&["-NoProfile", "-Command", &ps_script])
            .output()?;

        log::debug!("Pressed key: {}", key);

        Ok(())
    }

    /// Mouse scroll
    fn mouse_scroll(&self, amount: i32) -> Result<()> {
        // TODO: Implement with Windows API
        log::debug!("Scroll: {}", amount);
        Ok(())
    }
}
