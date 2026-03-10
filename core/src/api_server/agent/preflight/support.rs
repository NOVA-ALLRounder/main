use crate::db;
use crate::platform::{current_platform, PlatformFixAction};

pub(super) fn reveal_path_in_file_manager(path: &std::path::Path) -> Result<(), String> {
    current_platform()
        .reveal_path(path)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn parse_primary_mail_recipient(raw: &str) -> Option<String> {
    let is_email_char =
        |ch: char| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '%' | '+' | '-' | '@');

    let scrub_segment = |segment: &str| -> Option<String> {
        let trimmed = segment
            .trim()
            .trim_matches('<')
            .trim_matches('>')
            .trim_matches('"')
            .trim_matches('\'');
        if trimmed.is_empty() {
            return None;
        }

        let mut cleaned = String::new();
        let mut started = false;
        for ch in trimmed.chars() {
            if is_email_char(ch) {
                cleaned.push(ch);
                started = true;
            } else if started {
                break;
            }
        }

        let normalized = cleaned
            .trim_end_matches(['.', ',', ';', ':', ')', '('])
            .to_ascii_lowercase();
        if normalized.contains('@') && normalized.contains('.') {
            Some(normalized)
        } else {
            None
        }
    };

    raw.split(|ch: char| ch.is_whitespace() || ch == ',' || ch == ';')
        .filter_map(scrub_segment)
        .next()
}

pub(super) fn resolve_mail_recipient_for_recovery(run_id: Option<&str>) -> Option<String> {
    if let Some(id) = run_id {
        if let Ok(Some(run)) = db::get_task_run(id) {
            for candidate in crate::semantic_contract::extract_expected_recipients(&run.prompt) {
                if let Some(parsed) = parse_primary_mail_recipient(&candidate) {
                    return Some(parsed);
                }
            }
        }
    }
    std::env::var("STEER_DEFAULT_MAIL_TO")
        .ok()
        .and_then(|v| parse_primary_mail_recipient(&v))
}

pub(super) fn mail_fill_default_recipient(recipient: &str) -> Result<String, String> {
    let action = PlatformFixAction::FillDefaultMailRecipient {
        recipient: recipient.to_string(),
    };
    current_platform()
        .run_fix_action(&action)
        .map_err(|e| e.to_string())
}

pub(super) fn cleanup_outgoing_mail_drafts() -> Result<String, String> {
    current_platform()
        .run_fix_action(&PlatformFixAction::CleanupOutgoingMailDrafts)
        .map_err(|e| e.to_string())
}

pub(super) fn save_front_text_document() -> Result<String, String> {
    current_platform()
        .run_fix_action(&PlatformFixAction::SaveFrontTextDocument)
        .map_err(|e| e.to_string())
}

pub(super) fn preflight_focus_mode() -> String {
    std::env::var("STEER_PREFLIGHT_FOCUS_MODE")
        .ok()
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "passive".to_string())
}

pub(super) fn preflight_ui_automation_snapshot_probe() -> Result<String, String> {
    current_platform()
        .ui_automation_probe()
        .map(|probe| {
            let window = probe.window_title.unwrap_or_default();
            format!("{} :: {}", probe.app_name, window)
        })
        .map_err(|e| e.to_string())
}

pub(super) fn persist_recovery_event(
    run_id: &str,
    stage_name: &str,
    action_key: &str,
    status: &str,
    expected: Option<&str>,
    actual: Option<&str>,
    details: Option<&str>,
) -> Result<bool, String> {
    let run_exists = db::get_task_run(run_id)
        .map_err(|e| e.to_string())?
        .is_some();
    if !run_exists {
        return Ok(false);
    }

    let clean_stage = if stage_name.trim().is_empty() {
        "recovery"
    } else {
        stage_name.trim()
    };
    let clean_action = if action_key.trim().is_empty() {
        "recovery.event"
    } else {
        action_key.trim()
    };
    let clean_status = if status.trim().is_empty() {
        "completed"
    } else {
        status.trim()
    };
    let expected_value = expected
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("completed");
    let actual_value = actual
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(clean_status);
    let passed = matches!(
        clean_status.to_lowercase().as_str(),
        "completed" | "success" | "ok"
    );
    let stage_order = 5;

    db::record_task_stage_run(run_id, clean_stage, stage_order, clean_status, details)
        .map_err(|e| e.to_string())?;
    db::record_task_stage_assertion(
        run_id,
        clean_stage,
        clean_action,
        expected_value,
        actual_value,
        passed,
        details,
    )
    .map_err(|e| e.to_string())?;
    Ok(true)
}
