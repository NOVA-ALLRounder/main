use anyhow::{anyhow, Context, Result};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::{thread, time::Duration};

use crate::applescript;
use crate::peekaboo_cli;
use crate::permission_manager::PermissionManager;

use super::apps::app_role_primary_name;
use super::registry::PlatformAdapter;
use super::types::{
    AppRole, BrowserSnapshotCapture, BrowserSnapshotSource, PlatformFixAction, PlatformKind,
    SystemSettingsTarget, UiAutomationProbe, UiBounds, UiSnapshotElement,
};

pub struct MacOSPlatform;

impl MacOSPlatform {
    fn activate_application(&self, role: AppRole) -> Result<String> {
        let app_name = app_role_primary_name(PlatformKind::MacOS, role);
        applescript::activate_app(app_name)
            .map(|_| format!("{}를 전면으로 전환했습니다.", app_name))
    }

    fn prepare_isolated_mode(&self) -> Result<String> {
        applescript::run(
            "tell application \"Finder\" to activate\n\
             delay 0.1\n\
             tell application \"System Events\" to keystroke \"h\" using {command down, option down}\n\
             delay 0.1\n\
             tell application \"Finder\" to activate",
        )
        .map(|_| {
            "격리 실행 모드를 준비했습니다(다른 앱 숨김 + Finder 전면). 실행 중 키보드/마우스 입력을 피하세요."
                .to_string()
        })
    }

    fn open_settings(&self, target: SystemSettingsTarget) -> Result<String> {
        let url = match target {
            SystemSettingsTarget::UiAutomation => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            SystemSettingsTarget::ScreenCapture => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
            }
            SystemSettingsTarget::InputMonitoring => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
            }
        };
        Command::new("open")
            .arg(url)
            .status()
            .map_err(|e| anyhow!(e.to_string()))
            .and_then(|status| {
                if status.success() {
                    Ok(match target {
                        SystemSettingsTarget::UiAutomation => {
                            "접근성 권한 설정 화면을 열었습니다.".to_string()
                        }
                        SystemSettingsTarget::ScreenCapture => {
                            "화면 기록 권한 설정 화면을 열었습니다.".to_string()
                        }
                        SystemSettingsTarget::InputMonitoring => {
                            "입력 모니터링 권한 설정 화면을 열었습니다.".to_string()
                        }
                    })
                } else {
                    Err(anyhow!("open command failed for {}", url))
                }
            })
    }

    fn request_ui_automation_access(&self) -> String {
        let ok = PermissionManager::request_accessibility();
        if ok {
            "접근성 권한을 요청했습니다(프롬프트가 떴으면 허용 후 재시작).".to_string()
        } else {
            "접근성 권한이 아직 없습니다. 설정 화면에서 코어 바이너리를 추가(+)한 뒤 재시작하세요."
                .to_string()
        }
    }

    fn request_screen_capture_access(&self) -> String {
        let ok = PermissionManager::request_screen_recording();
        if ok {
            "화면 기록 권한을 요청했습니다(프롬프트가 떴으면 허용 후 재시작).".to_string()
        } else {
            "화면 기록 권한이 아직 없습니다. 설정 화면에서 코어 바이너리를 추가(+)한 뒤 재시작하세요.".to_string()
        }
    }

    fn probe_screen_capture(&self) -> Result<String> {
        if !PermissionManager::check_screen_recording() {
            return Err(anyhow!(
                "화면 캡처 불가: 코어 프로세스(local_os_agent)에 '화면 기록' 권한이 필요합니다."
            ));
        }
        let shot_path = format!("/tmp/steer_agent_preflight_{}.png", std::process::id());
        Command::new("screencapture")
            .args(["-x", shot_path.as_str()])
            .status()
            .map_err(|e| anyhow!(e.to_string()))
            .and_then(|status| {
                let _ = std::fs::remove_file(&shot_path);
                if status.success() {
                    Ok("ok".to_string())
                } else {
                    Err(anyhow!("screencapture_failed"))
                }
            })
    }

    fn reveal_path_in_file_manager(&self, path: &Path) -> Result<String> {
        Command::new("open")
            .arg("-R")
            .arg(path)
            .status()
            .map_err(|e| anyhow!(e.to_string()))
            .and_then(|status| {
                if status.success() {
                    Ok(format!(
                        "코어 바이너리를 Finder에서 표시했습니다: {}",
                        path.to_string_lossy()
                    ))
                } else {
                    Err(anyhow!("open -R failed"))
                }
            })
    }

    fn fill_mail_recipient(&self, recipient: &str) -> Result<String> {
        let recipient_escaped = recipient.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!(
            "tell application \"Mail\"\n\
                activate\n\
                if (count of outgoing messages) = 0 then return \"NO_OUTGOING\"\n\
                set _msg to (last outgoing message)\n\
                set _hasRecipient to false\n\
                try\n\
                    if (count of to recipients of _msg) > 0 then\n\
                        set _first to address of first to recipient of _msg as text\n\
                        if _first is not \"\" then set _hasRecipient to true\n\
                    end if\n\
                end try\n\
                if _hasRecipient is false then\n\
                    make new to recipient at end of to recipients of _msg with properties {{address:\"{}\"}}\n\
                end if\n\
                set visible of _msg to true\n\
                set _draftId to \"\"\n\
                try\n\
                    set _draftId to id of _msg as text\n\
                end try\n\
                return \"OK|\" & _draftId\n\
            end tell",
            recipient_escaped
        );
        applescript::run(&script).map(|result| {
            if result.starts_with("NO_OUTGOING") {
                "Mail의 outgoing message가 없어 수신자를 채우지 못했습니다.".to_string()
            } else {
                format!(
                    "Mail 수신자를 기본값({})으로 보강했습니다 ({})",
                    recipient, result
                )
            }
        })
    }

    fn cleanup_mail_drafts(&self) -> Result<String> {
        applescript::run(
            "tell application \"Mail\"\n\
                activate\n\
                set _count to (count of outgoing messages)\n\
                if _count = 0 then return \"NO_OUTGOING|0\"\n\
                repeat with _msg in outgoing messages\n\
                    try\n\
                        set visible of _msg to false\n\
                    end try\n\
                end repeat\n\
                return \"OK|\" & (_count as text)\n\
            end tell",
        )
        .map(|result| {
            if result.starts_with("NO_OUTGOING") {
                "Mail outgoing 초안이 없어 정리할 항목이 없습니다.".to_string()
            } else {
                format!("Mail outgoing 초안 창을 정리했습니다 ({})", result)
            }
        })
    }

    fn save_front_text_document_impl(&self) -> Result<String> {
        applescript::run(
            "tell application \"TextEdit\"\n\
                activate\n\
                if (count of documents) = 0 then return \"NO_DOCUMENT\"\n\
                set _doc to front document\n\
                save _doc\n\
                set _docId to \"\"\n\
                try\n\
                    set _docId to id of _doc as text\n\
                end try\n\
                return \"OK|\" & _docId\n\
            end tell",
        )
        .map(|result| {
            if result.starts_with("NO_DOCUMENT") {
                "TextEdit 문서가 없어 저장하지 못했습니다.".to_string()
            } else {
                format!("TextEdit front document 저장을 실행했습니다 ({})", result)
            }
        })
    }
}

impl PlatformAdapter for MacOSPlatform {
    fn kind(&self) -> PlatformKind {
        PlatformKind::MacOS
    }

    fn run_fix_action(&self, action: &PlatformFixAction) -> Result<String> {
        match action {
            PlatformFixAction::ActivateApp(role) => self.activate_application(*role),
            PlatformFixAction::PrepareIsolatedMode => self.prepare_isolated_mode(),
            PlatformFixAction::OpenSystemSettings(target) => self.open_settings(*target),
            PlatformFixAction::RequestUiAutomationAccess => Ok(self.request_ui_automation_access()),
            PlatformFixAction::RequestScreenCaptureAccess => {
                Ok(self.request_screen_capture_access())
            }
            PlatformFixAction::RevealPath(path) => self.reveal_path_in_file_manager(path),
            PlatformFixAction::FillDefaultMailRecipient { recipient } => {
                self.fill_mail_recipient(recipient)
            }
            PlatformFixAction::CleanupOutgoingMailDrafts => self.cleanup_mail_drafts(),
            PlatformFixAction::SaveFrontTextDocument => self.save_front_text_document_impl(),
        }
    }

    fn frontmost_app_name(&self) -> Result<Option<String>> {
        applescript::run(
            "tell application \"System Events\" to return name of first application process whose frontmost is true",
        )
        .map(|name| Some(name.trim().to_string()))
    }

    fn selected_text(&self) -> Result<Option<String>> {
        Ok(crate::macos::accessibility::get_selected_text())
    }

    fn ui_element_center_at(&self, x: i32, y: i32) -> Result<Option<(i32, i32)>> {
        Ok(crate::macos::accessibility::get_element_center_at(x, y))
    }

    fn ui_snapshot(&self, scope: Option<String>) -> Result<serde_json::Value> {
        Ok(crate::macos::accessibility::snapshot(scope))
    }

    fn prepare_snapshot_surface(&self) -> Result<()> {
        if !matches!(self.frontmost_app_name()?.as_deref(), Some("Safari")) {
            return Ok(());
        }

        applescript::run(
            r#"
                tell application "System Events"
                    tell process "Safari"
                        if exists window 1 then
                            if exists pop over 1 of window 1 then
                                try
                                    click button 1 of pop over 1 of window 1
                                end try
                            end if
                        end if
                    end tell
                end tell
            "#,
        )?;
        Ok(())
    }

    fn ui_automation_probe(&self) -> Result<UiAutomationProbe> {
        let raw = applescript::run(
            "tell application \"System Events\"\n\
                set frontProc to first application process whose frontmost is true\n\
                set appName to name of frontProc\n\
                set winName to \"\"\n\
                try\n\
                    if (count of windows of frontProc) > 0 then\n\
                        set winName to name of window 1 of frontProc\n\
                    end if\n\
                end try\n\
                if winName is missing value then set winName to \"\"\n\
                return appName & \" :: \" & winName\n\
            end tell",
        )?;
        let mut parts = raw.splitn(2, " :: ");
        let app_name = parts.next().unwrap_or_default().trim().to_string();
        let window_title = parts
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        Ok(UiAutomationProbe {
            app_name,
            window_title,
        })
    }

    fn screen_capture_probe(&self) -> Result<String> {
        self.probe_screen_capture()
    }

    fn browser_snapshot_capture(&self) -> Result<BrowserSnapshotCapture> {
        let raw = applescript::run(
            r#"
                tell application "System Events"
                    set frontApp to first application process whose frontmost is true
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
            "#,
        )?;
        let elements: Vec<UiSnapshotElement> = raw
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() < 6 {
                    return None;
                }
                Some(UiSnapshotElement {
                    role: parts[0].to_string(),
                    name: parts[1].to_string(),
                    bounds: Some(UiBounds {
                        x: parts[2].parse().unwrap_or(0),
                        y: parts[3].parse().unwrap_or(0),
                        width: parts[4].parse().unwrap_or(0),
                        height: parts[5].parse().unwrap_or(0),
                    }),
                })
            })
            .collect();
        if !elements.is_empty() {
            return Ok(BrowserSnapshotCapture {
                elements,
                source: BrowserSnapshotSource::Accessibility,
                snapshot_id: None,
            });
        }
        if peekaboo_cli::is_available() {
            let front_app = self.frontmost_app_name()?.unwrap_or_default();
            let snapshot = peekaboo_cli::take_snapshot(Some(front_app.as_str()))?;
            let elements = snapshot
                .elements
                .into_iter()
                .map(|element| UiSnapshotElement {
                    role: element.role,
                    name: element.name,
                    bounds: element.bounds.map(|(x, y, width, height)| UiBounds {
                        x,
                        y,
                        width,
                        height,
                    }),
                })
                .collect();
            return Ok(BrowserSnapshotCapture {
                elements,
                source: BrowserSnapshotSource::Peekaboo,
                snapshot_id: snapshot.snapshot_id,
            });
        }
        Ok(BrowserSnapshotCapture {
            elements,
            source: BrowserSnapshotSource::Accessibility,
            snapshot_id: None,
        })
    }

    fn browser_click_ref(
        &self,
        ref_id: &str,
        snapshot_id: Option<&str>,
        front_app: Option<&str>,
        double_click: bool,
    ) -> Result<bool> {
        let Some(snapshot_id) = snapshot_id else {
            return Ok(false);
        };
        peekaboo_cli::click(ref_id, Some(snapshot_id), front_app)
            .context("Peekaboo click failed")?;
        if double_click {
            thread::sleep(Duration::from_millis(100));
            peekaboo_cli::click(ref_id, Some(snapshot_id), front_app)
                .context("Peekaboo double click failed")?;
        }
        Ok(true)
    }

    fn browser_hover_ref(
        &self,
        ref_id: &str,
        snapshot_id: Option<&str>,
        front_app: Option<&str>,
    ) -> Result<bool> {
        let Some(snapshot_id) = snapshot_id else {
            return Ok(false);
        };
        peekaboo_cli::click(ref_id, Some(snapshot_id), front_app)
            .context("Peekaboo hover fallback (click) failed")?;
        Ok(true)
    }

    fn browser_click_at(&self, x: i32, y: i32, double_click: bool) -> Result<()> {
        let click_count = if double_click { 2 } else { 1 };
        let script = format!(
            r#"tell application "System Events" to click at {{{}, {}}}"#,
            x, y
        );
        for _ in 0..click_count {
            Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .output()
                .map_err(|e| anyhow!(e.to_string()))?;
            if double_click {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        Ok(())
    }

    fn browser_hover_at(&self, x: i32, y: i32) -> Result<()> {
        let script = format!("do shell script \"cliclick m:{},{}\"", x, y);
        Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| anyhow!(e.to_string()))?;
        Ok(())
    }

    fn browser_type_text(&self, text: &str, delay_ms: u64) -> Result<()> {
        if delay_ms > 0 {
            let escaped = text.replace("\"", "\\\"").replace("\\", "\\\\");
            let script = format!(
                "tell application \"System Events\"\n\
                    repeat with c in characters of \"{}\"\n\
                        keystroke c\n\
                        delay {}\n\
                    end repeat\n\
                 end tell",
                escaped,
                delay_ms as f64 / 1000.0
            );
            applescript::run(&script).map(|_| ())
        } else {
            crate::macos::actions::type_text(text)
        }
    }

    fn browser_navigate(&self, url: &str, browser_hint: Option<&str>) -> Result<()> {
        let browser_name = browser_hint.unwrap_or("Safari");
        let script = format!(
            "tell application \"{}\"\n\
                activate\n\
                open location \"{}\"\n\
             end tell",
            browser_name, url
        );
        applescript::run(&script).map(|_| ())
    }

    fn browser_scroll(&self, pixels: i32) -> Result<()> {
        let direction = if pixels > 0 { "down" } else { "up" };
        let amount = pixels.abs();
        let script = format!(
            r#"tell application "System Events" to scroll {} by {}"#,
            direction, amount
        );
        Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| anyhow!(e.to_string()))?;
        Ok(())
    }

    fn activate_app_by_name(&self, app_name: &str) -> Result<()> {
        let trimmed = app_name.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("frontmost") {
            return Ok(());
        }
        applescript::activate_app(trimmed).map(|_| ())
    }

    fn keyboard_shortcut(&self, key: &str, modifiers: &[String]) -> Result<()> {
        let key_trimmed = key.trim();
        if key_trimmed.eq_ignore_ascii_case("escape") || key_trimmed.eq_ignore_ascii_case("esc") {
            return applescript::run("tell application \"System Events\" to key code 53")
                .map(|_| ());
        }

        let modifier_tokens = modifiers
            .iter()
            .filter_map(
                |modifier| match modifier.trim().to_ascii_lowercase().as_str() {
                    "command" | "cmd" | "meta" => Some("command down"),
                    "control" | "ctrl" => Some("control down"),
                    "option" | "alt" => Some("option down"),
                    "shift" => Some("shift down"),
                    _ => None,
                },
            )
            .collect::<Vec<_>>();

        let escaped_key = key_trimmed.replace('\\', "\\\\").replace('"', "\\\"");
        let script = if modifier_tokens.is_empty() {
            format!(
                "tell application \"System Events\" to keystroke \"{}\"",
                escaped_key
            )
        } else {
            format!(
                "tell application \"System Events\" to keystroke \"{}\" using {{{}}}",
                escaped_key,
                modifier_tokens.join(", ")
            )
        };
        applescript::run(&script).map(|_| ())
    }

    fn set_clipboard_text(&self, text: &str) -> Result<()> {
        let mut child = Command::new("pbcopy")
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!(e.to_string()))?;
        if let Some(stdin) = child.stdin.as_mut() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| anyhow!(e.to_string()))?;
        }
        let status = child.wait().map_err(|e| anyhow!(e.to_string()))?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("pbcopy failed with status {}", status))
        }
    }

    fn get_clipboard_text(&self) -> Result<String> {
        let output = Command::new("pbpaste")
            .output()
            .map_err(|e| anyhow!(e.to_string()))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(anyhow!(
                "pbpaste failed(status={}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }

    fn reveal_path(&self, path: &Path) -> Result<String> {
        self.reveal_path_in_file_manager(path)
    }
}
