mod fixes;
mod input;
mod mail;
mod screen_capture;
mod uia_snapshot;

use anyhow::{anyhow, Result};
use std::process::Command;

use super::registry::PlatformAdapter;
use super::types::{
    BrowserSnapshotCapture, BrowserSnapshotSource, PlatformFixAction, PlatformKind,
    UiAutomationProbe, UiSnapshotElement,
};

pub struct WindowsPlatform;

fn run_powershell(script: &str) -> Result<String> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output()
        .map_err(|e| anyhow!(e.to_string()))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(anyhow!(
            "powershell_failed(status={}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn run_cmd_start(target: &str) -> Result<()> {
    Command::new("cmd")
        .args(["/C", "start", "", target])
        .status()
        .map_err(|e| anyhow!(e.to_string()))
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(anyhow!("cmd start failed for {}", target))
            }
        })
}

impl PlatformAdapter for WindowsPlatform {
    fn kind(&self) -> PlatformKind {
        PlatformKind::Windows
    }

    fn run_fix_action(&self, action: &PlatformFixAction) -> Result<String> {
        fixes::run_fix_action(action)
    }

    fn frontmost_app_name(&self) -> Result<Option<String>> {
        uia_snapshot::frontmost_app_name()
    }

    fn selected_text(&self) -> Result<Option<String>> {
        uia_snapshot::selected_text()
    }

    fn ui_element_center_at(&self, x: i32, y: i32) -> Result<Option<(i32, i32)>> {
        uia_snapshot::ui_element_center_at(x, y)
    }

    fn ui_automation_probe(&self) -> Result<UiAutomationProbe> {
        uia_snapshot::ui_automation_probe()
    }

    fn screen_capture_probe(&self) -> Result<String> {
        screen_capture::screen_capture_probe()
    }

    fn browser_snapshot_capture(&self) -> Result<BrowserSnapshotCapture> {
        Ok(BrowserSnapshotCapture {
            elements: uia_snapshot::browser_snapshot()?,
            source: BrowserSnapshotSource::Accessibility,
            snapshot_id: None,
        })
    }

    fn browser_snapshot(&self) -> Result<Vec<UiSnapshotElement>> {
        uia_snapshot::browser_snapshot()
    }

    fn browser_click_at(&self, x: i32, y: i32, double_click: bool) -> Result<()> {
        input::browser_click_at(x, y, double_click)
    }

    fn browser_hover_at(&self, x: i32, y: i32) -> Result<()> {
        input::browser_hover_at(x, y)
    }

    fn browser_type_text(&self, text: &str, delay_ms: u64) -> Result<()> {
        input::browser_type_text(text, delay_ms)
    }

    fn browser_navigate(&self, url: &str, browser_hint: Option<&str>) -> Result<()> {
        input::browser_navigate(url, browser_hint)
    }

    fn browser_scroll(&self, pixels: i32) -> Result<()> {
        input::browser_scroll(pixels)
    }

    fn activate_app_by_name(&self, app_name: &str) -> Result<()> {
        input::activate_app_by_name(app_name)
    }

    fn keyboard_shortcut(&self, key: &str, modifiers: &[String]) -> Result<()> {
        input::keyboard_shortcut(key, modifiers)
    }

    fn set_clipboard_text(&self, text: &str) -> Result<()> {
        input::set_clipboard_text(text)
    }

    fn get_clipboard_text(&self) -> Result<String> {
        input::get_clipboard_text()
    }

    fn ensure_mail_draft(
        &self,
        preferred_id: &str,
        recipient_hint: &str,
        marker_hint: &str,
    ) -> Result<String> {
        mail::ensure_mail_draft(preferred_id, recipient_hint, marker_hint)
    }

    fn set_mail_recipient_if_missing(&self, recipient: &str, draft_hint: &str) -> Result<String> {
        mail::set_mail_recipient_if_missing(recipient, draft_hint)
    }

    fn outgoing_mail_draft_count(&self) -> Result<i64> {
        mail::outgoing_mail_draft_count()
    }

    fn cleanup_outgoing_mail_drafts(
        &self,
        marker_hint: &str,
        keep_draft_id: &str,
    ) -> Result<i64> {
        mail::cleanup_outgoing_mail_drafts(marker_hint, keep_draft_id)
    }

    fn set_mail_subject(&self, subject: &str, draft_hint: &str) -> Result<String> {
        mail::set_mail_subject(subject, draft_hint)
    }

    fn append_mail_body(&self, text: &str, draft_hint: &str) -> Result<(String, i64)> {
        mail::append_mail_body(text, draft_hint)
    }

    fn create_filled_mail_draft(
        &self,
        body_text: &str,
        subject_hint: &str,
        recipient_hint: &str,
    ) -> Result<(String, i64)> {
        mail::create_filled_mail_draft(body_text, subject_hint, recipient_hint)
    }

    fn send_mail_draft(
        &self,
        fallback_address: &str,
        subject_hint: &str,
        marker_hint: &str,
        draft_hint: &str,
        strict_draft_check: bool,
    ) -> Result<String> {
        mail::send_mail_draft(
            fallback_address,
            subject_hint,
            marker_hint,
            draft_hint,
            strict_draft_check,
        )
    }
}
