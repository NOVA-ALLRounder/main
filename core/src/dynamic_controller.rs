use crate::llm_gateway::LLMClient;
use crate::visual_driver::{VisualDriver, UiAction, SmartStep};
use anyhow::Result;
use serde_json::Value; 
use crate::db; 
use log::{info, warn, error};

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
        info!("🌊 Starting Dynamic Surf: '{}'", goal);
        let mut history: Vec<String> = Vec::new();
        let mut session_steps: Vec<SmartStep> = Vec::new(); // Track for macro recording
        let mut consecutive_failures = 0;

        for i in 1..=self.max_steps {
            info!("\n🔄 [Step {}/{}] Observing...", i, self.max_steps);
            
            // 1. Capture Screen
            let (image_b64, _scale) = VisualDriver::capture_screen()?;
            
            // 2. Plan (Think)
            info!("   🧠 Thinking...");
            let plan = self.llm.plan_vision_step(goal, &image_b64, &history).await?;
            
            info!("   💡 Idea: {}", plan);

            // 3. Act
            let action_type = plan["action"].as_str().unwrap_or("fail");
            let mut driver = VisualDriver::new();
            let mut description = format!("Step {}", i);
            let mut step_to_record: Option<SmartStep> = None; // Step to save (if Action)

            match action_type {
                "click_visual" => {
                    let desc = plan["description"].as_str().unwrap_or("element");
                    let step = SmartStep::new(UiAction::ClickVisual(desc.to_string()), desc);
                    driver.add_step(step.clone());
                    step_to_record = Some(step);
                    description = format!("Clicked '{}'", desc);
                },
                "type" => {
                    let text = plan["text"].as_str().unwrap_or("");
                    let step = SmartStep::new(UiAction::Type(text.to_string()), "Typing");
                    driver.add_step(step.clone());
                    step_to_record = Some(step);
                    description = format!("Typed '{}'", text);
                },
                "key" => {
                    let key = plan["key"].as_str().unwrap_or("return");
                    let key_char = match key.to_lowercase().as_str() {
                        "return" | "enter" => "\r",
                        "tab" => "\t",
                        _ => key
                    };
                    let step = SmartStep::new(UiAction::Type(key_char.to_string()), "Pressing Key");
                    driver.add_step(step.clone());
                    step_to_record = Some(step);
                    description = format!("Pressed '{}'", key);
                },
                "scroll" => {
                    let dir = plan["direction"].as_str().unwrap_or("down");
                    let step = SmartStep::new(UiAction::Scroll(dir.to_string()), "Scrolling");
                    driver.add_step(step.clone());
                    step_to_record = Some(step);
                    description = format!("Scrolled {}", dir);
                },
                "read" => {
                    let query = plan["query"].as_str().unwrap_or("Describe the screen");
                    info!("      📖 Reading: '{}'", query);
                    match self.llm.analyze_screen(query, &image_b64).await {
                        Ok(text) => {
                            info!("      📝 Extracted: {}", text);
                            description = format!("Read Info: '{}' -> '{}'", query, text);
                        },
                        Err(e) => {
                             error!("      ❌ Read failed: {}", e);
                             description = format!("Failed to read: {}", e);
                        }
                    }
                },
                "save_routine" => {
                    let name = plan["name"].as_str().unwrap_or("unnamed");
                    info!("      💾 Saving Routine as '{}' ({} steps)...", name, session_steps.len());
                    match serde_json::to_string(&session_steps) {
                        Ok(json) => {
                            if let Err(e) = db::save_learned_routine(name, &json) {
                                error!("      ❌ Save failed: {}", e);
                                description = format!("Failed to save routine: {}", e);
                            } else {
                                info!("      ✅ Routine Saved!");
                                description = format!("Saved Routine '{}'", name);
                            }
                        },
                        Err(e) => {
                            error!("      ❌ Serialization failed: {}", e);
                             description = format!("Failed to serialize routine: {}", e);
                        }
                    }
                },
                "replay_routine" => {
                    let name = plan["name"].as_str().unwrap_or("unnamed");
                    info!("      ⏪ Replaying Routine '{}'...", name);
                    match db::get_learned_routine(name) {
                        Ok(Some(routine)) => {
                            match serde_json::from_str::<Vec<SmartStep>>(&routine.steps_json) {
                                Ok(steps) => {
                                    info!("      ▶️ Loaded {} steps. Executing...", steps.len());
                                    // Construct a driver with all steps and run it
                                    let mut replay_driver = VisualDriver::new();
                                    for s in steps {
                                        replay_driver.add_step(s);
                                    }
                                    if let Err(e) = replay_driver.execute(Some(&self.llm)).await {
                                        error!("      ❌ Replay failed: {}", e);
                                        description = format!("Replay '{}' failed: {}", name, e);
                                    } else {
                                        description = format!("Replayed Routine '{}'", name);
                                    }
                                },
                                Err(e) => {
                                     error!("      ❌ Corrupt routine data: {}", e);
                                     description = format!("Corrupt routine data: {}", e);
                                }
                            }
                        },
                        Ok(None) => {
                             error!("      ❌ Routine '{}' not found.", name);
                             description = format!("Routine '{}' not found", name);
                        },
                        Err(e) => {
                             error!("      ❌ DB Error: {}", e);
                             description = format!("DB Error: {}", e);
                        }
                    }
                },
                "wait" => {
                    let secs = plan["seconds"].as_u64().unwrap_or(2);
                    let step = SmartStep::new(UiAction::Wait(secs), "Waiting");
                    driver.add_step(step.clone());
                    step_to_record = Some(step);
                    description = format!("Waited {}s", secs);
                },
                "done" => {
                    info!("✅ Goal Achieved!");
                    return Ok(());
                },
                "reply" => {
                    let text = plan["text"].as_str().unwrap_or("...");
                    info!("🤖 Agent: {}", text);
                    return Ok(()); // Chat is a single-turn action
                },
                "fail" => {
                    let reason = plan["reason"].as_str().unwrap_or("Unknown");
                    error!("❌ Agent gave up: {}", reason);
                    return Err(anyhow::anyhow!("Agent failed: {}", reason));
                },
                "open_app" => {
                    let name = plan["name"].as_str().unwrap_or("Finder");
                    info!("      🚀 Launching/Focusing App: '{}'", name);
                    match std::process::Command::new("open").arg("-a").arg(name).output() {
                        Ok(_) => {
                            let step = SmartStep::new(UiAction::Type(name.to_string()), "Open App");
                            session_steps.push(step);
                            description = format!("Opened '{}'", name);
                            // Wait for app to appear
                            tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                        },
                        Err(e) => {
                             error!("      ❌ Failed to open app: {}", e);
                             description = format!("Failed to open '{}': {}", name, e);
                        }
                    }
                },
                "open_url" => {
                    let url = plan["url"].as_str().unwrap_or("https://google.com");
                    info!("      🌐 Opening URL: '{}'", url);
                    // Use 'open' command which uses default browser
                    match std::process::Command::new("open").arg(url).output() {
                        Ok(_) => {
                            let step = SmartStep::new(UiAction::Type(url.to_string()), "Open URL");
                            session_steps.push(step);
                            description = format!("Opened URL '{}'", url);
                            tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                        },
                        Err(e) => {
                             error!("      ❌ Failed to open URL: {}", e);
                             description = format!("Failed to open URL: {}", e);
                        }
                    }
                },
                _ => {
                    warn!("⚠️ Unknown action: {}", action_type);
                }
            }

            // Execute the single step (Unless it was pure cognitive like read/save/replay which have handled themselves)
            // Replay handles its own execution. Read/Save are instant.
            // Only Execute if we added steps to `driver` (Action steps)
            if !driver.steps.is_empty() {
                if let Err(e) = driver.execute(Some(&self.llm)).await {
                    warn!("   ⚠️ Action failed: {}", e);
                    history.push(format!("FAILED: {}", description));
                    consecutive_failures += 1;
                    // Don't record failed steps
                } else {
                    history.push(description);
                    // Record success step
                    if let Some(s) = step_to_record {
                        session_steps.push(s);
                    }
                    consecutive_failures = 0; // Reset on success
                }
            } else {
                // For non-driver actions, just push history
                 history.push(description);
                 consecutive_failures = 0; // Reset
            }
            
            // COMBAT MODE CHECK
            if consecutive_failures >= 2 {
                println!("      ⚔️ CRITICAL: Consecutive failures detected ({})", consecutive_failures);
                println!("      🛡️ ENGAGING COMBAT PROTOCOL: Attempting to clear obstacles...");
                
                // Strategy 1: The "Escape" Hatch (Close Modals)
                println!("         👉 Strategy 1: Pressing ESC");
                let esc_script = "tell application \"System Events\" to key code 53"; // 53 is Esc
                let _ = std::process::Command::new("osascript").arg("-e").arg(esc_script).status();
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                // Strategy 2: The "Enter" Ram (Confirm Dialogs)
                println!("         👉 Strategy 2: Pressing ENTER");
                let enter_script = "tell application \"System Events\" to keystroke return";
                let _ = std::process::Command::new("osascript").arg("-e").arg(enter_script).status();
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                
                // Logic: After attempting blind fixes, we hope the next iteration sees a clear screen.
                // We reset the counter to give it another chance before spamming keys again.
                // But we keep it high enough next time? No, let's reset to 1.
                consecutive_failures = 1; 
                history.push("Combat Protocol executed: Esc + Enter".to_string());
            }
            
            // Adaptive Wait (Hyper-Speed)
            println!("      👁️ Monitoring UI stability...");
            if let Err(e) = VisualDriver::wait_for_ui_settle(3000).await {
                println!("      ⚠️ Adaptive wait error: {}", e);
            }
            // Fallback safety sleep (short)
            // tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        println!("🛑 Max steps reached.");
        Ok(())
    }
}
