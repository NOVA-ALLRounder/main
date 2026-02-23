#[cfg(not(target_os = "windows"))]
use crate::applescript;
use crate::browser_automation;
#[cfg(not(target_os = "windows"))]
use anyhow::Context;
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
#[cfg(not(target_os = "windows"))]
use std::fs;
use std::io::Cursor;
#[cfg(not(target_os = "windows"))]
use std::process::Command;
#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::HWND,
    Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, GetDeviceCaps, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS, HGDIOBJ, HORZRES, SRCCOPY, VERTRES,
    },
};

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

// =====================================================
// Clawdbot-inspired helper functions
// =====================================================

/// Convert error to AI-friendly format (clawdbot pattern: toAIFriendlyError)
/// Provides actionable context for the LLM to retry with a different approach
pub fn to_ai_friendly_error(err: anyhow::Error, context: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "Failed to {}: {} (Try a different approach - the element may have moved or be unavailable)",
        context,
        err
    )
}

/// Normalize timeout values to safe bounds (clawdbot pattern: normalizeTimeoutMs)
pub fn normalize_timeout_ms(value: Option<u64>, default: u64, max: u64) -> u64 {
    value.map(|v| v.max(500).min(max)).unwrap_or(default)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UiAction {
    OpenUrl(String),
    Wait(u64),           // Seconds
    Click(String),       // Element description or AppleScript target
    ClickVisual(String), // Vision-based click: "Click the blue submit button"
    Type(String),
    Scroll(String),      // "down" | "up"
    ActivateApp(String), // "frontmost" or app name
    KeyboardShortcut(String, Vec<String>), // key, modifiers (e.g. "n", ["command"])
                         // Verify(String), // Removed: Legacy standalone verify unused
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartStep {
    pub action: UiAction,
    pub description: String,
    pub pre_verify: Option<String>, // Prompt for checking BEFORE action
    pub post_verify: Option<String>, // Prompt for checking AFTER action
    pub critical: bool,             // Stop on failure?
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

    #[cfg(target_os = "windows")]
    fn capture_windows_primary_screen() -> Result<image::DynamicImage> {
        use image::{DynamicImage, ImageBuffer, Rgba};

        unsafe {
            let hwnd = HWND(std::ptr::null_mut());
            let screen_dc = GetDC(hwnd);
            if screen_dc.is_invalid() {
                return Err(AppError::Vision("GetDC failed".to_string()).into());
            }

            let width = GetDeviceCaps(screen_dc, HORZRES);
            let height = GetDeviceCaps(screen_dc, VERTRES);
            if width <= 0 || height <= 0 {
                let _ = ReleaseDC(hwnd, screen_dc);
                return Err(AppError::Vision("Invalid display dimensions".to_string()).into());
            }

            let mem_dc = CreateCompatibleDC(screen_dc);
            if mem_dc.is_invalid() {
                let _ = ReleaseDC(hwnd, screen_dc);
                return Err(AppError::Vision("CreateCompatibleDC failed".to_string()).into());
            }

            let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
            if bitmap.is_invalid() {
                let _ = DeleteDC(mem_dc);
                let _ = ReleaseDC(hwnd, screen_dc);
                return Err(AppError::Vision("CreateCompatibleBitmap failed".to_string()).into());
            }

            let old_obj = SelectObject(mem_dc, HGDIOBJ(bitmap.0));
            if old_obj.0.is_null() {
                let _ = DeleteObject(bitmap);
                let _ = DeleteDC(mem_dc);
                let _ = ReleaseDC(hwnd, screen_dc);
                return Err(AppError::Vision("SelectObject failed".to_string()).into());
            }

            let blt_ok = BitBlt(mem_dc, 0, 0, width, height, screen_dc, 0, 0, SRCCOPY).is_ok();
            if !blt_ok {
                let _ = SelectObject(mem_dc, old_obj);
                let _ = DeleteObject(bitmap);
                let _ = DeleteDC(mem_dc);
                let _ = ReleaseDC(hwnd, screen_dc);
                return Err(AppError::Vision("BitBlt failed".to_string()).into());
            }

            let pixel_count = (width as usize)
                .checked_mul(height as usize)
                .ok_or_else(|| AppError::Vision("Screen size overflow".to_string()))?;
            let mut bgra = vec![
                0u8;
                pixel_count
                    .checked_mul(4)
                    .ok_or_else(|| AppError::Vision("Buffer size overflow".to_string()))?
            ];

            let mut bmi = BITMAPINFO::default();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = width;
            // Negative height gives top-down DIB (no manual vertical flip needed)
            bmi.bmiHeader.biHeight = -height;
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB.0;

            let got = GetDIBits(
                mem_dc,
                bitmap,
                0,
                height as u32,
                Some(bgra.as_mut_ptr() as *mut std::ffi::c_void),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            let _ = SelectObject(mem_dc, old_obj);
            let _ = DeleteObject(bitmap);
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(hwnd, screen_dc);

            if got == 0 {
                return Err(AppError::Vision("GetDIBits failed".to_string()).into());
            }

            // BGRA -> RGBA
            for px in bgra.chunks_exact_mut(4) {
                px.swap(0, 2);
            }

            let img = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width as u32, height as u32, bgra)
                .ok_or_else(|| AppError::Vision("Failed to build image buffer".to_string()))?;
            Ok(DynamicImage::ImageRgba8(img))
        }
    }

    // Internal helper that can skip optimization if needed, but for now we keep it same.
    // Actually, to implement diffing efficiently, we might want the raw DynamicImage, but keeping B64 interface is easier for now to avoid refactoring everything.
    // Or better, let's just make a new helper that returns the DynamicImage for internal use.

    fn capture_image_internal() -> Result<image::DynamicImage> {
        #[cfg(target_os = "windows")]
        {
            return Self::capture_windows_primary_screen();
        }

        #[cfg(not(target_os = "windows"))]
        {
            let uuid = uuid::Uuid::new_v4();
            let mut output_path = std::env::temp_dir();
            output_path.push(format!("steer_vision_{}.jpg", uuid));
            let output_path_str = output_path.to_string_lossy().to_string();
            let status = Command::new("screencapture")
                .arg("-x")
                .arg("-t")
                .arg("jpg")
                .arg("-C")
                .arg(&output_path_str)
                .status()
                .context("Failed to run screencapture")?;

            if !status.success() {
                return Err(AppError::Vision("screencapture failed".to_string()).into());
            }

            let image_data = fs::read(&output_path).context("Failed to read image")?;
            let _ = fs::remove_file(&output_path);
            image::load_from_memory(&image_data).context("Failed to load image")
        }
    }

    fn vision_max_dim() -> u32 {
        let default = if cfg!(target_os = "windows") { 1600 } else { 1920 };
        std::env::var("STEER_VISION_MAX_DIM")
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .map(|v| v.clamp(640, 3840))
            .unwrap_or(default)
    }

    fn ui_settle_interval_ms() -> u64 {
        let default = if cfg!(target_os = "windows") { 140 } else { 200 };
        std::env::var("STEER_UI_SETTLE_INTERVAL_MS")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(|v| v.clamp(50, 1000))
            .unwrap_or(default)
    }

    fn ui_settle_required_frames() -> u32 {
        let default = if cfg!(target_os = "windows") { 2 } else { 3 };
        std::env::var("STEER_UI_SETTLE_FRAMES")
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .map(|v| v.clamp(1, 10))
            .unwrap_or(default)
    }

    pub fn capture_screen_internal(optimize: bool) -> Result<(String, f32)> {
        let img = Self::capture_image_internal()?;

        let (orig_w, _orig_h) = (img.width(), img.height());
        // ... (resizing logic from before)
        let max_dim = Self::vision_max_dim();
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

        if w1 != w2 || h1 != h2 {
            return 1.0;
        } // Changed resolution is a big diff

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

                if r_diff + g_diff + b_diff > 30 {
                    // Sensitivity threshold
                    diff_pixels += 1;
                }
            }
        }

        diff_pixels as f64 / total_pixels as f64
    }

    /// Adaptive Wait: Wait until the screen is STABLE (Diff < threshold)
    /// This ensures we don't act during animations, but proceed immediately when static.
    /// Returns Ok(true) if settled, Ok(false) if timed out.
    pub async fn wait_for_ui_settle(timeout_ms: u64) -> Result<bool> {
        let start = std::time::Instant::now();
        let threshold = 0.005; // 0.5% diff = stricter stability
        let mut consecutive_stable_frames = 0;
        let required_stable_frames = Self::ui_settle_required_frames();
        let settle_interval = Self::ui_settle_interval_ms();

        let mut prev_img = match Self::capture_image_internal() {
            Ok(img) => img,
            Err(_) => return Ok(true), // Fail safe: assume stable if capture fails (to avoid blocking)
        };

        loop {
            if start.elapsed().as_millis() as u64 > timeout_ms {
                warn!(
                    "      ??Timeout waiting for UI settle (waited {}ms). Unstable.",
                    timeout_ms
                );
                return Ok(false);
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(settle_interval)).await;

            match Self::capture_image_internal() {
                Ok(new_img) => {
                    let diff = Self::calculate_diff(&prev_img, &new_img);
                    if diff < threshold {
                        consecutive_stable_frames += 1;
                        if consecutive_stable_frames >= required_stable_frames {
                            info!("      ?∽툘 UI Settled (Ready).");
                            return Ok(true);
                        }
                    } else {
                        consecutive_stable_frames = 0;
                        info!("      ?뙄 UI Moving ({:.1}% diff)...", diff * 100.0);
                    }
                    prev_img = new_img;
                }
                Err(_) => {}
            }
        }
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

    async fn verify_condition(
        llm: &dyn crate::llm_gateway::LLMClient,
        prompt: &str,
    ) -> Result<bool> {
        info!("      ?몓截?Vision Check: '{}'", prompt);
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await; // Brief pause before capture

        match Self::capture_screen() {
            Ok((b64, _scale)) => {
                // Ignore scale for verification
                let full_prompt = format!(
                    "Screen Verification Task.\nCondition to verify: '{}'.\nReply ONLY with 'YES' or 'NO'.",
                    prompt
                );
                match llm.analyze_screen(&full_prompt, &b64).await {
                    Ok(resp) => {
                        let success = resp.trim().to_uppercase().starts_with("YES");
                        info!("      ?쨼 Result: {}", if success { "PASS" } else { "FAIL" });
                        Ok(success)
                    }
                    Err(e) => {
                        warn!("      ?좑툘 Vision API Error: {}", e);
                        Ok(false) // Conservative failure
                    }
                }
            }
            Err(e) => {
                warn!("      ?좑툘 Capture Failed: {}", e);
                Ok(false)
            }
        }
    }

    pub async fn execute(&self, llm: Option<&dyn crate::llm_gateway::LLMClient>) -> Result<()> {
        info!("?뫛 [Smart Visual Driver] Starting Verified Automation...");

        for (i, step) in self.steps.iter().enumerate() {
            info!("   Step {}: {}", i + 1, step.description);

            if let Some(pre_prompt) = &step.pre_verify {
                if let Some(brain) = llm {
                    if !Self::verify_condition(brain, pre_prompt).await? {
                        if step.critical {
                            return Err(AppError::Execution(format!(
                                "??Pre-check failed: {}",
                                pre_prompt
                            ))
                            .into());
                        }
                        warn!("      ?좑툘 Pre-check failed, but proceeding (non-critical).");
                    }
                }
            }

            match &step.action {
                UiAction::OpenUrl(url) => {
                    #[cfg(target_os = "windows")]
                    {
                        browser_automation::get_browser_automation().navigate(url, None)?;
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        crate::applescript::open_url(url).map(|_| ())?;
                    }
                }
                UiAction::Wait(secs) => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(*secs)).await;
                }
                UiAction::Click(target) => {
                    if crate::env_flag("STEER_ADAPTIVE_POLLING") {
                        let _ = Self::wait_for_ui_settle(2000).await;
                    }

                    #[cfg(target_os = "windows")]
                    {
                        let target_clone = target.clone();
                        let task = tokio::task::spawn_blocking(move || -> Result<()> {
                            let trimmed = target_clone.trim();
                            let mut browser = browser_automation::get_browser_automation();

                            if trimmed.starts_with('E') && browser.click_by_ref(trimmed, false).is_ok() {
                                return Ok(());
                            }

                            let _ = browser.take_snapshot();
                            if let Some(ref_id) = browser.find_by_name(trimmed) {
                                browser.click_by_ref(&ref_id, false)?;
                                return Ok(());
                            }

                            drop(browser);
                            crate::macos::actions::click_element(trimmed)
                                .map_err(|e| anyhow::anyhow!(e))
                        });

                        match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                            Ok(Ok(Ok(_))) => {}
                            Ok(Ok(Err(e))) => {
                                if step.critical {
                                    return Err(AppError::Execution(format!("Critical Click Failed: {}", e)).into());
                                }
                            }
                            Ok(Err(_)) => return Err(anyhow::anyhow!("Task Panic")),
                            Err(_) => {
                                if step.critical {
                                    return Err(anyhow::anyhow!("Critical Click Timed Out"));
                                }
                            }
                        }
                    }

                    #[cfg(not(target_os = "windows"))]
                    {
                        let target_clone = target.clone();
                        let script = format!(
                            "tell application \"System Events\" to click button {:?} of window 1 of (first application process whose frontmost is true)",
                            target_clone
                        );
                        let task = tokio::task::spawn_blocking(move || applescript::run(&script));

                        match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                            Ok(Ok(Ok(_))) => {}
                            Ok(Ok(Err(e))) => {
                                if step.critical {
                                    return Err(AppError::Execution(format!("Critical Click Failed: {}", e)).into());
                                }
                            }
                            Ok(Err(_)) => return Err(anyhow::anyhow!("Task Panic")),
                            Err(_) => {
                                if step.critical {
                                    return Err(anyhow::anyhow!("Critical Click Timed Out"));
                                }
                            }
                        }
                    }
                }
                UiAction::Type(text) => {
                    let text_clone = text.clone();

                    #[cfg(target_os = "windows")]
                    {
                        let task = tokio::task::spawn_blocking(move || -> Result<()> {
                            browser_automation::get_browser_automation().type_text(&text_clone, 0)?;
                            Ok(())
                        });
                        match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                            Ok(Ok(Ok(_))) => {}
                            Ok(Ok(Err(e))) => return Err(anyhow::anyhow!("Type Failed: {}", e)),
                            Ok(Err(_)) => return Err(anyhow::anyhow!("Task Panic")),
                            Err(_) => return Err(anyhow::anyhow!("Type Timed Out")),
                        }
                    }

                    #[cfg(not(target_os = "windows"))]
                    {
                        let compact: String =
                            text_clone.chars().filter(|c| !c.is_whitespace()).collect();
                        let calc_like = !compact.is_empty()
                            && compact.chars().all(|c| {
                                c.is_ascii_digit()
                                    || matches!(c, '+' | '-' | '*' | '/' | '=' | '.' | ',' | '(' | ')')
                            });

                        let task = tokio::task::spawn_blocking(move || {
                            if calc_like {
                                let script = format!(
                                    "tell application \"System Events\" to keystroke {:?}",
                                    text_clone
                                );
                                applescript::run(&script)
                            } else {
                                let lines = [
                                    "on run argv",
                                    "set targetText to item 1 of argv",
                                    "set oldClipboard to the clipboard",
                                    "set the clipboard to targetText",
                                    "tell application \"System Events\" to keystroke \"v\" using {command down}",
                                    "delay 0.35",
                                    "try",
                                    "set the clipboard to oldClipboard",
                                    "end try",
                                    "return \"ok\"",
                                    "end run",
                                ];
                                applescript::run_with_args(&lines, &[text_clone])
                            }
                        });

                        match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                            Ok(Ok(Ok(_))) => {}
                            Ok(Ok(Err(e))) => return Err(anyhow::anyhow!("Type Failed: {}", e)),
                            Ok(Err(_)) => return Err(anyhow::anyhow!("Task Panic")),
                            Err(_) => return Err(anyhow::anyhow!("Type Timed Out")),
                        }
                    }
                }
                UiAction::Scroll(direction) => {
                    let dir = direction.to_ascii_lowercase();

                    #[cfg(target_os = "windows")]
                    {
                        let pixels = if dir == "up" { -500 } else { 500 };
                        browser_automation::scroll_page(pixels)?;
                    }

                    #[cfg(not(target_os = "windows"))]
                    {
                        let key_code = if dir == "up" { 116 } else { 121 };
                        let script = format!(
                            "tell application \"System Events\" to key code {}",
                            key_code
                        );
                        let task = tokio::task::spawn_blocking(move || applescript::run(&script));
                        match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                            Ok(Ok(Ok(_))) => {}
                            Ok(Ok(Err(e))) => return Err(anyhow::anyhow!("Scroll Failed: {}", e)),
                            Ok(Err(_)) => return Err(anyhow::anyhow!("Task Panic")),
                            Err(_) => return Err(anyhow::anyhow!("Scroll Timed Out")),
                        }
                    }
                }
                UiAction::ActivateApp(app) => {
                    #[cfg(target_os = "windows")]
                    {
                        if app.to_ascii_lowercase() != "frontmost" {
                            crate::tool_chaining::CrossAppBridge::switch_to_app(app)?;
                        }
                    }

                    #[cfg(not(target_os = "windows"))]
                    {
                        let app_name = app.clone();
                        let task = tokio::task::spawn_blocking(move || {
                            if app_name.to_lowercase() == "frontmost" {
                                applescript::activate_frontmost_app()
                            } else {
                                applescript::activate_app(&app_name)
                            }
                        });
                        match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                            Ok(Ok(Ok(_))) => {}
                            Ok(Ok(Err(e))) => return Err(anyhow::anyhow!("Activate Failed: {}", e)),
                            Ok(Err(_)) => return Err(anyhow::anyhow!("Task Panic")),
                            Err(_) => return Err(anyhow::anyhow!("Activate Timed Out")),
                        }
                    }
                }
                UiAction::KeyboardShortcut(key, modifiers) => {
                    #[cfg(target_os = "windows")]
                    {
                        let key_owned = key.clone();
                        let modifiers_owned = modifiers.clone();
                        let task = tokio::task::spawn_blocking(move || {
                            send_windows_shortcut(&key_owned, &modifiers_owned)
                        });
                        match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                            Ok(Ok(Ok(_))) => {}
                            Ok(Ok(Err(e))) => return Err(anyhow::anyhow!("Shortcut Failed: {}", e)),
                            Ok(Err(_)) => return Err(anyhow::anyhow!("Task Panic")),
                            Err(_) => return Err(anyhow::anyhow!("Shortcut Timed Out")),
                        }
                    }

                    #[cfg(not(target_os = "windows"))]
                    {
                        let key_str = key.clone();
                        let mods_str = modifiers
                            .iter()
                            .map(|m| format!("{} down", m))
                            .collect::<Vec<_>>()
                            .join(", ");

                        let script = if key_str.eq_ignore_ascii_case("escape")
                            || key_str.eq_ignore_ascii_case("esc")
                        {
                            "tell application \"System Events\" to key code 53".to_string()
                        } else if modifiers.is_empty() {
                            format!(
                                "tell application \"System Events\" to keystroke \"{}\"",
                                key_str
                            )
                        } else {
                            format!(
                                "tell application \"System Events\" to keystroke \"{}\" using {{{}}}",
                                key_str, mods_str
                            )
                        };

                        let task = tokio::task::spawn_blocking(move || applescript::run(&script));
                        match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                            Ok(Ok(Ok(_))) => {}
                            Ok(Ok(Err(e))) => return Err(anyhow::anyhow!("Shortcut Failed: {}", e)),
                            Ok(Err(_)) => return Err(anyhow::anyhow!("Task Panic")),
                            Err(_) => return Err(anyhow::anyhow!("Shortcut Timed Out")),
                        }
                    }
                }
                UiAction::ClickVisual(desc) => {
                    if let Some(brain) = llm {
                        let max_retries = 2;
                        let find_timeout_ms = normalize_timeout_ms(
                            std::env::var("STEER_CLICK_VISUAL_TIMEOUT_MS")
                                .ok()
                                .and_then(|v| v.parse::<u64>().ok()),
                            8000,
                            30000,
                        );
                        let click_timeout_ms = normalize_timeout_ms(
                            std::env::var("STEER_CLICK_EXEC_TIMEOUT_MS")
                                .ok()
                                .and_then(|v| v.parse::<u64>().ok()),
                            5000,
                            15000,
                        );

                        for attempt in 0..=max_retries {
                            if attempt > 0 {
                                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                            }

                            match Self::capture_screen() {
                                Ok((b64, scale)) => {
                                    let coord_result = tokio::time::timeout(
                                        tokio::time::Duration::from_millis(find_timeout_ms),
                                        brain.find_element_coordinates(desc, &b64),
                                    )
                                    .await;

                                    match coord_result {
                                        Ok(Ok(Some((x_raw, y_raw)))) => {
                                            let x = (x_raw as f32 * scale) as i32;
                                            let y = (y_raw as f32 * scale) as i32;

                                            #[cfg(target_os = "windows")]
                                            let click_task = tokio::task::spawn_blocking(move || {
                                                click_point_windows(x, y)
                                            });

                                            #[cfg(not(target_os = "windows"))]
                                            let click_task = {
                                                let script = format!(
                                                    "tell application \"System Events\" to click at {{{}, {}}}",
                                                    x, y
                                                );
                                                tokio::task::spawn_blocking(move || applescript::run(&script))
                                            };

                                            let click_result = tokio::time::timeout(
                                                tokio::time::Duration::from_millis(click_timeout_ms),
                                                click_task,
                                            )
                                            .await;

                                            match click_result {
                                                Ok(Ok(Ok(_))) => break,
                                                Ok(Ok(Err(e))) => {
                                                    if attempt == max_retries && step.critical {
                                                        return Err(anyhow::anyhow!(
                                                            "Visual Click execution failed: {}",
                                                            e
                                                        ));
                                                    }
                                                }
                                                Ok(Err(_)) => {
                                                    if attempt == max_retries && step.critical {
                                                        return Err(anyhow::anyhow!("Visual Click task panic"));
                                                    }
                                                }
                                                Err(_) => {
                                                    if attempt == max_retries && step.critical {
                                                        return Err(anyhow::anyhow!("Visual Click execution timeout"));
                                                    }
                                                }
                                            }
                                        }
                                        Ok(Ok(None)) => {
                                            if attempt == max_retries && step.critical {
                                                return Err(anyhow::anyhow!(
                                                    "Visual Element '{}' not found after retries",
                                                    desc
                                                ));
                                            }
                                        }
                                        Ok(Err(e)) => {
                                            if attempt == max_retries && step.critical {
                                                return Err(anyhow::anyhow!("Visual Click LLM error: {}", e));
                                            }
                                        }
                                        Err(_) => {
                                            if attempt == max_retries && step.critical {
                                                return Err(anyhow::anyhow!("Visual Click LLM timeout"));
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    if attempt == max_retries && step.critical {
                                        return Err(anyhow::anyhow!("Screen capture failed: {}", e));
                                    }
                                }
                            }
                        }
                    } else {
                        warn!("      ?좑툘 No LLM client provided for Visual Click.");
                    }
                }
            }

            if let Some(post_prompt) = &step.post_verify {
                if let Some(brain) = llm {
                    if !Self::wait_for_ui_settle(4000).await? {
                        if step.critical {
                            return Err(anyhow::anyhow!("??Post-action UI failed to settle."));
                        }
                    }

                    if !Self::verify_condition(brain, post_prompt).await? && step.critical {
                        return Err(anyhow::anyhow!("??Post-check failed: {}", post_prompt));
                    }
                }
            }
        }

        info!("?뫛 [Smart Visual Driver] Automation Complete.");
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn click_point_windows(x: i32, y: i32) -> Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, SendInput,
    };
    use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;

    unsafe {
        let _ = SetCursorPos(x, y);
        let mut inputs = [INPUT::default(), INPUT::default()];
        inputs[0].r#type = INPUT_MOUSE;
        inputs[0].Anonymous.mi.dwFlags = MOUSEEVENTF_LEFTDOWN;
        inputs[1].r#type = INPUT_MOUSE;
        inputs[1].Anonymous.mi.dwFlags = MOUSEEVENTF_LEFTUP;
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn send_windows_shortcut(key: &str, modifiers: &[String]) -> Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY, VK_CONTROL, VK_MENU,
        VK_RETURN, VK_SHIFT, VK_SPACE, VK_TAB,
    };

    fn map_key(k: &str) -> VIRTUAL_KEY {
        match k.to_ascii_lowercase().as_str() {
            "escape" | "esc" => VIRTUAL_KEY(0x1B),
            "enter" | "return" => VK_RETURN,
            "tab" => VK_TAB,
            "space" => VK_SPACE,
            s if s.len() == 1 => VIRTUAL_KEY(s.chars().next().unwrap().to_ascii_uppercase() as u16),
            _ => VK_SPACE,
        }
    }

    let mut mods: Vec<VIRTUAL_KEY> = Vec::new();
    for m in modifiers {
        match m.to_ascii_lowercase().as_str() {
            "command" | "cmd" | "control" | "ctrl" => mods.push(VK_CONTROL),
            "shift" => mods.push(VK_SHIFT),
            "option" | "alt" => mods.push(VK_MENU),
            _ => {}
        }
    }

    let key_vk = map_key(key);

    unsafe {
        let total = mods.len() * 2 + 2;
        let mut inputs: Vec<INPUT> = vec![INPUT::default(); total];
        let mut idx = 0usize;

        for vk in &mods {
            inputs[idx].r#type = INPUT_KEYBOARD;
            inputs[idx].Anonymous.ki.wVk = *vk;
            idx += 1;
        }

        inputs[idx].r#type = INPUT_KEYBOARD;
        inputs[idx].Anonymous.ki.wVk = key_vk;
        idx += 1;

        inputs[idx].r#type = INPUT_KEYBOARD;
        inputs[idx].Anonymous.ki.wVk = key_vk;
        inputs[idx].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
        idx += 1;

        for vk in mods.iter().rev() {
            inputs[idx].r#type = INPUT_KEYBOARD;
            inputs[idx].Anonymous.ki.wVk = *vk;
            inputs[idx].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
            idx += 1;
        }

        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }

    Ok(())
}
// Pre-built sequences (Updated)
pub fn n8n_fallback_create_workflow() -> VisualDriver {
    let mut driver = VisualDriver::new();
    // Legacy support wrapper
    driver
        .add_legacy_step(UiAction::OpenUrl("https://app.n8n.cloud".to_string()))
        .add_legacy_step(UiAction::Wait(5))
        .add_legacy_step(UiAction::Click("Create Workflow".to_string()));
    driver
}


