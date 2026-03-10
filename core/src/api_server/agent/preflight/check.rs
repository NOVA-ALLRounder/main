use axum::Json;

use crate::platform::{app_matches_role, current_platform, AppRole, PlatformFixAction};

use super::super::env_truthy_default;
use super::support::{preflight_focus_mode, preflight_ui_automation_snapshot_probe};
use super::types::{AgentPreflightCheckItem, AgentPreflightResponse};

fn is_file_manager_app(front: &str) -> bool {
    app_matches_role(current_platform().kind(), AppRole::FileManager, front)
}

pub(crate) async fn agent_preflight_handler() -> Json<AgentPreflightResponse> {
    let mut checks: Vec<AgentPreflightCheckItem> = Vec::new();
    let mut all_ok = true;
    let mut active_app: Option<String> = None;

    let ui_automation = current_platform()
        .frontmost_app_name()
        .map(|name| name.unwrap_or_else(|| "unknown".to_string()))
        .map_err(|e| e.to_string());
    match ui_automation {
        Ok(name) => checks.push(AgentPreflightCheckItem {
            key: "ui_automation".to_string(),
            label: "UI Automation".to_string(),
            ok: true,
            expected: None,
            actual: Some(name),
            message: "UI automation capability available".to_string(),
        }),
        Err(err) => {
            all_ok = false;
            checks.push(AgentPreflightCheckItem {
                key: "ui_automation".to_string(),
                label: "UI Automation".to_string(),
                ok: false,
                expected: None,
                actual: None,
                message: format!("UI automation unavailable: {}", err),
            });
        }
    }

    if env_truthy_default("STEER_PREFLIGHT_AX_SNAPSHOT", true) {
        match preflight_ui_automation_snapshot_probe() {
            Ok(actual) => checks.push(AgentPreflightCheckItem {
                key: "ui_automation_snapshot".to_string(),
                label: "UI Automation Snapshot".to_string(),
                ok: true,
                expected: Some("focused app + focused window".to_string()),
                actual: Some(actual),
                message: "UI automation snapshot ready".to_string(),
            }),
            Err(err) => {
                all_ok = false;
                checks.push(AgentPreflightCheckItem {
                    key: "ui_automation_snapshot".to_string(),
                    label: "UI Automation Snapshot".to_string(),
                    ok: false,
                    expected: Some("focused app + focused window".to_string()),
                    actual: None,
                    message: format!("UI automation snapshot blocked: {}", err),
                });
            }
        }
    } else {
        checks.push(AgentPreflightCheckItem {
            key: "ui_automation_snapshot".to_string(),
            label: "UI Automation Snapshot".to_string(),
            ok: true,
            expected: Some("focused app + focused window".to_string()),
            actual: Some("skipped".to_string()),
            message: "Snapshot check disabled by env".to_string(),
        });
    }

    if env_truthy_default("STEER_PREFLIGHT_SCREEN_CAPTURE", true) {
        match current_platform().screen_capture_probe() {
            Ok(actual) => {
                checks.push(AgentPreflightCheckItem {
                    key: "screen_capture".to_string(),
                    label: "Screen Capture".to_string(),
                    ok: true,
                    expected: None,
                    actual: Some(actual),
                    message: "Screen capture probe succeeded".to_string(),
                });
            }
            Err(initial_err) => {
                let requested = current_platform()
                    .run_fix_action(&PlatformFixAction::RequestScreenCaptureAccess)
                    .is_ok();
                match current_platform().screen_capture_probe() {
                    Ok(actual) => checks.push(AgentPreflightCheckItem {
                        key: "screen_capture".to_string(),
                        label: "Screen Capture".to_string(),
                        ok: true,
                        expected: None,
                        actual: Some(if requested {
                            format!("{} (after request)", actual)
                        } else {
                            actual
                        }),
                        message: if requested {
                            "Screen capture probe succeeded after request".to_string()
                        } else {
                            "Screen capture probe succeeded".to_string()
                        },
                    }),
                    Err(err) => {
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
                            message: format!("{} / {}", initial_err, err),
                        });
                    }
                }
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
        let front_res = current_platform()
            .frontmost_app_name()
            .map(|name| name.unwrap_or_else(|| "unknown".to_string()))
            .map_err(|e| e.to_string());
        let (focus_ok, focus_actual, focus_msg) = if focus_mode == "active" {
            let activate_res = current_platform()
                .run_fix_action(&PlatformFixAction::ActivateApp(AppRole::FileManager))
                .map(|_| ())
                .map_err(|e| e.to_string());
            match (activate_res, front_res) {
                (Ok(_), Ok(front)) => {
                    active_app = Some(front.clone());
                    if is_file_manager_app(&front) {
                        (
                            true,
                            Some(front),
                            "Focus handoff ready (active mode, frontmost=file manager)".to_string(),
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
                    if is_file_manager_app(&front) {
                        (
                            true,
                            Some(front),
                            "Focus handoff ready (passive mode, frontmost=file manager)"
                                .to_string(),
                        )
                    } else {
                        (
                            true,
                            Some(front.clone()),
                            format!(
                                "Focus handoff passive check only (frontmost={}; recommended=file manager)",
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
                "File manager (required)".to_string()
            } else {
                "File manager (recommended)".to_string()
            }),
            actual: focus_actual,
            message: focus_msg,
        });
    } else {
        checks.push(AgentPreflightCheckItem {
            key: "focus_handoff".to_string(),
            label: "Focus Handoff".to_string(),
            ok: true,
            expected: Some("File manager".to_string()),
            actual: Some("skipped".to_string()),
            message: "Focus handoff check disabled by env".to_string(),
        });
    }

    if active_app.is_none() {
        if let Ok(Some(front)) = current_platform().frontmost_app_name() {
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
