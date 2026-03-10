use super::*;

use crate::platform::{
    app_role_aliases, current_platform, AppRole, BrowserSnapshotSource, PlatformFixAction,
    UiSnapshotElement,
};

impl BrowserAutomation {
    fn env_bool_with_default(key: &str, default: bool) -> bool {
        std::env::var(key)
            .ok()
            .map(|v| {
                matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(default)
    }

    fn env_usize_bounded(key: &str, default: usize, min: usize, max: usize) -> usize {
        std::env::var(key)
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .map(|v| v.clamp(min, max))
            .unwrap_or(default)
    }

    fn env_u64_bounded(key: &str, default: u64, min: u64, max: u64) -> u64 {
        std::env::var(key)
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(|v| v.clamp(min, max))
            .unwrap_or(default)
    }

    fn snapshot_focus_recovery_enabled() -> bool {
        Self::env_bool_with_default("STEER_BROWSER_SNAPSHOT_FOCUS_RECOVERY", true)
    }

    fn snapshot_retry_count() -> usize {
        Self::env_usize_bounded("STEER_BROWSER_SNAPSHOT_RETRIES", 2, 0, 8)
    }

    fn snapshot_retry_ms() -> u64 {
        Self::env_u64_bounded("STEER_BROWSER_SNAPSHOT_RETRY_MS", 160, 30, 2000)
    }

    fn snapshot_recovery_apps() -> Vec<String> {
        if let Ok(raw) = std::env::var("STEER_BROWSER_SNAPSHOT_RECOVERY_APPS") {
            return raw
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
        }

        app_role_aliases(current_platform().kind(), AppRole::Browser)
            .iter()
            .copied()
            .filter(|name| !name.eq_ignore_ascii_case("browser"))
            .map(str::to_string)
            .collect()
    }

    fn frontmost_snapshot_app() -> Option<String> {
        current_platform().frontmost_app_name().ok().flatten()
    }

    fn recover_snapshot_focus(attempt: usize) {
        if current_platform()
            .run_fix_action(&PlatformFixAction::ActivateApp(AppRole::Browser))
            .is_ok()
        {
            return;
        }
        let apps = Self::snapshot_recovery_apps();
        if apps.is_empty() {
            return;
        }
        let front = Self::frontmost_snapshot_app().unwrap_or_default();
        let front_already_browser = apps.iter().any(|a| a.eq_ignore_ascii_case(front.trim()));
        if front_already_browser {
            return;
        }
        let idx = if attempt == 0 {
            0
        } else {
            (attempt - 1) % apps.len()
        };
        if apps.get(idx).is_some() {
            let _ = current_platform()
                .run_fix_action(&PlatformFixAction::ActivateApp(AppRole::Browser));
        }
    }

    fn parse_snapshot_elements(&mut self, elements: &[UiSnapshotElement]) -> Vec<ElementRef> {
        let mut refs = Vec::new();
        self.element_refs.clear();
        self.ref_counter = 0;
        self.last_snapshot_refs.clear();
        for element in elements {
            self.ref_counter += 1;
            let ref_id = format!("E{}", self.ref_counter);
            let elem_ref = ElementRef {
                id: ref_id.clone(),
                role: element.role.clone(),
                name: element.name.clone(),
                bounds: element.bounds.as_ref().map(|bounds| Bounds {
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width,
                    height: bounds.height,
                }),
            };
            self.element_refs.insert(ref_id, elem_ref.clone());
            refs.push(elem_ref);
        }
        refs
    }

    pub fn take_snapshot(&mut self) -> Result<Vec<ElementRef>> {
        let mut refs = Vec::new();
        self.element_refs.clear();
        self.ref_counter = 0;
        self.last_snapshot_refs.clear();
        self.last_snapshot_source = SnapshotSource::AppleScript;
        self.last_snapshot_id = None;

        let retry_count = Self::snapshot_retry_count();
        let retry_sleep = std::time::Duration::from_millis(Self::snapshot_retry_ms());
        let recovery_enabled = Self::snapshot_focus_recovery_enabled();
        let mut last_script_issue: Option<String> = None;

        for attempt in 0..=retry_count {
            if attempt > 0 && recovery_enabled {
                Self::recover_snapshot_focus(attempt);
                std::thread::sleep(retry_sleep);
            }

            match current_platform().browser_snapshot_capture() {
                Ok(capture) => {
                    self.last_snapshot_source = match capture.source {
                        BrowserSnapshotSource::Accessibility => SnapshotSource::AppleScript,
                        BrowserSnapshotSource::Peekaboo => SnapshotSource::Peekaboo,
                    };
                    self.last_snapshot_id = capture.snapshot_id.clone();
                    refs = self.parse_snapshot_elements(&capture.elements);
                    if !refs.is_empty() {
                        break;
                    }
                    last_script_issue = Some("platform_snapshot_ok_but_no_elements".to_string());
                }
                Err(err) => {
                    last_script_issue = Some(format!("platform_snapshot_error: {}", err));
                }
            }
        }

        self.last_snapshot_refs = refs.clone();
        println!("📸 [Browser] Snapshot captured: {} elements", refs.len());
        if refs.is_empty() && !crate::env_flag("STEER_ALLOW_EMPTY_SNAPSHOT_REFS") {
            let front_app = Self::frontmost_snapshot_app().unwrap_or_default();
            let issue = last_script_issue.unwrap_or_else(|| "unknown".to_string());
            return Err(anyhow::anyhow!(
                "snapshot returned zero elements (frontmost='{}', issue='{}'). ensure target window is visible/focused and UI automation permissions are granted",
                front_app,
                issue
            ));
        }
        Ok(refs)
    }

    pub fn reset_snapshot(&mut self) {
        self.element_refs.clear();
        self.ref_counter = 0;
        self.last_snapshot_refs.clear();
        self.last_snapshot_id = None;
    }
}
