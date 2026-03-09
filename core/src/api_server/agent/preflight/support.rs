use std::process::Command;

use crate::db;

pub(super) fn run_osascript_inline(script: &str) -> Result<String, String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub(super) fn open_system_settings_url(url: &str) -> Result<(), String> {
    Command::new("open")
        .arg(url)
        .status()
        .map_err(|e| e.to_string())
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(format!("open command failed for {}", url))
            }
        })
}

pub(super) fn reveal_path_in_finder(path: &std::path::Path) -> Result<(), String> {
    Command::new("open")
        .arg("-R")
        .arg(path)
        .status()
        .map_err(|e| e.to_string())
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err("open -R failed".to_string())
            }
        })
}

fn escape_applescript_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
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
    let recipient_escaped = escape_applescript_string(recipient);
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
    run_osascript_inline(&script)
}

pub(super) fn mail_cleanup_outgoing_windows() -> Result<String, String> {
    let script = "tell application \"Mail\"\n\
        activate\n\
        set _count to (count of outgoing messages)\n\
        if _count = 0 then return \"NO_OUTGOING|0\"\n\
        repeat with _msg in outgoing messages\n\
            try\n\
                set visible of _msg to false\n\
            end try\n\
        end repeat\n\
        return \"OK|\" & (_count as text)\n\
    end tell";
    run_osascript_inline(script)
}

pub(super) fn textedit_save_front_document() -> Result<String, String> {
    let script = "tell application \"TextEdit\"\n\
        activate\n\
        if (count of documents) = 0 then return \"NO_DOCUMENT\"\n\
        set _doc to front document\n\
        save _doc\n\
        set _docId to \"\"\n\
        try\n\
            set _docId to id of _doc as text\n\
        end try\n\
        return \"OK|\" & _docId\n\
    end tell";
    run_osascript_inline(script)
}

pub(super) fn preflight_focus_mode() -> String {
    std::env::var("STEER_PREFLIGHT_FOCUS_MODE")
        .ok()
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "passive".to_string())
}

pub(super) fn preflight_accessibility_snapshot_probe() -> Result<String, String> {
    let script = r#"
tell application "System Events"
    set frontProc to first application process whose frontmost is true
    set appName to name of frontProc
    set winName to ""
    try
        if (count of windows of frontProc) > 0 then
            set winName to name of window 1 of frontProc
        end if
    end try
    if winName is missing value then set winName to ""
    return appName & " :: " & winName
end tell
"#;
    run_osascript_inline(script)
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
