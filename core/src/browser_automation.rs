// Browser Automation Module - Ported from clawdbot-main/src/browser/pw-tools-core.interactions.ts
// Provides stable element references and Playwright-style automation

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[path = "browser_automation/interactions.rs"]
mod interactions;
#[path = "browser_automation/legacy.rs"]
mod legacy;
#[path = "browser_automation/snapshot.rs"]
mod snapshot;

pub use legacy::{
    apply_flight_filters, apply_shopping_filters, autofill_form, click_search_button,
    extract_flight_summary, extract_shopping_summary, fill_flight_fields, fill_search_query,
    get_page_context, open_url_in_chrome, scroll_page,
};

/// Element reference from accessibility tree snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementRef {
    pub id: String,   // e.g., "E123"
    pub role: String, // e.g., "button", "textbox"
    pub name: String, // Accessible name
    pub bounds: Option<Bounds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Bounds {
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapshotSource {
    AppleScript,
    Peekaboo,
}

/// Browser automation context
pub struct BrowserAutomation {
    /// Cache of element references from last snapshot
    element_refs: HashMap<String, ElementRef>,
    /// Counter for generating unique element IDs
    ref_counter: u32,
    /// Ordered refs from last snapshot
    last_snapshot_refs: Vec<ElementRef>,
    /// Snapshot source for last capture
    last_snapshot_source: SnapshotSource,
    /// Snapshot id when using Peekaboo
    last_snapshot_id: Option<String>,
}

impl Default for BrowserAutomation {
    fn default() -> Self {
        Self::new()
    }
}

impl BrowserAutomation {
    pub fn new() -> Self {
        Self {
            element_refs: HashMap::new(),
            ref_counter: 0,
            last_snapshot_refs: Vec::new(),
            last_snapshot_source: SnapshotSource::AppleScript,
            last_snapshot_id: None,
        }
    }
}

/// Singleton-style access
use once_cell::sync::Lazy;

/// Singleton-style access (Thread-Safe)
static BROWSER_AUTOMATION: Lazy<Mutex<BrowserAutomation>> =
    Lazy::new(|| Mutex::new(BrowserAutomation::new()));

pub fn get_browser_automation() -> std::sync::MutexGuard<'static, BrowserAutomation> {
    BROWSER_AUTOMATION.lock().unwrap()
}

#[cfg(test)]
#[path = "browser_automation/tests.rs"]
mod tests;
