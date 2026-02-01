use crate::executor;
use crate::applescript;
use anyhow::{Context, Result};
use std::process::Command;
use std::fs;
use base64::{Engine as _, engine::general_purpose};
use image::io::Reader as ImageReader;
use std::io::Cursor;

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UiAction {
    OpenUrl(String),
    Wait(u64), // Seconds
    Click(String), // Element description or AppleScript target
    ClickVisual(String), // Vision-based click: "Click the blue submit button"
    Type(String),
    Scroll(String), // "down" | "up"
    ActivateApp(String), // "frontmost" or app name
    // Verify(String), // Removed: Legacy standalone verify unused
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub steps: Vec<SmartStep>,
}

impl VisualDriver {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn capture_screen() -> Result<(String, f32)> {
        // ... (existing code, ensure it returns result)
        // I will just reuse the existing implementation
        Self::capture_screen_internal(true)
    }
    
    // Internal helper that can skip optimization if needed, but for now we keep it same.
    // Actually, to implement diffing efficiently, we might want the raw DynamicImage, but keeping B64 interface is easier for now to avoid refactoring everything.
    // Or better, let's just make a new helper that returns the DynamicImage for internal use.
    
    fn capture_image_internal() -> Result<image::DynamicImage> {
         let uuid = uuid::Uuid::new_v4();
        let output_path = format!("/tmp/steer_vision_{}.jpg", uuid);
        
        let status = Command::new("screencapture")
            .arg("-x").arg("-t").arg("jpg").arg("-C").arg(&output_path)
            .status().context("Failed to run screencapture")?;
            
        if !status.success() { return Err(anyhow::anyhow!("screencapture failed")); }

        let image_data = fs::read(&output_path).context("Failed to read image")?;
        let _ = fs::remove_file(&output_path);

        image::load_from_memory(&image_data).context("Failed to load image")
    }

    pub fn capture_screen_internal(optimize: bool) -> Result<(String, f32)> {
        let img = Self::capture_image_internal()?;
        
        let (orig_w, _orig_h) = (img.width(), img.height());
        // ... (resizing logic from before)
        let max_dim = 1920u32;
        let scale_factor = if optimize && orig_w > max_dim {
            orig_w as f32 / max_dim as f32
        } else {
            1.0
        };

        let resized = if scale_factor > 1.0 {
            img.resize(max_dim, max_dim, image::imageops::FilterType::Triangle)
        } else {
            img
        };

        let mut buffer = Cursor::new(Vec::new());
        resized.write_to(&mut buffer, image::ImageOutputFormat::Jpeg(80))?;
        let b64 = general_purpose::STANDARD.encode(buffer.get_ref());
        Ok((b64, scale_factor))
    }

    /// Calculate percentage difference between two images (0.0 to 1.0)
    fn calculate_diff(img1: &image::DynamicImage, img2: &image::DynamicImage) -> f64 {
        use image::GenericImageView;
        let (w1, h1) = img1.dimensions();
        let (w2, h2) = img2.dimensions();
        
        if w1 != w2 || h1 != h2 { return 1.0; } // Changed resolution is a big diff

        // Simple pixel diff (could be optimized with checking random samples for speed)
        // For 1 second wait, full scan might be slow if 4K.
        // Let's resize both to small thumbnails for comparison (e.g., 256x ?)
        let thumb1 = img1.resize_exact(256, 144, image::imageops::FilterType::Nearest);
        let thumb2 = img2.resize_exact(256, 144, image::imageops::FilterType::Nearest);
        
        let mut diff_pixels = 0;
        let total_pixels = 256 * 144;
        
        for y in 0..144 {
            for x in 0..256 {
                let p1 = thumb1.get_pixel(x, y);
                let p2 = thumb2.get_pixel(x, y);
                
                // RGB Euclidean distance
                let r_diff = (p1[0] as i32 - p2[0] as i32).abs();
                let g_diff = (p1[1] as i32 - p2[1] as i32).abs();
                let b_diff = (p1[2] as i32 - p2[2] as i32).abs();
                
                if r_diff + g_diff + b_diff > 30 { // Sensitivity threshold
                    diff_pixels += 1;
                }
            }
        }
        
        diff_pixels as f64 / total_pixels as f64
    }

    /// Adaptive Wait: Wait until the screen is STABLE (Diff < threshold)
    /// This ensures we don't act during animations, but proceed immediately when static.
    pub async fn wait_for_ui_settle(timeout_ms: u64) -> Result<()> {
        let start = std::time::Instant::now();
        let threshold = 0.01; // 1% diff = stable
        let mut consecutive_stable_frames = 0;
        let required_stable_frames = 2; // ~400ms of stability

        let mut prev_img = match Self::capture_image_internal() {
            Ok(img) => img,
            Err(_) => return Ok(()), // Fail safe
        };

        loop {
            if start.elapsed().as_millis() as u64 > timeout_ms {
                println!("      ⏰ Timeout waiting for UI settle (waited {}ms). Proceeding.", timeout_ms);
                break;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            
            match Self::capture_image_internal() {
                Ok(new_img) => {
                    let diff = Self::calculate_diff(&prev_img, &new_img);
                    if diff < threshold {
                        consecutive_stable_frames += 1;
                        // print!("."); // Heartbeat
                        if consecutive_stable_frames >= required_stable_frames {
                            println!("      ⚡️ UI Settled (Ready).");
                            break;
                        }
                    } else {
                        consecutive_stable_frames = 0;
                         println!("      🌊 UI Moving ({:.1}% diff)...", diff * 100.0);
                    }
                    prev_img = new_img;
                },
                Err(_) => {} 
            }
        }
        Ok(())
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
            Ok((b64, _scale)) => { // Ignore scale for verification
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
                UiAction::ClickVisual(desc) => {
                    println!("      👁️ Vision Click: Finding '{}'...", desc);
                    if let Some(brain) = llm {
                         let max_retries = 2;
                         for attempt in 0..=max_retries {
                             if attempt > 0 {
                                 println!("      ⏳ Retry {}/{}: Re-observing screen...", attempt, max_retries);
                                 tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                             }

                             match Self::capture_screen() {
                                Ok((b64, scale)) => {
                                    match brain.find_element_coordinates(desc, &b64).await {
                                        Ok(Some((x_raw, y_raw))) => {
                                            // Apply scaling back to original screen size
                                            let x = (x_raw as f32 * scale) as i32;
                                            let y = (y_raw as f32 * scale) as i32;
                                            println!("      🎯 LLM Target: ({}, {}) [Scaled x{:.2}]", x, y, scale);
                                            
                                            // [Phase 4] Hybrid Grounding (macOS only)
                                            #[cfg(target_os = "macos")]
                                            {
                                                use crate::macos::accessibility;
                                                if let Some((_sx, _sy)) = accessibility::get_element_center_at(x, y) {
                                                    println!("      🧲 Grounded: Valid UI Element confirmed at ({}, {})", x, y);
                                                } else {
                                                    println!("      ⚠️  Warning: No UI Element found at coordinates via Accessibility API.");
                                                    // Optional: If we are strict, we could continue (retry) here.
                                                    // For now, we trust vision but warn.
                                                }
                                            }

                                            let script = format!("tell application \"System Events\" to click at {{{}, {}}}", x, y);
                                            if let Err(e) = applescript::run(&script) {
                                                 println!("      ❌ Click visual script failed: {}", e);
                                                 if step.critical { return Err(anyhow::anyhow!("Visual Click execution failed")); }
                                            }
                                            break; // Success!
                                        },
                                        Ok(None) => {
                                            println!("      ⚠️ Element '{}' not found on screen.", desc);
                                            if attempt == max_retries && step.critical {
                                                 return Err(anyhow::anyhow!("Visual Element '{}' not found after retries", desc));
                                            }
                                        },
                                        Err(e) => {
                                            println!("      ⚠️ LLM Vision Error: {}", e);
                                            if attempt == max_retries && step.critical {
                                                 return Err(anyhow::anyhow!("Visual Click LLM error"));
                                            }
                                        }
                                    }
                                },
                                Err(e) => {
                                    println!("      ❌ Screen capture failed: {}", e);
                                    if attempt == max_retries && step.critical { return Err(anyhow::anyhow!("Screen capture failed")); }
                                }
                             }
                         }
                    } else {
                        println!("      ⚠️ No LLM client provided for Visual Click.");
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
