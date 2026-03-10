use super::*;

use crate::platform::current_platform;
use anyhow::Context;

impl BrowserAutomation {
    pub fn click_by_ref(&self, ref_id: &str, double_click: bool) -> Result<()> {
        if self.last_snapshot_source == SnapshotSource::Peekaboo {
            let front_app = current_platform().frontmost_app_name().ok().flatten();
            let snapshot_id = self.last_snapshot_id.as_deref();
            let handled = current_platform()
                .browser_click_ref(ref_id, snapshot_id, front_app.as_deref(), double_click)
                .context("Peekaboo click failed")?;
            if handled {
                println!("🖱️ [Browser] Clicked ref '{}' via Peekaboo", ref_id);
                return Ok(());
            }
        }

        let elem = self.element_refs.get(ref_id).ok_or_else(|| {
            anyhow::anyhow!("Element ref '{}' not found. Take a new snapshot.", ref_id)
        })?;

        let (x, y) = elem
            .bounds
            .as_ref()
            .map(|b| b.center())
            .ok_or_else(|| anyhow::anyhow!("Element '{}' has no bounds", ref_id))?;

        current_platform()
            .browser_click_at(x, y, double_click)
            .context("Failed to execute click")?;

        println!("🖱️ [Browser] Clicked '{}' at ({}, {})", elem.name, x, y);
        Ok(())
    }

    pub fn hover_by_ref(&self, ref_id: &str) -> Result<()> {
        if self.last_snapshot_source == SnapshotSource::Peekaboo {
            let front_app = current_platform().frontmost_app_name().ok().flatten();
            let snapshot_id = self.last_snapshot_id.as_deref();
            let handled = current_platform()
                .browser_hover_ref(ref_id, snapshot_id, front_app.as_deref())
                .context("Peekaboo hover fallback (click) failed")?;
            if handled {
                return Ok(());
            }
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

        let _ = current_platform().browser_hover_at(x, y);

        println!("👆 [Browser] Hover over '{}' at ({}, {})", elem.name, x, y);
        Ok(())
    }

    pub fn type_text(&self, text: &str, delay_ms: u64) -> Result<()> {
        current_platform()
            .browser_type_text(text, delay_ms)
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
        current_platform()
            .browser_navigate(url, browser)
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
