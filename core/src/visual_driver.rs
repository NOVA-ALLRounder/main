use crate::executor;
use crate::applescript;
use std::thread;
use std::time::Duration;
use anyhow::{Context, Result};
use std::process::Command;
use std::fs;
use base64::{Engine as _, engine::general_purpose};

#[derive(Debug, Clone)]
pub enum UiAction {
    OpenUrl(String),
    Wait(u64), // Seconds
    Click(String), // Element description or AppleScript target
    Type(String),
    Scroll(String), // "down" | "up"
    ActivateApp(String), // "frontmost" or app name
    // Verify(String), // Removed: Legacy standalone verify unused
}

#[derive(Debug, Clone)]
pub struct SmartStep {
    pub action: UiAction,
    pub description: String,
    pub pre_verify: Option<String>,  // Prompt for checking BEFORE action
    pub post_verify: Option<String>, // Prompt for checking AFTER action
    pub critical: bool,              // Stop on failure?
}

impl SmartStep {
    pub fn new(action: UiAction, desc: &str) -> Self {
        Self {
            action,
            description: desc.to_string(),
            pre_verify: None,
            post_verify: None,
            critical: true,
        }
    }

    pub fn with_pre_check(mut self, prompt: &str) -> Self {
        self.pre_verify = Some(prompt.to_string());
        self
    }

    pub fn with_post_check(mut self, prompt: &str) -> Self {
        self.post_verify = Some(prompt.to_string());
        self
    }
}

pub struct VisualDriver {
    steps: Vec<SmartStep>,
}

impl VisualDriver {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Capture the entire primary screen and return Base64 encoded JPEG
    pub fn capture_screen() -> Result<String> {
        let uuid = uuid::Uuid::new_v4();
        let output_path = format!("/tmp/steer_vision_{}.jpg", uuid);
        
        let status = Command::new("screencapture")
            .arg("-x")
            .arg("-t")
            .arg("jpg")
            .arg("-C") 
            .arg(&output_path)
            .status()
            .context("Failed to run screencapture command")?;

        if !status.success() {
            return Err(anyhow::anyhow!("screencapture returned non-zero exit code"));
        }

        let image_data = fs::read(&output_path)
            .context("Failed to read captured image file")?;
            
        // Cleanup immediately
        let _ = fs::remove_file(&output_path);
            
        let b64 = general_purpose::STANDARD.encode(&image_data);
        Ok(b64)
    }

    pub fn add_step(&mut self, step: SmartStep) -> &mut Self {
        self.steps.push(step);
        self
    }

    // Helper for legacy support
    pub fn add_legacy_step(&mut self, action: UiAction) -> &mut Self {
        self.steps.push(SmartStep::new(action, "Legacy Step"));
        self
    }

    async fn verify_condition(llm: &crate::llm_gateway::LLMClient, prompt: &str) -> Result<bool> {
        println!("      👁️ Vision Check: '{}'", prompt);
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await; // Brief pause before capture
        
        match Self::capture_screen() {
            Ok(b64) => {
                let full_prompt = format!(
                    "Screen Verification Task.\nCondition to verify: '{}'.\nReply ONLY with 'YES' or 'NO'.",
                    prompt
                );
                match llm.analyze_screen(&full_prompt, &b64).await {
                    Ok(resp) => {
                        let success = resp.trim().to_uppercase().starts_with("YES");
                        println!("      🤖 Result: {}", if success { "PASS" } else { "FAIL" });
                        Ok(success)
                    },
                    Err(e) => {
                        println!("      ⚠️ Vision API Error: {}", e);
                        Ok(false) // Conservative failure
                    }
                }
            },
            Err(e) => {
                println!("      ⚠️ Capture Failed: {}", e);
                Ok(false)
            }
        }
    }

    pub async fn execute(&self, llm: Option<&crate::llm_gateway::LLMClient>) -> Result<()> {
        println!("👻 [Smart Visual Driver] Starting Verified Automation...");
        
        for (i, step) in self.steps.iter().enumerate() {
            println!("   Step {}: {}", i + 1, step.description);
            
            // 1. Pre-Verification
            if let Some(pre_prompt) = &step.pre_verify {
                if let Some(brain) = llm {
                    if !Self::verify_condition(brain, pre_prompt).await? {
                         if step.critical {
                             return Err(anyhow::anyhow!("❌ Pre-check failed: {}", pre_prompt));
                         } else {
                             println!("      ⚠️ Pre-check failed, but proceeding (non-critical).");
                         }
                    }
                }
            }

            // 2. Action Execution
            match &step.action {
                UiAction::OpenUrl(url) => {
                    executor::open_url(url)?;
                }
                UiAction::Wait(secs) => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(*secs)).await;
                }
                UiAction::Click(target) => {
                    // Use frontmost application instead of hardcoded Safari
                    let target_clone = target.clone();
                    let script = format!(
                        "tell application \"System Events\" to click button {:?} of window 1 of (first application process whose frontmost is true)",
                        target_clone
                    );
                    
                    // [Survival] Run blocking script with timeout
                    let task = tokio::task::spawn_blocking(move || {
                        applescript::run(&script)
                    });

                    match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                        Ok(Ok(Ok(_))) => {}, // Success
                        Ok(Ok(Err(e))) => {
                            println!("      (Click failed: {})", e);
                            if step.critical { return Err(anyhow::anyhow!("Critical Click Failed: {}", e)); }
                        }
                        Ok(Err(_)) => { // JoinError
                             return Err(anyhow::anyhow!("Task Panic"));
                        }
                        Err(_) => { // Timeout
                             println!("      (Click timed out)");
                             if step.critical { return Err(anyhow::anyhow!("Critical Click Timed Out")); }
                        }
                    }
                }
                UiAction::Type(text) => {
                    let text_clone = text.clone();
                    let script = format!("tell application \"System Events\" to keystroke {:?}", text_clone);
                    
                    // [Survival] Run blocking script with timeout
                    let task = tokio::task::spawn_blocking(move || {
                        applescript::run(&script)
                    });
                    
                    match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                        Ok(Ok(Ok(_))) => {},
                        Ok(Ok(Err(e))) => return Err(anyhow::anyhow!("Type Failed: {}", e)),
                        Ok(Err(_)) => return Err(anyhow::anyhow!("Task Panic")),
                        Err(_) => return Err(anyhow::anyhow!("Type Timed Out")),
                    }
                }
                UiAction::Scroll(direction) => {
                    let dir = direction.to_lowercase();
                    let key_code = if dir == "up" { 116 } else { 121 }; // page up/down
                    let script = format!("tell application \"System Events\" to key code {}", key_code);
                    let task = tokio::task::spawn_blocking(move || {
                        applescript::run(&script)
                    });
                    match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                        Ok(Ok(Ok(_))) => {},
                        Ok(Ok(Err(e))) => return Err(anyhow::anyhow!("Scroll Failed: {}", e)),
                        Ok(Err(_)) => return Err(anyhow::anyhow!("Task Panic")),
                        Err(_) => return Err(anyhow::anyhow!("Scroll Timed Out")),
                    }
                }
                UiAction::ActivateApp(app) => {
                    let app_name = app.clone();
                    let task = tokio::task::spawn_blocking(move || {
                        if app_name.to_lowercase() == "frontmost" {
                            applescript::activate_frontmost_app()
                        } else {
                            applescript::activate_app(&app_name)
                        }
                    });
                    match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                        Ok(Ok(Ok(_))) => {},
                        Ok(Ok(Err(e))) => return Err(anyhow::anyhow!("Activate Failed: {}", e)),
                        Ok(Err(_)) => return Err(anyhow::anyhow!("Task Panic")),
                        Err(_) => return Err(anyhow::anyhow!("Activate Timed Out")),
                    }
                }
            }

            // 3. Post-Verification
            if let Some(post_prompt) = &step.post_verify {
                 if let Some(brain) = llm {
                    // Wait a bit for UI to settle
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    if !Self::verify_condition(brain, post_prompt).await? && step.critical {
                         return Err(anyhow::anyhow!("❌ Post-check failed: {}", post_prompt));
                    }
                }
            }
        }
        
        println!("👻 [Smart Visual Driver] Automation Complete.");
        Ok(())
    }
}

// Pre-built sequences (Updated)
pub fn n8n_fallback_create_workflow() -> VisualDriver {
    let mut driver = VisualDriver::new();
    // Legacy support wrapper
    driver.add_legacy_step(UiAction::OpenUrl("https://app.n8n.cloud".to_string()))
          .add_legacy_step(UiAction::Wait(5))
          .add_legacy_step(UiAction::Click("Create Workflow".to_string()));
    driver
}
