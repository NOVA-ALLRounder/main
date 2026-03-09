use super::*;

use crate::peekaboo_cli;
use crate::tool_chaining::CrossAppBridge;
use std::process::Command;

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
        let raw = std::env::var("STEER_BROWSER_SNAPSHOT_RECOVERY_APPS")
            .unwrap_or_else(|_| "Safari,Google Chrome,Arc".to_string());
        raw.split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }

    fn activate_app(app_name: &str) {
        let escaped = app_name.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!("tell application \"{}\" to activate", escaped);
        let _ = Command::new("osascript").arg("-e").arg(script).output();
    }

    fn recover_snapshot_focus(attempt: usize) {
        let apps = Self::snapshot_recovery_apps();
        if apps.is_empty() {
            return;
        }
        let front = CrossAppBridge::get_frontmost_app().unwrap_or_default();
        let front_already_browser = apps.iter().any(|a| a.eq_ignore_ascii_case(front.trim()));
        if front_already_browser {
            return;
        }
        let idx = if attempt == 0 {
            0
        } else {
            (attempt - 1) % apps.len()
        };
        if let Some(target) = apps.get(idx) {
            Self::activate_app(target);
        }
    }

    fn parse_snapshot_stdout(&mut self, stdout: &str) -> Vec<ElementRef> {
        let mut refs = Vec::new();
        self.element_refs.clear();
        self.ref_counter = 0;
        self.last_snapshot_refs.clear();
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() < 6 {
                continue;
            }
            self.ref_counter += 1;
            let ref_id = format!("E{}", self.ref_counter);
            let elem_ref = ElementRef {
                id: ref_id.clone(),
                role: parts[0].to_string(),
                name: parts[1].to_string(),
                bounds: Some(Bounds {
                    x: parts[2].parse().unwrap_or(0),
                    y: parts[3].parse().unwrap_or(0),
                    width: parts[4].parse().unwrap_or(0),
                    height: parts[5].parse().unwrap_or(0),
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

        let script = r#"
            tell application "System Events"
                set frontApp to first application process whose frontmost is true
                set appName to name of frontApp

                set output to ""
                try
                    set allElements to entire contents of window 1 of frontApp
                    repeat with elem in allElements
                        try
                            set elemRole to role of elem
                            set elemName to name of elem
                            set elemPos to position of elem
                            set elemSize to size of elem
                            if elemName is not "" then
                                set output to output & elemRole & "|" & elemName & "|" & (item 1 of elemPos) & "|" & (item 2 of elemPos) & "|" & (item 1 of elemSize) & "|" & (item 2 of elemSize) & "
"
                            end if
                        end try
                    end repeat
                end try
                return output
            end tell
        "#;

        let retry_count = Self::snapshot_retry_count();
        let retry_sleep = std::time::Duration::from_millis(Self::snapshot_retry_ms());
        let recovery_enabled = Self::snapshot_focus_recovery_enabled();
        let mut last_script_issue: Option<String> = None;

        for attempt in 0..=retry_count {
            if attempt > 0 && recovery_enabled {
                Self::recover_snapshot_focus(attempt);
                std::thread::sleep(retry_sleep);
            }

            match Command::new("osascript").arg("-e").arg(script).output() {
                Ok(output) => {
                    if output.status.success() {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        refs = self.parse_snapshot_stdout(&stdout);
                        if !refs.is_empty() {
                            break;
                        }
                        last_script_issue = Some("osascript_ok_but_no_elements".to_string());
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        let detail = if !stderr.is_empty() { stderr } else { stdout };
                        last_script_issue = Some(format!(
                            "osascript_failed(status={} detail={})",
                            output.status, detail
                        ));
                    }
                }
                Err(err) => {
                    last_script_issue = Some(format!("osascript_exec_error: {}", err));
                }
            }
        }

        if refs.is_empty() && peekaboo_cli::is_available() {
            let front_app = CrossAppBridge::get_frontmost_app().ok();
            if let Ok(snapshot) = peekaboo_cli::take_snapshot(front_app.as_deref()) {
                self.element_refs.clear();
                self.ref_counter = 0;
                self.last_snapshot_refs.clear();
                self.last_snapshot_source = SnapshotSource::Peekaboo;
                self.last_snapshot_id = snapshot.snapshot_id.clone();

                for elem in snapshot.elements {
                    let bounds = elem.bounds.map(|(x, y, w, h)| Bounds {
                        x,
                        y,
                        width: w,
                        height: h,
                    });
                    let elem_ref = ElementRef {
                        id: elem.id.clone(),
                        role: elem.role.clone(),
                        name: elem.name.clone(),
                        bounds,
                    };
                    self.element_refs.insert(elem.id.clone(), elem_ref.clone());
                    self.last_snapshot_refs.push(elem_ref.clone());
                    refs.push(elem_ref);
                }
                println!(
                    "📸 [Browser] Snapshot captured via Peekaboo: {} elements",
                    refs.len()
                );
                return Ok(refs);
            }
        }

        self.last_snapshot_refs = refs.clone();
        println!("📸 [Browser] Snapshot captured: {} elements", refs.len());
        if refs.is_empty() && !crate::env_flag("STEER_ALLOW_EMPTY_SNAPSHOT_REFS") {
            let front_app = CrossAppBridge::get_frontmost_app().unwrap_or_default();
            let issue = last_script_issue.unwrap_or_else(|| "unknown".to_string());
            return Err(anyhow::anyhow!(
                "snapshot returned zero elements (frontmost='{}', issue='{}'). ensure target window is visible/focused and accessibility permissions are granted",
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
