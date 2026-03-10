use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::platform::{current_platform, AppRole, PlatformFixAction, SystemSettingsTarget};

use super::support::{
    cleanup_outgoing_mail_drafts, mail_fill_default_recipient, persist_recovery_event,
    resolve_mail_recipient_for_recovery, reveal_path_in_file_manager, save_front_text_document,
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
        "activate_finder" | "activate_file_manager" => current_platform()
            .run_fix_action(&PlatformFixAction::ActivateApp(AppRole::FileManager))
            .map_err(|e| e.to_string()),
        "activate_mail" | "activate_mail_client" => current_platform()
            .run_fix_action(&PlatformFixAction::ActivateApp(AppRole::MailClient))
            .map_err(|e| e.to_string()),
        "activate_notes" | "activate_notes_app" => current_platform()
            .run_fix_action(&PlatformFixAction::ActivateApp(AppRole::NotesApp))
            .map_err(|e| e.to_string()),
        "activate_textedit" | "activate_text_editor" => current_platform()
            .run_fix_action(&PlatformFixAction::ActivateApp(AppRole::TextEditor))
            .map_err(|e| e.to_string()),
        "prepare_isolated_mode" => current_platform()
            .run_fix_action(&PlatformFixAction::PrepareIsolatedMode)
            .map_err(|e| e.to_string()),
        "open_ui_automation_settings" | "open_accessibility_settings" => current_platform()
            .open_system_settings(SystemSettingsTarget::UiAutomation)
            .map_err(|e| e.to_string()),
        "open_screen_capture_settings" => current_platform()
            .open_system_settings(SystemSettingsTarget::ScreenCapture)
            .map_err(|e| e.to_string()),
        "request_screen_capture_access" => current_platform()
            .run_fix_action(&PlatformFixAction::RequestScreenCaptureAccess)
            .map_err(|e| e.to_string()),
        "request_ui_automation_access" | "request_accessibility_access" => current_platform()
            .run_fix_action(&PlatformFixAction::RequestUiAutomationAccess)
            .map_err(|e| e.to_string()),
        "reveal_core_binary" => std::env::current_exe()
            .map_err(|e| e.to_string())
            .and_then(|p| reveal_path_in_file_manager(&p).map(|_| p))
            .map(|p| {
                format!(
                    "코어 바이너리를 파일 관리자에서 표시했습니다: {}",
                    p.to_string_lossy()
                )
            }),
        "open_input_monitoring_settings" => current_platform()
            .open_system_settings(SystemSettingsTarget::InputMonitoring)
            .map_err(|e| e.to_string()),
        "mail_fill_default_recipient" => resolve_mail_recipient_for_recovery(run_id.as_deref())
            .ok_or_else(|| {
                "수신자 후보를 찾지 못했습니다(run prompt/STEER_DEFAULT_MAIL_TO 확인 필요)"
                    .to_string()
            })
            .and_then(|recipient| {
                mail_fill_default_recipient(&recipient).map(|result| {
                    if result.starts_with("NO_OUTGOING") {
                        "메일 클라이언트의 발신 초안이 없어 수신자를 채우지 못했습니다.".to_string()
                    } else {
                        format!(
                            "메일 클라이언트 수신자를 기본값({})으로 보강했습니다 ({})",
                            recipient, result
                        )
                    }
                })
            }),
        "cleanup_outgoing_mail_drafts" | "mail_cleanup_outgoing_windows" => {
            cleanup_outgoing_mail_drafts().map(|result| {
                if result.starts_with("NO_OUTGOING") {
                    "메일 클라이언트 발신 초안이 없어 정리할 항목이 없습니다.".to_string()
                } else {
                    format!("메일 클라이언트 발신 초안 창을 정리했습니다 ({})", result)
                }
            })
        }
        "save_front_text_document" | "textedit_save_front_document" => save_front_text_document()
            .map(|result| {
                if result.starts_with("NO_DOCUMENT") {
                    "텍스트 편집기 문서가 없어 저장하지 못했습니다.".to_string()
                } else {
                    format!(
                        "텍스트 편집기 front document 저장을 실행했습니다 ({})",
                        result
                    )
                }
            }),
        _ => Err(format!("unsupported_action: {}", action)),
    };

    let active_app = current_platform().frontmost_app_name().ok().flatten();

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
