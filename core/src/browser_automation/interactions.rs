use super::*;

use crate::peekaboo_cli;
use crate::tool_chaining::CrossAppBridge;
use anyhow::Context;
use std::process::Command;

impl BrowserAutomation {
    pub fn click_by_ref(&self, ref_id: &str, double_click: bool) -> Result<()> {
        if self.last_snapshot_source == SnapshotSource::Peekaboo {
            let front_app = CrossAppBridge::get_frontmost_app().ok();
            let snapshot_id = self.last_snapshot_id.as_deref();
            peekaboo_cli::click(ref_id, snapshot_id, front_app.as_deref())
                .context("Peekaboo click failed")?;
            if double_click {
                std::thread::sleep(std::time::Duration::from_millis(100));
                peekaboo_cli::click(ref_id, snapshot_id, front_app.as_deref())
                    .context("Peekaboo double click failed")?;
            }
            println!("🖱️ [Browser] Clicked ref '{}' via Peekaboo", ref_id);
            return Ok(());
        }

        let elem = self.element_refs.get(ref_id).ok_or_else(|| {
            anyhow::anyhow!("Element ref '{}' not found. Take a new snapshot.", ref_id)
        })?;

        let (x, y) = elem
            .bounds
            .as_ref()
            .map(|b| b.center())
            .ok_or_else(|| anyhow::anyhow!("Element '{}' has no bounds", ref_id))?;

        let click_count = if double_click { 2 } else { 1 };
        let script = format!(
            r#"tell application "System Events" to click at {{{}, {}}} "#,
            x, y
        );

        for _ in 0..click_count {
            Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .output()
                .context("Failed to execute click")?;

            if double_click {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }

        println!("🖱️ [Browser] Clicked '{}' at ({}, {})", elem.name, x, y);
        Ok(())
    }

    pub fn hover_by_ref(&self, ref_id: &str) -> Result<()> {
        if self.last_snapshot_source == SnapshotSource::Peekaboo {
            let front_app = CrossAppBridge::get_frontmost_app().ok();
            let snapshot_id = self.last_snapshot_id.as_deref();
            peekaboo_cli::click(ref_id, snapshot_id, front_app.as_deref())
                .context("Peekaboo hover fallback (click) failed")?;
            return Ok(());
        }

        let elem = self
            .element_refs
            .get(ref_id)
            .ok_or_else(|| anyhow::anyhow!("Element ref '{}' not found", ref_id))?;

        let (x, y) = elem
            .bounds
            .as_ref()
            .map(|b| b.center())
            .ok_or_else(|| anyhow::anyhow!("Element '{}' has no bounds", ref_id))?;

        let script = format!(
            r#"
            do shell script "cliclick m:{},{}"
            "#,
            x, y
        );
        let _ = Command::new("osascript").arg("-e").arg(&script).output();

        println!("👆 [Browser] Hover over '{}' at ({}, {})", elem.name, x, y);
        Ok(())
    }

    pub fn type_text(&self, text: &str, delay_ms: u64) -> Result<()> {
        let escaped = text.replace("\"", "\\\"").replace("\\", "\\\\");

        let script = if delay_ms > 0 {
            format!(
                r#"
                tell application "System Events"
                    repeat with c in characters of "{}"
                        keystroke c
                        delay {}
                    end repeat
                end tell
                "#,
                escaped,
                delay_ms as f64 / 1000.0
            )
        } else {
            format!(
                r#"tell application "System Events" to keystroke "{}""#,
                escaped
            )
        };

        Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .context("Failed to type text")?;

        println!(
            "⌨️ [Browser] Typed: '{}'",
            if text.len() > 20 { &text[..20] } else { text }
        );
        Ok(())
    }

    pub fn find_by_name(&self, name: &str) -> Option<String> {
        let name_lower = name.to_lowercase();

        for (ref_id, elem) in &self.element_refs {
            if elem.name.to_lowercase().contains(&name_lower) {
                return Some(ref_id.clone());
            }
        }
        None
    }

    pub fn find_first_by_role_contains(&self, needle: &str) -> Option<String> {
        let needle_lower = needle.to_lowercase();
        for elem in &self.last_snapshot_refs {
            if elem.role.to_lowercase().contains(&needle_lower) {
                return Some(elem.id.clone());
            }
        }
        None
    }

    pub fn summarize_refs(refs: &[ElementRef], max: usize) -> String {
        if refs.is_empty() {
            return "SNAPSHOT_REFS: (none)".to_string();
        }
        let mut parts: Vec<String> = Vec::new();
        for r in refs.iter().take(max) {
            let name = if r.name.trim().is_empty() {
                "(unnamed)"
            } else {
                r.name.as_str()
            };
            parts.push(format!("{} [{}] \"{}\"", r.id, r.role, name));
        }
        let mut summary = format!("SNAPSHOT_REFS: {}", parts.join("; "));
        if refs.len() > max {
            summary.push_str(&format!("; +{} more", refs.len() - max));
        }
        summary
    }

    pub fn navigate(&self, url: &str, browser: Option<&str>) -> Result<()> {
        let browser_name = browser.unwrap_or("Safari");
        let script = format!(
            r#"
            tell application "{}"
                activate
                open location "{}"
            end tell
            "#,
            browser_name, url
        );

        Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .context("Failed to navigate")?;

        println!("🌐 [Browser] Navigate to: {}", url);
        Ok(())
    }

    pub fn to_ai_friendly_error(err: anyhow::Error, context: &str) -> anyhow::Error {
        anyhow::anyhow!(
            "Action failed on '{}': {}. Try taking a new snapshot or using a different approach.",
            context,
            err
        )
    }
}
