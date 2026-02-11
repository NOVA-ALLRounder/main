/// Windows platform implementation using Win32 APIs, DXGI, and UI Automation
///
/// This module provides Windows-specific implementations of all platform traits
/// defined in `crate::platform::traits`.

pub mod events;
pub mod actions;
pub mod accessibility;
pub mod screen_capture;
pub mod app_control;
pub mod notifications;

use crate::platform::traits::*;
use anyhow::Result;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

/// Windows platform provider
///
/// Aggregates all Windows-specific implementations into a single provider.
pub struct WindowsPlatform {
    _com_guard: ComGuard,
}

impl WindowsPlatform {
    /// Create a new Windows platform provider
    ///
    /// Initializes COM library with COINIT_MULTITHREADED for UI Automation.
    pub fn new() -> Result<Self> {
        Ok(Self {
            _com_guard: ComGuard::new()?,
        })
    }
}

impl PlatformProvider for WindowsPlatform {
    fn event_monitor(&self) -> Box<dyn EventMonitor> {
        Box::new(events::WindowsEventMonitor::new())
    }

    fn input_simulator(&self) -> Box<dyn InputSimulator> {
        Box::new(actions::WindowsInputSimulator)
    }

    fn accessibility(&self) -> Box<dyn AccessibilityProvider> {
        match accessibility::WindowsAccessibility::new() {
            Ok(accessibility) => Box::new(accessibility),
            Err(e) => {
                eprintln!("?醫묓닔  Failed to initialize Windows UI Automation: {}", e);
                Box::new(accessibility::StubAccessibility)
            }
        }
    }

    fn screen_capture(&self) -> Box<dyn ScreenCapture> {
        match screen_capture::WindowsScreenCapture::new() {
            Ok(capture) => Box::new(capture),
            Err(e) => {
                eprintln!("?醫묓닔  Failed to initialize DXGI screen capture: {}", e);
                Box::new(screen_capture::StubScreenCapture)
            }
        }
    }

    fn app_controller(&self) -> Box<dyn AppController> {
        Box::new(app_control::WindowsAppController)
    }

    fn screen_recorder(&self) -> Box<dyn ScreenRecorder> {
        Box::new(screen_capture::WindowsScreenRecorder::new())
    }

    fn notifications(&self) -> Box<dyn NotificationProvider> {
        Box::new(notifications::WindowsNotifications)
    }
}

/// RAII guard for COM initialization/uninitialization
struct ComGuard;

impl ComGuard {
    fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .map_err(|e| anyhow::anyhow!("Failed to initialize COM: {:?}", e))?;
        }
        Ok(Self)
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

/// Initialize COM library (called from platform::initialize())
pub fn initialize_com() -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .map_err(|e| anyhow::anyhow!("Failed to initialize COM: {:?}", e))?;
    }
    Ok(())
}

/// Cleanup COM library (called from platform::cleanup())
pub fn cleanup_com() -> Result<()> {
    unsafe {
        CoUninitialize();
    }
    Ok(())
}
