use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::permission_manager::PermissionManager;

use super::support::{
    mail_cleanup_outgoing_windows, mail_fill_default_recipient, open_system_settings_url,
    persist_recovery_event, resolve_mail_recipient_for_recovery, reveal_path_in_finder,
    run_osascript_inline, textedit_save_front_document,
};
use super::types::{AgentPreflightFixRequest, AgentPreflightFixResponse};

pub(crate) async fn agent_preflight_fix_handler(
    Json(payload): Json<AgentPreflightFixRequest>,
) -> impl IntoResponse {
    let action = payload.action.trim().to_string();
    if action.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "missing_action" })),
        )
            .into_response();
    }

    let run_id = payload
        .run_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string());
    let stage_name = payload
        .stage_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("recovery")
        .to_string();
    let assertion_key = payload
        .assertion_key
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .unwrap_or_else(|| format!("recovery.preflight.{}", action.replace(' ', "_")));

    let fix_result: Result<String, String> = match action.as_str() {
        "activate_finder" => run_osascript_inline("tell application \"Finder\" to activate")
            .map(|_| "Finder를 전면으로 전환했습니다. 다시 점검을 실행하세요.".to_string()),
        "activate_mail" => run_osascript_inline("tell application \"Mail\" to activate")
            .map(|_| "Mail을 전면으로 전환했습니다.".to_string()),
        "activate_notes" => run_osascript_inline("tell application \"Notes\" to activate")
            .map(|_| "Notes를 전면으로 전환했습니다.".to_string()),
        "activate_textedit" => run_osascript_inline("tell application \"TextEdit\" to activate")
            .map(|_| "TextEdit를 전면으로 전환했습니다.".to_string()),
        "prepare_isolated_mode" => run_osascript_inline(
            "tell application \"Finder\" to activate\n\
             delay 0.1\n\
             tell application \"System Events\" to keystroke \"h\" using {command down, option down}\n\
             delay 0.1\n\
             tell application \"Finder\" to activate",
        )
        .map(|_| {
            "격리 실행 모드를 준비했습니다(다른 앱 숨김 + Finder 전면). 실행 중 키보드/마우스 입력을 피하세요."
                .to_string()
        }),
        "open_accessibility_settings" => open_system_settings_url(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        )
        .map(|_| "접근성 권한 설정 화면을 열었습니다.".to_string()),
        "open_screen_capture_settings" => open_system_settings_url(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
        )
        .map(|_| "화면 기록 권한 설정 화면을 열었습니다.".to_string()),
        "request_screen_capture_access" => {
            let ok = PermissionManager::request_screen_recording();
            if ok {
                Ok("화면 기록 권한을 요청했습니다(프롬프트가 떴으면 허용 후 재시작).".to_string())
            } else {
                Ok("화면 기록 권한이 아직 없습니다. 설정 화면에서 코어 바이너리를 추가(+)한 뒤 재시작하세요.".to_string())
            }
        }
        "request_accessibility_access" => {
            let ok = PermissionManager::request_accessibility();
            if ok {
                Ok("접근성 권한을 요청했습니다(프롬프트가 떴으면 허용 후 재시작).".to_string())
            } else {
                Ok("접근성 권한이 아직 없습니다. 설정 화면에서 코어 바이너리를 추가(+)한 뒤 재시작하세요.".to_string())
            }
        }
        "reveal_core_binary" => std::env::current_exe()
            .map_err(|e| e.to_string())
            .and_then(|p| reveal_path_in_finder(&p).map(|_| p))
            .map(|p| format!("코어 바이너리를 Finder에서 표시했습니다: {}", p.to_string_lossy())),
        "open_input_monitoring_settings" => open_system_settings_url(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
        )
        .map(|_| "입력 모니터링 권한 설정 화면을 열었습니다.".to_string()),
        "mail_fill_default_recipient" => resolve_mail_recipient_for_recovery(run_id.as_deref())
            .ok_or_else(|| {
                "수신자 후보를 찾지 못했습니다(run prompt/STEER_DEFAULT_MAIL_TO 확인 필요)"
                    .to_string()
            })
            .and_then(|recipient| {
                mail_fill_default_recipient(&recipient).map(|result| {
                    if result.starts_with("NO_OUTGOING") {
                        "Mail의 outgoing message가 없어 수신자를 채우지 못했습니다.".to_string()
                    } else {
                        format!("Mail 수신자를 기본값({})으로 보강했습니다 ({})", recipient, result)
                    }
                })
            }),
        "mail_cleanup_outgoing_windows" => mail_cleanup_outgoing_windows().map(|result| {
            if result.starts_with("NO_OUTGOING") {
                "Mail outgoing 초안이 없어 정리할 항목이 없습니다.".to_string()
            } else {
                format!("Mail outgoing 초안 창을 정리했습니다 ({})", result)
            }
        }),
        "textedit_save_front_document" => textedit_save_front_document().map(|result| {
            if result.starts_with("NO_DOCUMENT") {
                "TextEdit 문서가 없어 저장하지 못했습니다.".to_string()
            } else {
                format!("TextEdit front document 저장을 실행했습니다 ({})", result)
            }
        }),
        _ => Err(format!("unsupported_action: {}", action)),
    };

    let active_app = run_osascript_inline(
        "tell application \"System Events\" to return name of first application process whose frontmost is true",
    )
    .ok();

    let persist_for_fix = |status: &str, details: &str| -> bool {
        let Some(id) = run_id.as_deref() else {
            return false;
        };
        persist_recovery_event(
            id,
            &stage_name,
            &assertion_key,
            status,
            Some("completed"),
            Some(status),
            Some(details),
        )
        .unwrap_or(false)
    };

    match fix_result {
        Ok(message) => {
            let evidence = if let Some(front) = active_app.as_deref() {
                format!("{} (front={})", message, front)
            } else {
                message.clone()
            };
            let recorded = persist_for_fix("completed", &evidence);
            (
                StatusCode::OK,
                Json(json!(AgentPreflightFixResponse {
                    ok: true,
                    action,
                    message,
                    active_app,
                    fixed_at: chrono::Utc::now().to_rfc3339(),
                    recorded,
                    run_id,
                    stage_name: Some(stage_name),
                })),
            )
                .into_response()
        }
        Err(err) => {
            let evidence = if let Some(front) = active_app.as_deref() {
                format!("{} (front={})", err, front)
            } else {
                err.clone()
            };
            let recorded = persist_for_fix("failed", &evidence);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "ok": false,
                    "action": action,
                    "error": err,
                    "active_app": active_app,
                    "fixed_at": chrono::Utc::now().to_rfc3339(),
                    "recorded": recorded,
                    "run_id": run_id,
                    "stage_name": stage_name,
                })),
            )
                .into_response()
        }
    }
}
