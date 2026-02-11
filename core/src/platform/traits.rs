/// Cross-platform abstraction traits for Steer OS Agent
///
/// This module defines the core trait interfaces that abstract platform-specific
/// functionality across macOS, Windows, and potentially Linux in the future.
///
/// # Architecture
///
/// Each platform implements these traits in their respective modules:
/// - `platform::macos::*` - macOS implementations using Core Graphics, Accessibility API
/// - `platform::windows::*` - Windows implementations using Win32 APIs, DXGI, UI Automation
///
/// # Usage
///
/// ```rust
/// use crate::platform::{get_platform, traits::*};
///
/// let platform = get_platform();
/// let screen_data = platform.screen_capture().capture_screen()?;
/// ```

use anyhow::Result;
use serde_json::Value;
use tokio::sync::mpsc;

/// Cross-platform event monitoring for keyboard and mouse events
///
/// Implementations:
/// - macOS: Uses Core Graphics `CGEventTap` for low-level event capture
/// - Windows: Uses Raw Input API with hidden window message pump
///
/// # Example
///
/// ```rust
/// let (tx, mut rx) = mpsc::channel(1000);
/// let monitor = platform.event_monitor();
/// monitor.start(tx)?;
///
/// while let Some(event_json) = rx.recv().await {
///     // Process EventEnvelope JSON
/// }
/// ```
pub trait EventMonitor: Send + Sync {
    /// Start monitoring system events (keyboard, mouse)
    ///
    /// Events are sent asynchronously to the provided channel as JSON strings
    /// conforming to the `EventEnvelope` schema.
    ///
    /// # Arguments
    ///
    /// * `tx` - Channel sender for event JSON strings
    ///
    /// # Returns
    ///
    /// * `Ok(())` if monitoring started successfully
    /// * `Err` if event tap/hook creation failed
    fn start(&self, tx: mpsc::Sender<String>) -> Result<()>;

    /// Stop monitoring and cleanup resources
    ///
    /// # Returns
    ///
    /// * `Ok(())` if cleanup successful
    /// * `Err` if cleanup failed (resources may leak)
    fn stop(&self) -> Result<()>;
}

/// Cross-platform input simulation (keyboard and mouse injection)
///
/// Implementations:
/// - macOS: Uses Core Graphics `CGEvent` API for event posting
/// - Windows: Uses `SendInput` Win32 API
///
/// # Safety
///
/// Input simulation can be used for automation but should be guarded by
/// the security policy engine to prevent unauthorized actions.
///
/// # Example
///
/// ```rust
/// let simulator = platform.input_simulator();
/// simulator.click_at(100, 200)?;
/// simulator.type_text("Hello, World!")?;
/// ```
pub trait InputSimulator: Send + Sync {
    /// Simulate mouse click at screen coordinates
    ///
    /// # Arguments
    ///
    /// * `x` - Screen X coordinate (pixels from left)
    /// * `y` - Screen Y coordinate (pixels from top)
    ///
    /// # Returns
    ///
    /// * `Ok(())` if click succeeded
    /// * `Err` if coordinates invalid or injection failed
    fn click_at(&self, x: i32, y: i32) -> Result<()>;

    /// Type text using keyboard simulation
    ///
    /// Supports full Unicode text including emoji and CJK characters.
    ///
    /// # Arguments
    ///
    /// * `text` - Text to type
    ///
    /// # Returns
    ///
    /// * `Ok(())` if typing succeeded
    /// * `Err` if keyboard injection failed
    fn type_text(&self, text: &str) -> Result<()>;

    /// Simulate key press by virtual key code
    ///
    /// # Arguments
    ///
    /// * `key_code` - Platform-specific virtual key code
    ///
    /// # Returns
    ///
    /// * `Ok(())` if key press succeeded
    /// * `Err` if key code invalid or injection failed
    fn press_key(&self, key_code: u32) -> Result<()>;

    /// Scroll in the specified direction
    ///
    /// # Arguments
    ///
    /// * `direction` - One of: "up", "down", "left", "right"
    /// * `amount` - Scroll distance (platform-specific units)
    ///
    /// # Returns
    ///
    /// * `Ok(())` if scroll succeeded
    /// * `Err` if direction invalid or injection failed
    fn scroll(&self, direction: &str, amount: i32) -> Result<()>;
}

/// Cross-platform UI automation and accessibility tree inspection
///
/// Implementations:
/// - macOS: Uses Accessibility API (`AXUIElement`)
/// - Windows: Uses UI Automation COM API (`IUIAutomation`)
///
/// # Example
///
/// ```rust
/// let accessibility = platform.accessibility();
/// let ui_tree = accessibility.snapshot(None)?;
/// let selected = accessibility.get_selected_text()?;
/// ```
pub trait AccessibilityProvider: Send + Sync {
    /// Get UI tree snapshot of focused application
    ///
    /// Returns a JSON tree representing the UI hierarchy with elements,
    /// roles, titles, and values. Tree traversal is limited to depth 2
    /// for performance.
    ///
    /// # Arguments
    ///
    /// * `scope` - Optional scope filter (e.g., "window", "app")
    ///
    /// # Returns
    ///
    /// * `Ok(Value)` - JSON tree of UI elements
    /// * `Err` if focused app inaccessible or API failed
    fn snapshot(&self, scope: Option<String>) -> Result<Value>;

    /// Get currently selected text from focused application
    ///
    /// # Returns
    ///
    /// * `Ok(Some(text))` if text is selected
    /// * `Ok(None)` if no text selected
    /// * `Err` if accessibility API failed
    fn get_selected_text(&self) -> Result<Option<String>>;

    /// Check if interactive UI element exists at screen coordinates
    ///
    /// Used for hybrid grounding to validate vision-based click targets.
    ///
    /// # Arguments
    ///
    /// * `x` - Screen X coordinate
    /// * `y` - Screen Y coordinate
    ///
    /// # Returns
    ///
    /// * `Ok(true)` if clickable element exists at position
    /// * `Ok(false)` if position is non-interactive
    /// * `Err` if coordinate query failed
    fn element_exists_at(&self, x: i32, y: i32) -> Result<bool>;

    /// Get properties of UI element at screen coordinates
    ///
    /// # Arguments
    ///
    /// * `x` - Screen X coordinate
    /// * `y` - Screen Y coordinate
    ///
    /// # Returns
    ///
    /// * `Ok(Some(info))` - JSON with role, name, value
    /// * `Ok(None)` if no element at position
    /// * `Err` if query failed
    fn get_element_info(&self, x: i32, y: i32) -> Result<Option<Value>>;
}

/// Cross-platform screen capture
///
/// Implementations:
/// - macOS: Uses `screencapture` command-line tool
/// - Windows: Uses DXGI Desktop Duplication API (GPU-accelerated)
///
/// # Example
///
/// ```rust
/// let capture = platform.screen_capture();
/// let jpeg_bytes = capture.capture_screen()?;
/// ```
pub trait ScreenCapture: Send + Sync {
    /// Capture entire primary screen as JPEG bytes
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<u8>)` - JPEG-encoded image bytes
    /// * `Err` if screen capture failed or encoding failed
    fn capture_screen(&self) -> Result<Vec<u8>>;

    /// Capture specific screen region as JPEG bytes
    ///
    /// # Arguments
    ///
    /// * `x` - Region X offset
    /// * `y` - Region Y offset
    /// * `width` - Region width
    /// * `height` - Region height
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<u8>)` - JPEG-encoded image bytes
    /// * `Err` if region invalid or capture failed
    fn capture_region(&self, x: i32, y: i32, width: u32, height: u32) -> Result<Vec<u8>>;
}

/// Cross-platform application control and automation
///
/// Implementations:
/// - macOS: Uses AppleScript via `osascript` command
/// - Windows: Uses PowerShell scripts and Win32 APIs
///
/// # Example
///
/// ```rust
/// let controller = platform.app_controller();
/// controller.launch_app("notepad")?;
/// controller.activate_app("notepad")?;
/// controller.open_url("https://example.com")?;
/// ```
pub trait AppController: Send + Sync {
    /// Launch application by name or path
    ///
    /// # Arguments
    ///
    /// * `app_name` - Application name (e.g., "Safari", "notepad.exe")
    ///
    /// # Returns
    ///
    /// * `Ok(())` if launch succeeded
    /// * `Err` if app not found or launch failed
    fn launch_app(&self, app_name: &str) -> Result<()>;

    /// Activate (bring to foreground) application by name
    ///
    /// # Arguments
    ///
    /// * `app_name` - Application name or window title
    ///
    /// # Returns
    ///
    /// * `Ok(())` if activation succeeded
    /// * `Err` if app not running or activation failed
    fn activate_app(&self, app_name: &str) -> Result<()>;

    /// Get active/focused application information
    ///
    /// # Returns
    ///
    /// * `Ok((app_name, window_title))` - Tuple of app name and window title
    /// * `Err` if query failed
    fn get_active_app(&self) -> Result<(String, String)>;

    /// Open URL in default browser
    ///
    /// # Arguments
    ///
    /// * `url` - URL to open
    ///
    /// # Returns
    ///
    /// * `Ok(())` if URL opened
    /// * `Err` if browser launch failed
    fn open_url(&self, url: &str) -> Result<()>;

    /// Execute platform-specific script
    ///
    /// - macOS: AppleScript code
    /// - Windows: PowerShell code
    ///
    /// # Arguments
    ///
    /// * `script` - Script code to execute
    ///
    /// # Returns
    ///
    /// * `Ok(output)` - Script stdout
    /// * `Err` if script execution failed
    fn run_script(&self, script: &str) -> Result<String>;
}

/// Cross-platform screen recording
///
/// Implementations:
/// - macOS: FFmpeg with `avfoundation` input
/// - Windows: FFmpeg with `gdigrab` input
///
/// # Example
///
/// ```rust
/// let recorder = platform.screen_recorder();
/// recorder.start_recording("/tmp/recording.mp4")?;
/// // ... do stuff ...
/// recorder.stop_recording()?;
/// ```
pub trait ScreenRecorder: Send + Sync {
    /// Start recording screen to file
    ///
    /// # Arguments
    ///
    /// * `output_path` - Path to output MP4 file
    ///
    /// # Returns
    ///
    /// * `Ok(())` if recording started
    /// * `Err` if FFmpeg not found or recording failed to start
    fn start_recording(&self, output_path: &str) -> Result<()>;

    /// Stop recording and finalize video file
    ///
    /// # Returns
    ///
    /// * `Ok(())` if recording stopped and file finalized
    /// * `Err` if stop failed (file may be corrupted)
    fn stop_recording(&self) -> Result<()>;

    /// Check if recording is currently active
    ///
    /// # Returns
    ///
    /// * `true` if recording is in progress
    /// * `false` if not recording
    fn is_recording(&self) -> bool;
}

/// Cross-platform system notifications
///
/// Implementations:
/// - macOS: Uses `osascript` "display notification" command
/// - Windows: Uses Windows Toast Notifications via PowerShell
///
/// # Example
///
/// ```rust
/// let notifications = platform.notifications();
/// notifications.send_notification("Steer", "Task completed!")?;
/// ```
pub trait NotificationProvider: Send + Sync {
    /// Send native system notification
    ///
    /// # Arguments
    ///
    /// * `title` - Notification title
    /// * `message` - Notification message body
    ///
    /// # Returns
    ///
    /// * `Ok(())` if notification sent
    /// * `Err` if notification system unavailable
    fn send_notification(&self, title: &str, message: &str) -> Result<()>;
}

/// Platform capabilities factory
///
/// This trait aggregates all platform-specific providers into a single
/// interface. Each platform (macOS, Windows) implements this trait to
/// provide access to their native implementations.
///
/// # Example
///
/// ```rust
/// let platform = get_platform(); // Returns Arc<dyn PlatformProvider>
/// let events = platform.event_monitor();
/// let screen = platform.screen_capture();
/// ```
pub trait PlatformProvider: Send + Sync {
    /// Get event monitoring implementation
    fn event_monitor(&self) -> Box<dyn EventMonitor>;

    /// Get input simulation implementation
    fn input_simulator(&self) -> Box<dyn InputSimulator>;

    /// Get accessibility/UI automation implementation
    fn accessibility(&self) -> Box<dyn AccessibilityProvider>;

    /// Get screen capture implementation
    fn screen_capture(&self) -> Box<dyn ScreenCapture>;

    /// Get application control implementation
    fn app_controller(&self) -> Box<dyn AppController>;

    /// Get screen recording implementation
    fn screen_recorder(&self) -> Box<dyn ScreenRecorder>;

    /// Get notification provider implementation
    fn notifications(&self) -> Box<dyn NotificationProvider>;
}
