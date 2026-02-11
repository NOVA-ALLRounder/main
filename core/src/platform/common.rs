/// Common utilities shared across platform implementations
///
/// This module contains cross-platform helper functions and types that are
/// used by multiple platform implementations (macOS, Windows).

use anyhow::{Context, Result};
use std::process::Command;

/// Execute a shell command and return stdout
///
/// This is a platform-aware wrapper around `Command::new()` that uses:
/// - macOS/Linux: `sh -c`
/// - Windows: `cmd /C`
///
/// # Arguments
///
/// * `command` - Shell command to execute
///
/// # Returns
///
/// * `Ok(String)` - Command stdout as string
/// * `Err` if command failed or returned non-zero exit code
///
/// # Example
///
/// ```rust
/// let output = execute_shell_command("echo hello")?;
/// assert_eq!(output.trim(), "hello");
/// ```
pub fn execute_shell_command(command: &str) -> Result<String> {
    #[cfg(target_os = "windows")]
    let output = Command::new("cmd")
        .args(&["/C", command])
        .output()
        .with_context(|| format!("Failed to execute command: {}", command))?;

    #[cfg(not(target_os = "windows"))]
    let output = Command::new("sh")
        .args(&["-c", command])
        .output()
        .with_context(|| format!("Failed to execute command: {}", command))?;

    if !output.status.success() {
        anyhow::bail!(
            "Command failed with exit code {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Coordinate conversion utilities
pub mod coordinates {
    /// Convert screen coordinates from one coordinate space to another
    ///
    /// # Arguments
    ///
    /// * `x` - X coordinate
    /// * `y` - Y coordinate
    /// * `from_width` - Source coordinate space width
    /// * `from_height` - Source coordinate space height
    /// * `to_width` - Target coordinate space width
    /// * `to_height` - Target coordinate space height
    ///
    /// # Returns
    ///
    /// Tuple of (new_x, new_y) in target coordinate space
    pub fn convert_coords(
        x: i32,
        y: i32,
        from_width: u32,
        from_height: u32,
        to_width: u32,
        to_height: u32,
    ) -> (i32, i32) {
        let new_x = (x as f64 / from_width as f64 * to_width as f64) as i32;
        let new_y = (y as f64 / from_height as f64 * to_height as f64) as i32;
        (new_x, new_y)
    }

    /// Convert pixel coordinates to absolute coordinates (0-65535 range)
    ///
    /// Used by Windows `SendInput` API which requires absolute mouse coordinates.
    ///
    /// # Arguments
    ///
    /// * `x` - Pixel X coordinate
    /// * `y` - Pixel Y coordinate
    /// * `screen_width` - Screen width in pixels
    /// * `screen_height` - Screen height in pixels
    ///
    /// # Returns
    ///
    /// Tuple of (absolute_x, absolute_y) in 0-65535 range
    pub fn pixel_to_absolute(x: i32, y: i32, screen_width: u32, screen_height: u32) -> (i32, i32) {
        let abs_x = (x * 65536) / screen_width as i32;
        let abs_y = (y * 65536) / screen_height as i32;
        (abs_x, abs_y)
    }

    /// Convert absolute coordinates (0-65535 range) to pixel coordinates
    ///
    /// # Arguments
    ///
    /// * `abs_x` - Absolute X coordinate (0-65535)
    /// * `abs_y` - Absolute Y coordinate (0-65535)
    /// * `screen_width` - Screen width in pixels
    /// * `screen_height` - Screen height in pixels
    ///
    /// # Returns
    ///
    /// Tuple of (pixel_x, pixel_y)
    pub fn absolute_to_pixel(
        abs_x: i32,
        abs_y: i32,
        screen_width: u32,
        screen_height: u32,
    ) -> (i32, i32) {
        let pixel_x = (abs_x * screen_width as i32) / 65536;
        let pixel_y = (abs_y * screen_height as i32) / 65536;
        (pixel_x, pixel_y)
    }
}

/// Image processing utilities
pub mod image_utils {
    use anyhow::Result;

    /// Convert BGRA pixel data to RGB
    ///
    /// Windows DXGI returns BGRA format, but JPEG encoding expects RGB.
    ///
    /// # Arguments
    ///
    /// * `bgra_pixels` - Source pixels in BGRA format
    ///
    /// # Returns
    ///
    /// Vec of RGB pixels (3 bytes per pixel)
    pub fn bgra_to_rgb(bgra_pixels: &[u8]) -> Vec<u8> {
        let mut rgb_pixels = Vec::with_capacity((bgra_pixels.len() / 4) * 3);
        for chunk in bgra_pixels.chunks(4) {
            if chunk.len() >= 3 {
                rgb_pixels.push(chunk[2]); // R
                rgb_pixels.push(chunk[1]); // G
                rgb_pixels.push(chunk[0]); // B
            }
        }
        rgb_pixels
    }

    /// Convert RGBA pixel data to RGB
    ///
    /// # Arguments
    ///
    /// * `rgba_pixels` - Source pixels in RGBA format
    ///
    /// # Returns
    ///
    /// Vec of RGB pixels (3 bytes per pixel)
    pub fn rgba_to_rgb(rgba_pixels: &[u8]) -> Vec<u8> {
        let mut rgb_pixels = Vec::with_capacity((rgba_pixels.len() / 4) * 3);
        for chunk in rgba_pixels.chunks(4) {
            if chunk.len() >= 3 {
                rgb_pixels.push(chunk[0]); // R
                rgb_pixels.push(chunk[1]); // G
                rgb_pixels.push(chunk[2]); // B
            }
        }
        rgb_pixels
    }

    /// Encode RGB pixel data to JPEG bytes
    ///
    /// # Arguments
    ///
    /// * `rgb_pixels` - RGB pixel data (3 bytes per pixel)
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    /// * `quality` - JPEG quality (1-100, default 85)
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<u8>)` - JPEG-encoded bytes
    /// * `Err` if encoding failed
    pub fn encode_jpeg(
        rgb_pixels: Vec<u8>,
        width: u32,
        height: u32,
        quality: Option<u8>,
    ) -> Result<Vec<u8>> {
        use image::{ImageBuffer, RgbImage};
        

        let img: RgbImage = ImageBuffer::from_raw(width, height, rgb_pixels)
            .ok_or_else(|| anyhow::anyhow!("Failed to create image from raw pixels"))?;

        let mut jpeg_bytes = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
            &mut jpeg_bytes,
            quality.unwrap_or(85),
        );

        encoder
            .encode_image(&img)
            .map_err(|e| anyhow::anyhow!("Failed to encode JPEG: {}", e))?;

        Ok(jpeg_bytes)
    }
}

/// Platform-specific temporary directory paths
pub mod temp_paths {
    use std::path::PathBuf;

    /// Get temporary directory for screen captures
    ///
    /// Returns:
    /// - macOS/Linux: `/tmp/`
    /// - Windows: `%TEMP%`
    pub fn get_temp_dir() -> PathBuf {
        std::env::temp_dir()
    }

    /// Generate a unique temporary file path with UUID
    ///
    /// # Arguments
    ///
    /// * `prefix` - File name prefix
    /// * `extension` - File extension (without dot)
    ///
    /// # Returns
    ///
    /// PathBuf to unique temporary file
    ///
    /// # Example
    ///
    /// ```rust
    /// let path = generate_temp_file("steer_vision", "jpg");
    /// // Returns: /tmp/steer_vision_<uuid>.jpg (macOS)
    /// //      or: C:\Users\<user>\AppData\Local\Temp\steer_vision_<uuid>.jpg (Windows)
    /// ```
    pub fn generate_temp_file(prefix: &str, extension: &str) -> PathBuf {
        let uuid = uuid::Uuid::new_v4();
        let filename = format!("{}_{}.{}", prefix, uuid, extension);
        get_temp_dir().join(filename)
    }
}

/// Process management utilities
pub mod process {
    use std::process::Child;
    use anyhow::Result;

    /// Kill a child process safely
    ///
    /// # Arguments
    ///
    /// * `child` - Child process to terminate
    ///
    /// # Returns
    ///
    /// * `Ok(())` if process killed successfully
    /// * `Err` if kill failed
    pub fn kill_child(child: &mut Child) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            child.kill()?;
        }

        #[cfg(not(target_os = "windows"))]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;

            if let Some(pid) = child.id() {
                let pid = Pid::from_raw(pid as i32);
                kill(pid, Signal::SIGTERM)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_shell_command() {
        #[cfg(target_os = "windows")]
        let output = execute_shell_command("echo hello").unwrap();

        #[cfg(not(target_os = "windows"))]
        let output = execute_shell_command("echo hello").unwrap();

        assert!(output.contains("hello"));
    }

    #[test]
    fn test_coordinate_conversion() {
        let (x, y) = coordinates::convert_coords(100, 100, 1920, 1080, 3840, 2160);
        assert_eq!(x, 200);
        assert_eq!(y, 200);
    }

    #[test]
    fn test_pixel_to_absolute() {
        let (abs_x, abs_y) = coordinates::pixel_to_absolute(1920, 1080, 1920, 1080);
        assert_eq!(abs_x, 65536);
        assert_eq!(abs_y, 65536);
    }

    #[test]
    fn test_bgra_to_rgb() {
        let bgra = vec![255, 0, 0, 255, 0, 255, 0, 255]; // Blue, Green
        let rgb = image_utils::bgra_to_rgb(&bgra);
        assert_eq!(rgb, vec![0, 0, 255, 0, 255, 0]); // Red, Green in RGB
    }

    #[test]
    fn test_temp_dir() {
        let temp = temp_paths::get_temp_dir();
        assert!(temp.exists());
    }
}
