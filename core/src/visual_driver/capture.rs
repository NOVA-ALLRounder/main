use super::*;

impl VisualDriver {
    pub fn capture_screen() -> Result<(String, f32)> {
        Self::capture_screen_internal(true)
    }

    fn capture_image_internal() -> Result<image::DynamicImage> {
        let uuid = uuid::Uuid::new_v4();
        let output_path = format!("/tmp/steer_vision_{}.jpg", uuid);

        let status = Command::new("screencapture")
            .arg("-x")
            .arg("-t")
            .arg("jpg")
            .arg("-C")
            .arg(&output_path)
            .status()
            .context("Failed to run screencapture")?;

        if !status.success() {
            return Err(AppError::Vision("screencapture failed".to_string()).into());
        }

        let image_data = fs::read(&output_path).context("Failed to read image")?;
        let _ = fs::remove_file(&output_path);

        image::load_from_memory(&image_data).context("Failed to load image")
    }

    pub(super) fn capture_screen_internal(optimize: bool) -> Result<(String, f32)> {
        let img = Self::capture_image_internal()?;

        let (orig_w, _orig_h) = (img.width(), img.height());
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

    fn calculate_diff(img1: &image::DynamicImage, img2: &image::DynamicImage) -> f64 {
        use image::GenericImageView;
        let (w1, h1) = img1.dimensions();
        let (w2, h2) = img2.dimensions();

        if w1 != w2 || h1 != h2 {
            return 1.0;
        }

        let thumb1 = img1.resize_exact(256, 144, image::imageops::FilterType::Nearest);
        let thumb2 = img2.resize_exact(256, 144, image::imageops::FilterType::Nearest);

        let mut diff_pixels = 0;
        let total_pixels = 256 * 144;

        for y in 0..144 {
            for x in 0..256 {
                let p1 = thumb1.get_pixel(x, y);
                let p2 = thumb2.get_pixel(x, y);

                let r_diff = (p1[0] as i32 - p2[0] as i32).abs();
                let g_diff = (p1[1] as i32 - p2[1] as i32).abs();
                let b_diff = (p1[2] as i32 - p2[2] as i32).abs();

                if r_diff + g_diff + b_diff > 30 {
                    diff_pixels += 1;
                }
            }
        }

        diff_pixels as f64 / total_pixels as f64
    }

    pub(super) async fn wait_for_ui_settle(timeout_ms: u64) -> Result<bool> {
        let start = std::time::Instant::now();
        let threshold = 0.005;
        let mut consecutive_stable_frames = 0;
        let required_stable_frames = 3;

        let mut prev_img = match Self::capture_image_internal() {
            Ok(img) => img,
            Err(_) => return Ok(true),
        };

        loop {
            if start.elapsed().as_millis() as u64 > timeout_ms {
                warn!(
                    "      ⏰ Timeout waiting for UI settle (waited {}ms). Unstable.",
                    timeout_ms
                );
                return Ok(false);
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            if let Ok(new_img) = Self::capture_image_internal() {
                let diff = Self::calculate_diff(&prev_img, &new_img);
                if diff < threshold {
                    consecutive_stable_frames += 1;
                    if consecutive_stable_frames >= required_stable_frames {
                        info!("      ⚡️ UI Settled (Ready).");
                        return Ok(true);
                    }
                } else {
                    consecutive_stable_frames = 0;
                    info!("      🌊 UI Moving ({:.1}% diff)...", diff * 100.0);
                }
                prev_img = new_img;
            }
        }
    }

    pub(super) async fn verify_condition(
        llm: &dyn crate::llm_gateway::LLMClient,
        prompt: &str,
    ) -> Result<bool> {
        info!("      👁️ Vision Check: '{}'", prompt);
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        match Self::capture_screen() {
            Ok((b64, _scale)) => {
                let full_prompt = format!(
                    "Screen Verification Task.\nCondition to verify: '{}'.\nReply ONLY with 'YES' or 'NO'.",
                    prompt
                );
                match llm.analyze_screen(&full_prompt, &b64).await {
                    Ok(resp) => {
                        let success = resp.trim().to_uppercase().starts_with("YES");
                        info!("      🤖 Result: {}", if success { "PASS" } else { "FAIL" });
                        Ok(success)
                    }
                    Err(e) => {
                        warn!("      ⚠️ Vision API Error: {}", e);
                        Ok(false)
                    }
                }
            }
            Err(e) => {
                warn!("      ⚠️ Capture Failed: {}", e);
                Ok(false)
            }
        }
    }
}
