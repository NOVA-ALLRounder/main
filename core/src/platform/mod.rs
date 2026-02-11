/// Platform abstraction layer for Steer OS Agent
///
/// This module provides cross-platform abstractions for system-level operations
/// including event monitoring, input simulation, screen capture, and UI automation.
///
/// # Supported Platforms
///
/// - **macOS**: Full support via Core Graphics, Accessibility API, and AppleScript
/// - **Windows**: Full support via Win32 APIs, DXGI, and UI Automation (Phase 2+)
///
/// # Architecture
///
/// Each platform implements the `PlatformProvider` trait which provides access
/// to platform-specific implementations of core capabilities:
///
/// ```text
/// PlatformProvider
///   ????? EventMonitor      (keyboard/mouse capture)
///   ????? InputSimulator    (keyboard/mouse injection)
///   ????? AccessibilityProvider (UI tree inspection)
///   ????? ScreenCapture     (screenshot capture)
///   ????? AppController     (app launch/activation)
///   ????? ScreenRecorder    (video recording)
///   ?遺??? NotificationProvider (system notifications)
/// ```
///
/// # Usage
///
/// ```rust
/// use crate::platform;
///
/// // Initialize platform subsystems (COM on Windows, etc.)
/// platform::initialize()?;
///
/// // Get platform-specific provider
/// let platform = platform::get_platform();
///
/// // Use platform capabilities
/// let jpeg_bytes = platform.screen_capture().capture_screen()?;
/// ```

pub mod traits;
pub mod common;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

// Platform-specific utilities
pub mod applescript;
pub mod shell_analysis;

pub use applescript::*;
pub use shell_analysis::*;

use traits::PlatformProvider;
use std::sync::Arc;
use anyhow::Result;

/// Get platform provider for the current operating system
///
/// Returns an `Arc<dyn PlatformProvider>` that can be cloned and shared across threads.
/// The provider is lazily initialized and cached internally.
///
/// # Panics
///
/// Panics if running on an unsupported platform (not macOS or Windows).
///
/// # Example
///
/// ```rust
/// let platform = get_platform();
/// let events = platform.event_monitor();
/// events.start(tx)?;
/// ```
pub fn get_platform() -> Arc<dyn PlatformProvider> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(macos::MacOSPlatform::new())
    }

    #[cfg(target_os = "windows")]
    {
        Arc::new(windows::WindowsPlatform::new()
            .expect("Failed to initialize Windows platform"))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        compile_error!(
            "Unsupported platform. Steer OS Agent requires macOS or Windows. \
             Linux support is planned for future releases."
        )
    }
}

/// Initialize platform-specific subsystems
///
/// This function must be called once at application startup before using
/// any platform capabilities. It performs platform-specific initialization:
///
/// - **Windows**: Initializes COM library with `COINIT_MULTITHREADED`
/// - **macOS**: No initialization required (returns `Ok(())` immediately)
///
/// # Errors
///
/// Returns `Err` if platform initialization fails:
/// - Windows: COM initialization failure
///
/// # Safety
///
/// This function initializes global state and should only be called once
/// from the main thread before spawning any worker threads.
///
/// # Example
///
/// ```rust
/// fn main() -> Result<()> {
///     platform::initialize()?;
///     // ... rest of application
///     Ok(())
/// }
/// ```
pub fn initialize() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows::initialize_com()?;
    }

    #[cfg(target_os = "macos")]
    {
        // macOS doesn't require global initialization
    }

    Ok(())
}

/// Cleanup platform-specific resources
///
/// This function should be called at application shutdown to properly
/// cleanup platform resources:
///
/// - **Windows**: Uninitializes COM library
/// - **macOS**: No cleanup required
///
/// # Example
///
/// ```rust
/// fn main() -> Result<()> {
///     platform::initialize()?;
///     // ... application logic
///     platform::cleanup()?;
///     Ok(())
/// }
/// ```
pub fn cleanup() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows::cleanup_com()?;
    }

    #[cfg(target_os = "macos")]
    {
        // macOS doesn't require global cleanup
    }

    Ok(())
}

/// Get the current platform name as a string
///
/// # Returns
///
/// - `"macos"` on macOS
/// - `"windows"` on Windows
///
/// # Example
///
/// ```rust
/// println!("Running on: {}", platform::platform_name());
/// // Output: "Running on: macos" or "Running on: windows"
/// ```
pub fn platform_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "macos"
    }

    #[cfg(target_os = "windows")]
    {
        "windows"
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        "unsupported"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        let name = platform_name();
        assert!(name == "macos" || name == "windows");
    }

    #[test]
    fn test_platform_initialization() {
        assert!(initialize().is_ok());
    }

    #[test]
    fn test_get_platform() {
        let platform = get_platform();
        // Platform should be valid (doesn't panic)
        assert!(Arc::strong_count(&platform) >= 1);
    }

    #[test]
    fn test_platform_capabilities() {
        let platform = get_platform();

        // All capabilities should be accessible
        let _events = platform.event_monitor();
        let _input = platform.input_simulator();
        let _accessibility = platform.accessibility();
        let _capture = platform.screen_capture();
        let _controller = platform.app_controller();
        let _recorder = platform.screen_recorder();
        let _notifications = platform.notifications();
    }
}
