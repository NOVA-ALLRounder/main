use axum::Json;
use std::{fs, process::Command};

use crate::permission_manager::PermissionManager;

use super::super::env_truthy_default;
use super::support::{
    preflight_accessibility_snapshot_probe, preflight_focus_mode, run_osascript_inline,
};
use super::types::{AgentPreflightCheckItem, AgentPreflightResponse};

pub(crate) async fn agent_preflight_handler() -> Json<AgentPreflightResponse> {
    let mut checks: Vec<AgentPreflightCheckItem> = Vec::new();
    let mut all_ok = true;
    let mut active_app: Option<String> = None;

    let accessibility = run_osascript_inline(
        "tell application \"System Events\" to return name of first application process",
    );
    match accessibility {
        Ok(name) => checks.push(AgentPreflightCheckItem {
            key: "accessibility".to_string(),
            label: "Accessibility".to_string(),
            ok: true,
            expected: None,
            actual: Some(name),
            message: "Accessibility permission available".to_string(),
        }),
        Err(err) => {
            all_ok = false;
            checks.push(AgentPreflightCheckItem {
                key: "accessibility".to_string(),
                label: "Accessibility".to_string(),
                ok: false,
                expected: None,
                actual: None,
                message: format!("Accessibility unavailable: {}", err),
            });
        }
    }

    if env_truthy_default("STEER_PREFLIGHT_AX_SNAPSHOT", true) {
        match preflight_accessibility_snapshot_probe() {
            Ok(actual) => checks.push(AgentPreflightCheckItem {
                key: "accessibility_snapshot".to_string(),
                label: "Accessibility Snapshot".to_string(),
                ok: true,
                expected: Some("focused app + focused window".to_string()),
                actual: Some(actual),
                message: "Accessibility snapshot ready (osascript probe)".to_string(),
            }),
            Err(err) => {
                all_ok = false;
                checks.push(AgentPreflightCheckItem {
                    key: "accessibility_snapshot".to_string(),
                    label: "Accessibility Snapshot".to_string(),
                    ok: false,
                    expected: Some("focused app + focused window".to_string()),
                    actual: None,
                    message: format!("Accessibility snapshot blocked: {}", err),
                });
            }
        }
    } else {
        checks.push(AgentPreflightCheckItem {
            key: "accessibility_snapshot".to_string(),
            label: "Accessibility Snapshot".to_string(),
            ok: true,
            expected: Some("focused app + focused window".to_string()),
            actual: Some("skipped".to_string()),
            message: "Snapshot check disabled by env".to_string(),
        });
    }

    if env_truthy_default("STEER_PREFLIGHT_SCREEN_CAPTURE", true) {
        let mut is_granted = PermissionManager::check_screen_recording();
        let shot_path = format!("/tmp/steer_agent_preflight_{}.png", std::process::id());

        if is_granted {
            let _ = Command::new("screencapture")
                .args(["-x", shot_path.as_str()])
                .status();
            let _ = fs::remove_file(&shot_path);
            checks.push(AgentPreflightCheckItem {
                key: "screen_capture".to_string(),
                label: "Screen Capture".to_string(),
                ok: true,
                expected: None,
                actual: Some("ok".to_string()),
                message: "Screen capture permission available".to_string(),
            });
        } else {
            let requested = PermissionManager::request_screen_recording();
            if requested {
                is_granted = PermissionManager::check_screen_recording();
                if is_granted {
                    let _ = Command::new("screencapture")
                        .args(["-x", shot_path.as_str()])
                        .status();
                    let _ = fs::remove_file(&shot_path);
                    checks.push(AgentPreflightCheckItem {
                        key: "screen_capture".to_string(),
                        label: "Screen Capture".to_string(),
                        ok: true,
                        expected: None,
                        actual: Some("ok (after request)".to_string()),
                        message: "Screen capture permission granted after request".to_string(),
                    });
                } else {
                    all_ok = false;
                    let exe_hint = std::env::current_exe()
                        .ok()
                        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
                    checks.push(AgentPreflightCheckItem {
                        key: "screen_capture".to_string(),
                        label: "Screen Capture".to_string(),
                        ok: false,
                        expected: None,
                        actual: exe_hint,
                        message: "화면 캡처 불가: 코어 프로세스(local_os_agent)에 '화면 기록' 권한이 필요합니다. 설정에서 코어 바이너리를 추가(+), 해당 프로세스를 재시작하세요.".to_string(),
                    });
                }
            } else {
                all_ok = false;
                let exe_hint = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
                checks.push(AgentPreflightCheckItem {
                    key: "screen_capture".to_string(),
                    label: "Screen Capture".to_string(),
                    ok: false,
                    expected: None,
                    actual: exe_hint,
                    message: "화면 캡처 불가: 코어 프로세스(local_os_agent)에 '화면 기록' 권한이 필요합니다. 설정에서 코어 바이너리를 추가(+), 해당 프로세스를 재시작하세요.".to_string(),
                });
            }
        }
    } else {
        let skip_allowed = env_truthy_default("STEER_TEST_MODE", false)
            || env_truthy_default("STEER_ALLOW_SCREEN_CAPTURE_SKIP", false);
        if !skip_allowed {
            all_ok = false;
            checks.push(AgentPreflightCheckItem {
                key: "screen_capture".to_string(),
                label: "Screen Capture".to_string(),
                ok: false,
                expected: Some("STEER_PREFLIGHT_SCREEN_CAPTURE=1 (default)".to_string()),
                actual: Some("disabled_by_env".to_string()),
                message: "화면 캡처 체크가 비활성화되어 있어 실행 신뢰성을 보장할 수 없습니다. STEER_PREFLIGHT_SCREEN_CAPTURE=1로 복구하거나 테스트 모드에서만 skip 하세요.".to_string(),
            });
        } else {
            checks.push(AgentPreflightCheckItem {
                key: "screen_capture".to_string(),
                label: "Screen Capture".to_string(),
                ok: true,
                expected: None,
                actual: Some("skipped".to_string()),
                message: "Screen capture check skipped by env (test/allowlist mode only)."
                    .to_string(),
            });
        }
    }

    if env_truthy_default("STEER_PREFLIGHT_FOCUS_HANDOFF", true) {
        let focus_mode = preflight_focus_mode();
        let front_res = run_osascript_inline(
            "tell application \"System Events\" to return name of first application process whose frontmost is true",
        );
        let (focus_ok, focus_actual, focus_msg) = if focus_mode == "active" {
            let activate_res = run_osascript_inline("tell application \"Finder\" to activate");
            match (activate_res, front_res) {
                (Ok(_), Ok(front)) => {
                    active_app = Some(front.clone());
                    if front == "Finder" {
                        (
                            true,
                            Some(front),
                            "Focus handoff ready (active mode, frontmost=Finder)".to_string(),
                        )
                    } else {
                        (
                            false,
                            Some(front.clone()),
                            format!("Focus handoff blocked (active mode, frontmost={})", front),
                        )
                    }
                }
                (_, Err(err)) => (false, None, format!("Focus handoff check failed: {}", err)),
                (Err(err), _) => (
                    false,
                    None,
                    format!("Focus handoff activate failed: {}", err),
                ),
            }
        } else {
            match front_res {
                Ok(front) => {
                    active_app = Some(front.clone());
                    if front == "Finder" {
                        (
                            true,
                            Some(front),
                            "Focus handoff ready (passive mode, frontmost=Finder)".to_string(),
                        )
                    } else {
                        (
                            true,
                            Some(front.clone()),
                            format!(
                                "Focus handoff passive check only (frontmost={}; recommended=Finder)",
                                front
                            ),
                        )
                    }
                }
                Err(err) => (false, None, format!("Focus handoff check failed: {}", err)),
            }
        };
        if !focus_ok {
            all_ok = false;
        }
        checks.push(AgentPreflightCheckItem {
            key: "focus_handoff".to_string(),
            label: "Focus Handoff".to_string(),
            ok: focus_ok,
            expected: Some(if focus_mode == "active" {
                "Finder (required)".to_string()
            } else {
                "Finder (recommended)".to_string()
            }),
            actual: focus_actual,
            message: focus_msg,
        });
    } else {
        checks.push(AgentPreflightCheckItem {
            key: "focus_handoff".to_string(),
            label: "Focus Handoff".to_string(),
            ok: true,
            expected: Some("Finder".to_string()),
            actual: Some("skipped".to_string()),
            message: "Focus handoff check disabled by env".to_string(),
        });
    }

    if active_app.is_none() {
        if let Ok(front) = run_osascript_inline(
            "tell application \"System Events\" to return name of first application process whose frontmost is true",
        ) {
            active_app = Some(front);
        }
    }

    Json(AgentPreflightResponse {
        ok: all_ok,
        checks,
        active_app,
        checked_at: chrono::Utc::now().to_rfc3339(),
    })
}
