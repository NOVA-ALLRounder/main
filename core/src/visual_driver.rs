use crate::applescript;
use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use std::fs;
use std::io::Cursor;
use std::process::Command;

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

#[path = "visual_driver/capture.rs"]
mod capture;
#[path = "visual_driver/execute/mod.rs"]
mod execute;
#[path = "visual_driver/support.rs"]
mod support;
#[path = "visual_driver/workflows.rs"]
mod workflows;

use support::normalize_timeout_ms;
pub use workflows::n8n_fallback_create_workflow;

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

impl Default for VisualDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl VisualDriver {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn add_step(&mut self, step: SmartStep) -> &mut Self {
        self.steps.push(step);
        self
    }

    pub fn add_legacy_step(&mut self, action: UiAction) -> &mut Self {
        self.steps.push(SmartStep::new(action, "Legacy Step"));
        self
    }
}
