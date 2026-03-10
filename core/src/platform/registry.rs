use anyhow::Result;
use std::path::Path;

use super::types::{
    BrowserSnapshotCapture, PlatformFixAction, PlatformKind, SystemSettingsTarget,
    UiAutomationProbe, UiSnapshotElement,
};

pub trait PlatformAdapter: Send + Sync {
    fn kind(&self) -> PlatformKind;

    fn run_fix_action(&self, action: &PlatformFixAction) -> Result<String>;

    fn frontmost_app_name(&self) -> Result<Option<String>>;

    fn selected_text(&self) -> Result<Option<String>> {
        Ok(None)
    }

    fn ui_element_center_at(&self, _x: i32, _y: i32) -> Result<Option<(i32, i32)>> {
        Ok(None)
    }

    fn ui_snapshot(&self, _scope: Option<String>) -> Result<serde_json::Value> {
        Ok(serde_json::json!({
            "role": "Platform",
            "focused_window": { "children": [] },
            "warning": "UI snapshot unavailable on this platform",
            "ok": false
        }))
    }

    fn prepare_snapshot_surface(&self) -> Result<()> {
        Ok(())
    }

    fn ui_automation_probe(&self) -> Result<UiAutomationProbe>;

    fn screen_capture_probe(&self) -> Result<String>;

    fn browser_snapshot_capture(&self) -> Result<BrowserSnapshotCapture>;

    fn browser_snapshot(&self) -> Result<Vec<UiSnapshotElement>> {
        Ok(self.browser_snapshot_capture()?.elements)
    }

    fn browser_click_ref(
        &self,
        _ref_id: &str,
        _snapshot_id: Option<&str>,
        _front_app: Option<&str>,
        _double_click: bool,
    ) -> Result<bool> {
        Ok(false)
    }

    fn browser_hover_ref(
        &self,
        _ref_id: &str,
        _snapshot_id: Option<&str>,
        _front_app: Option<&str>,
    ) -> Result<bool> {
        Ok(false)
    }

    fn browser_click_at(&self, x: i32, y: i32, double_click: bool) -> Result<()>;

    fn browser_hover_at(&self, x: i32, y: i32) -> Result<()>;

    fn browser_type_text(&self, text: &str, delay_ms: u64) -> Result<()>;

    fn browser_navigate(&self, url: &str, browser_hint: Option<&str>) -> Result<()>;

    fn browser_scroll(&self, pixels: i32) -> Result<()>;

    fn activate_app_by_name(&self, app_name: &str) -> Result<()>;

    fn keyboard_shortcut(&self, key: &str, modifiers: &[String]) -> Result<()>;

    fn set_clipboard_text(&self, _text: &str) -> Result<()> {
        Err(anyhow::anyhow!("clipboard write unavailable on this platform"))
    }

    fn get_clipboard_text(&self) -> Result<String> {
        Err(anyhow::anyhow!("clipboard read unavailable on this platform"))
    }

    fn paste_clipboard(&self) -> Result<()> {
        let modifiers = vec!["command".to_string()];
        self.keyboard_shortcut("v", &modifiers)
    }

    fn ensure_mail_draft(
        &self,
        _preferred_id: &str,
        _recipient_hint: &str,
        _marker_hint: &str,
    ) -> Result<String> {
        Err(anyhow::anyhow!("mail draft capability unavailable on this platform"))
    }

    fn set_mail_recipient_if_missing(
        &self,
        _recipient: &str,
        _draft_hint: &str,
    ) -> Result<String> {
        Err(anyhow::anyhow!("mail recipient capability unavailable on this platform"))
    }

    fn outgoing_mail_draft_count(&self) -> Result<i64> {
        Err(anyhow::anyhow!("mail draft count unavailable on this platform"))
    }

    fn cleanup_outgoing_mail_drafts(
        &self,
        _marker_hint: &str,
        _keep_draft_id: &str,
    ) -> Result<i64> {
        Err(anyhow::anyhow!("mail draft cleanup unavailable on this platform"))
    }

    fn set_mail_subject(&self, _subject: &str, _draft_hint: &str) -> Result<String> {
        Err(anyhow::anyhow!("mail subject capability unavailable on this platform"))
    }

    fn append_mail_body(&self, _text: &str, _draft_hint: &str) -> Result<(String, i64)> {
        Err(anyhow::anyhow!("mail body capability unavailable on this platform"))
    }

    fn create_filled_mail_draft(
        &self,
        _body_text: &str,
        _subject_hint: &str,
        _recipient_hint: &str,
    ) -> Result<(String, i64)> {
        Err(anyhow::anyhow!("mail draft creation unavailable on this platform"))
    }

    fn send_mail_draft(
        &self,
        _fallback_address: &str,
        _subject_hint: &str,
        _marker_hint: &str,
        _draft_hint: &str,
        _strict_draft_check: bool,
    ) -> Result<String> {
        Err(anyhow::anyhow!("mail send capability unavailable on this platform"))
    }

    fn reveal_path(&self, path: &Path) -> Result<String> {
        self.run_fix_action(&PlatformFixAction::RevealPath(path.to_path_buf()))
    }

    fn open_system_settings(&self, target: SystemSettingsTarget) -> Result<String> {
        self.run_fix_action(&PlatformFixAction::OpenSystemSettings(target))
    }
}

#[cfg(target_os = "macos")]
static CURRENT_PLATFORM: crate::platform::macos::MacOSPlatform =
    crate::platform::macos::MacOSPlatform;

#[cfg(target_os = "windows")]
static CURRENT_PLATFORM: crate::platform::windows::WindowsPlatform =
    crate::platform::windows::WindowsPlatform;

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
static CURRENT_PLATFORM: crate::platform::windows::WindowsPlatform =
    crate::platform::windows::WindowsPlatform;

pub fn current_platform() -> &'static dyn PlatformAdapter {
    &CURRENT_PLATFORM
}
