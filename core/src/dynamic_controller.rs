use crate::llm_gateway::LLMClient;
use crate::visual_driver::{VisualDriver, UiAction, SmartStep};
use anyhow::Result;
use serde_json::Value;

pub struct DynamicController {
    llm: LLMClient,
    max_steps: usize,
}

impl DynamicController {
    pub fn new(llm: LLMClient) -> Self {
        Self {
            llm,
            max_steps: 15,
        }
    }

    pub async fn surf(&self, goal: &str) -> Result<()> {
        println!("🌊 Starting Dynamic Surf: '{}'", goal);
        let mut history: Vec<String> = Vec::new();

        for i in 1..=self.max_steps {
            println!("\n🔄 [Step {}/{}] Observing...", i, self.max_steps);
            
            // 1. Capture Screen
            let (image_b64, _scale) = VisualDriver::capture_screen()?;
            
            // 2. Plan (Think)
            println!("   🧠 Thinking...");
            let plan = self.llm.plan_vision_step(goal, &image_b64, &history).await?;
            
            println!("   💡 Idea: {}", plan);

            // 3. Act
            let action_type = plan["action"].as_str().unwrap_or("fail");
            let mut driver = VisualDriver::new();
            let mut description = format!("Step {}", i);

            match action_type {
                "click_visual" => {
                    let desc = plan["description"].as_str().unwrap_or("element");
                    driver.add_step(SmartStep::new(UiAction::ClickVisual(desc.to_string()), desc));
                    description = format!("Clicked '{}'", desc);
                },
                "type" => {
                    let text = plan["text"].as_str().unwrap_or("");
                    driver.add_step(SmartStep::new(UiAction::Type(text.to_string()), "Typing"));
                    description = format!("Typed '{}'", text);
                },
                "key" => {
                    let key = plan["key"].as_str().unwrap_or("return");
                    // Assuming Type actions handles generic keystroke via applescript for now or add explicit Key support later
                    // Using Type hack for keys: "\n" for return usually works in applescript keystroke
                    let key_char = match key.to_lowercase().as_str() {
                        "return" | "enter" => "\r",
                        "tab" => "\t",
                        _ => key
                    };
                    driver.add_step(SmartStep::new(UiAction::Type(key_char.to_string()), "Pressing Key"));
                    description = format!("Pressed '{}'", key);
                },
                "scroll" => {
                    let dir = plan["direction"].as_str().unwrap_or("down");
                    driver.add_step(SmartStep::new(UiAction::Scroll(dir.to_string()), "Scrolling"));
                    description = format!("Scrolled {}", dir);
                },
                "wait" => {
                    let secs = plan["seconds"].as_u64().unwrap_or(2);
                    driver.add_step(SmartStep::new(UiAction::Wait(secs), "Waiting"));
                    description = format!("Waited {}s", secs);
                },
                "done" => {
                    println!("✅ Goal Achieved!");
                    return Ok(());
                },
                "fail" => {
                    let reason = plan["reason"].as_str().unwrap_or("Unknown");
                    println!("❌ Agent gave up: {}", reason);
                    return Err(anyhow::anyhow!("Agent failed: {}", reason));
                },
                _ => {
                    println!("⚠️ Unknown action: {}", action_type);
                }
            }

            // Execute the single step
            if let Err(e) = driver.execute(Some(&self.llm)).await {
                println!("   ⚠️ Action failed: {}", e);
                history.push(format!("FAILED: {}", description));
            } else {
                history.push(description);
            }
            
            // Short pause to let UI settle
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }

        println!("🛑 Max steps reached.");
        Ok(())
    }
}
