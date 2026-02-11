/// macOS platform implementation using Core Graphics, Accessibility API, and AppleScript
///
/// This module provides macOS-specific implementations of all platform traits
/// defined in `crate::platform::traits`.

pub mod events;
pub mod accessibility;
pub mod actions;
pub mod screen_capture;
pub mod app_control;
pub mod notifications;

use crate::platform::traits::*;
use std::sync::Arc;

/// macOS platform provider
///
/// Aggregates all macOS-specific implementations into a single provider.
pub struct MacOSPlatform;

impl MacOSPlatform {
    /// Create a new macOS platform provider
    pub fn new() -> Self {
        Self
    }
}

impl PlatformProvider for MacOSPlatform {
    fn event_monitor(&self) -> Box<dyn EventMonitor> {
        Box::new(events::MacOSEventMonitor)
    }

    fn input_simulator(&self) -> Box<dyn InputSimulator> {
        Box::new(actions::MacOSInputSimulator)
    }

    fn accessibility(&self) -> Box<dyn AccessibilityProvider> {
        Box::new(accessibility::MacOSAccessibility)
    }

    fn screen_capture(&self) -> Box<dyn ScreenCapture> {
        Box::new(screen_capture::MacOSScreenCapture)
    }

    fn app_controller(&self) -> Box<dyn AppController> {
        Box::new(app_control::MacOSAppController)
    }

    fn screen_recorder(&self) -> Box<dyn ScreenRecorder> {
        Box::new(screen_capture::MacOSScreenRecorder)
    }

    fn notifications(&self) -> Box<dyn NotificationProvider> {
        Box::new(notifications::MacOSNotifications)
    }
}

impl Default for MacOSPlatform {
    fn default() -> Self {
        Self::new()
    }
}
