use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Result};

use super::{run_cmd_start, run_powershell};
use crate::platform::apps::app_role_primary_name;
use crate::platform::types::{AppRole, PlatformFixAction, PlatformKind, SystemSettingsTarget};

const OUTLOOK_FILL_RECIPIENT_SCRIPT_TEMPLATE: &str = r#"
try {
    $outlook = New-Object -ComObject Outlook.Application
} catch {
    Write-Output "OUTLOOK_UNAVAILABLE"
    return
}

$inspector = $outlook.ActiveInspector()
if ($null -eq $inspector) {
    $inspectors = $outlook.Inspectors()
    if ($null -ne $inspectors -and $inspectors.Count -gt 0) {
        $inspector = $inspectors.Item($inspectors.Count)
    }
}

if ($null -eq $inspector) {
    Write-Output "NO_OUTGOING|0"
    return
}

$item = $inspector.CurrentItem
if ($null -eq $item -or $item.Class -ne 43) {
    Write-Output "NO_OUTGOING|0"
    return
}

$recipient = '{recipient}'
$existing = ""
try { $existing = [string]$item.To } catch { }

if ([string]::IsNullOrWhiteSpace($existing)) {
    $item.To = $recipient
    try { $item.Save() } catch { }
}

$entryId = ""
try { $entryId = [string]$item.EntryID } catch { }
Write-Output ("OK|" + $entryId)
"#;

const OUTLOOK_CLEANUP_DRAFTS_SCRIPT: &str = r#"
try {
    $outlook = New-Object -ComObject Outlook.Application
} catch {
    Write-Output "NO_OUTGOING|0"
    return
}

$inspectors = $outlook.Inspectors()
if ($null -eq $inspectors -or $inspectors.Count -eq 0) {
    Write-Output "NO_OUTGOING|0"
    return
}

$closed = 0
for ($i = $inspectors.Count; $i -ge 1; $i--) {
    try {
        $inspector = $inspectors.Item($i)
        if ($null -eq $inspector) { continue }

        $item = $inspector.CurrentItem
        if ($null -eq $item -or $item.Class -ne 43) { continue }

        $sent = $false
        try { $sent = [bool]$item.Sent } catch { }
        if ($sent) { continue }

        try { $item.Save() } catch { }
        try { $inspector.Close(1) } catch { }
        $closed++
    } catch {
        continue
    }
}

if ($closed -eq 0) {
    Write-Output "NO_OUTGOING|0"
} else {
    Write-Output ("OK|" + $closed)
}
"#;

const SAVE_FRONT_TEXT_DOCUMENT_SCRIPT: &str = r#"
Add-Type -AssemblyName System.Windows.Forms;
try {
    [System.Windows.Forms.SendKeys]::SendWait('^s')
    Write-Output "OK|ctrl+s"
} catch {
    Write-Output "NO_DOCUMENT"
}
"#;

fn activate_application(role: AppRole) -> Result<String> {
    let target = app_role_primary_name(PlatformKind::Windows, role);
    run_cmd_start(target)?;
    Ok(format!("{} 실행을 요청했습니다.", target))
}

fn prepare_isolated_mode() -> Result<String> {
    run_cmd_start("explorer.exe")?;
    Ok("Windows 격리 모드 준비를 시작했습니다(Explorer 전면). 방해 요소 최소화는 수동 확인이 필요합니다.".to_string())
}

fn open_system_settings(target: SystemSettingsTarget) -> Result<String> {
    let uri = match target {
        SystemSettingsTarget::UiAutomation => "ms-settings:",
        SystemSettingsTarget::ScreenCapture => "ms-settings:",
        SystemSettingsTarget::InputMonitoring => "ms-settings:privacy",
    };
    run_cmd_start(uri)?;
    Ok(match target {
        SystemSettingsTarget::UiAutomation => {
            "Windows 설정을 열었습니다. UI Automation/UAC 제약은 수동 확인이 필요합니다."
                .to_string()
        }
        SystemSettingsTarget::ScreenCapture => {
            "Windows 설정을 열었습니다. 화면 캡처는 별도 권한보다 OS 정책/UAC 제약을 확인하세요."
                .to_string()
        }
        SystemSettingsTarget::InputMonitoring => {
            "Windows 개인정보 보호 설정을 열었습니다.".to_string()
        }
    })
}

fn reveal_path(path: &Path) -> Result<String> {
    Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .status()
        .map_err(|e| anyhow!(e.to_string()))
        .and_then(|status| {
            if status.success() {
                Ok(format!(
                    "Explorer에서 경로를 표시했습니다: {}",
                    path.display()
                ))
            } else {
                Err(anyhow!("explorer /select failed"))
            }
        })
}

fn escape_powershell_single_quoted(text: &str) -> String {
    text.replace('\'', "''")
}

fn fill_default_mail_recipient(recipient: &str) -> Result<String> {
    let script = OUTLOOK_FILL_RECIPIENT_SCRIPT_TEMPLATE
        .replace("{recipient}", &escape_powershell_single_quoted(recipient));
    run_powershell(&script)
}

fn cleanup_outgoing_mail_drafts() -> Result<String> {
    run_powershell(OUTLOOK_CLEANUP_DRAFTS_SCRIPT)
}

fn save_front_text_document() -> Result<String> {
    run_powershell(SAVE_FRONT_TEXT_DOCUMENT_SCRIPT)
}

pub(super) fn run_fix_action(action: &PlatformFixAction) -> Result<String> {
    match action {
        PlatformFixAction::ActivateApp(role) => activate_application(*role),
        PlatformFixAction::PrepareIsolatedMode => prepare_isolated_mode(),
        PlatformFixAction::OpenSystemSettings(target) => open_system_settings(*target),
        PlatformFixAction::RequestUiAutomationAccess => Ok(
            "Windows는 macOS식 접근성 권한 요청이 없습니다. UI Automation 사용 가능 여부와 UAC 제약을 확인하세요."
                .to_string(),
        ),
        PlatformFixAction::RequestScreenCaptureAccess => Ok(
            "Windows는 macOS식 화면 기록 권한 요청이 없습니다. 캡처 실패 시 OS 정책과 실행 권한을 확인하세요."
                .to_string(),
        ),
        PlatformFixAction::RevealPath(path) => reveal_path(path),
        PlatformFixAction::FillDefaultMailRecipient { recipient } => {
            fill_default_mail_recipient(recipient)
        }
        PlatformFixAction::CleanupOutgoingMailDrafts => cleanup_outgoing_mail_drafts(),
        PlatformFixAction::SaveFrontTextDocument => save_front_text_document(),
    }
}
