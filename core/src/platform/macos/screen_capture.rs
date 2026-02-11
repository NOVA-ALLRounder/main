/// macOS screen capture implementation using screencapture CLI and FFmpeg

use crate::platform::traits::{ScreenCapture, ScreenRecorder};
use anyhow::{Context, Result};
use std::process::{Command, Child, Stdio};
use std::sync::Mutex;

/// macOS screen capture using screencapture command
pub struct MacOSScreenCapture;

impl ScreenCapture for MacOSScreenCapture {
    fn capture_screen(&self) -> Result<Vec<u8>> {
        let uuid = uuid::Uuid::new_v4();
        let output_path = format!("/tmp/steer_capture_{}.jpg", uuid);

        // Use macOS screencapture command
        let status = Command::new("screencapture")
            .arg("-x") // No sound
            .arg("-t")
            .arg("jpg") // JPEG format
            .arg("-C") // Capture cursor
            .arg(&output_path)
            .status()
            .context("Failed to run screencapture command")?;

        if !status.success() {
            anyhow::bail!("screencapture command failed");
        }

        // Read the captured image
        let image_bytes = std::fs::read(&output_path)
            .context("Failed to read captured image")?;

        // Cleanup temporary file
        let _ = std::fs::remove_file(&output_path);

        Ok(image_bytes)
    }

    fn capture_region(&self, x: i32, y: i32, width: u32, height: u32) -> Result<Vec<u8>> {
        let uuid = uuid::Uuid::new_v4();
        let output_path = format!("/tmp/steer_capture_{}.jpg", uuid);

        // Use screencapture with region specification
        let region = format!("{},{},{},{}", x, y, width, height);
        let status = Command::new("screencapture")
            .arg("-x")
            .arg("-t")
            .arg("jpg")
            .arg("-R")
            .arg(&region)
            .arg(&output_path)
            .status()
            .context("Failed to run screencapture command")?;

        if !status.success() {
            anyhow::bail!("screencapture command failed");
        }

        let image_bytes = std::fs::read(&output_path)
            .context("Failed to read captured image")?;

        let _ = std::fs::remove_file(&output_path);

        Ok(image_bytes)
    }
}

/// macOS screen recorder using FFmpeg with avfoundation
pub struct MacOSScreenRecorder {
    child: Mutex<Option<Child>>,
}

impl MacOSScreenRecorder {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
        }
    }
}

impl ScreenRecorder for MacOSScreenRecorder {
    fn start_recording(&self, output_path: &str) -> Result<()> {
        let mut child_guard = self.child.lock().unwrap();

        if child_guard.is_some() {
            anyhow::bail!("Recording already in progress");
        }

        // Start FFmpeg with avfoundation input (macOS screen capture)
        let child = Command::new("ffmpeg")
            .args(&[
                "-f", "avfoundation",
                "-r", "5", // 5 FPS (low overhead)
                "-i", "1", // Capture display 1
                "-vf", "scale=1280:-1", // Downscale to 720p width
                "-c:v", "libx264",
                "-preset", "ultrafast",
                "-pix_fmt", "yuv420p",
                "-y", // Overwrite output file
                output_path,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to start FFmpeg recording")?;

        *child_guard = Some(child);
        Ok(())
    }

    fn stop_recording(&self) -> Result<()> {
        let mut child_guard = self.child.lock().unwrap();

        if let Some(mut child) = child_guard.take() {
            // Send 'q' to stdin to gracefully stop FFmpeg
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(b"q");
            }

            // Wait for process to exit
            let _ = child.wait();
        }

        Ok(())
    }

    fn is_recording(&self) -> bool {
        self.child.lock().unwrap().is_some()
    }
}

impl Default for MacOSScreenRecorder {
    fn default() -> Self {
        Self::new()
    }
}
