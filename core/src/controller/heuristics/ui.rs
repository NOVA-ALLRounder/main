use crate::applescript;
use sha2::{Digest, Sha256};

pub fn looks_like_subject(text: &str) -> bool {
    let lower = text.to_lowercase();
    text.len() <= 80
        && !text.contains('\n')
        && (lower.contains("meeting")
            || lower.contains("research findings")
            || lower.contains("notes")
            || lower.contains("subject"))
}

pub fn focus_text_area(app: &str, prefer_subject: bool) -> bool {
    let script = match app {
        "Mail" => {
            if prefer_subject {
                r#"
                    tell application "System Events"
                        tell process "Mail"
                            if exists window 1 then
                                try
                                    if exists text field 1 of window 1 then
                                        click text field 1 of window 1
                                        return "subject"
                                    end if
                                end try
                            end if
                        end tell
                    end tell
                    return ""
                "#
            } else {
                r#"
                    tell application "System Events"
                        tell process "Mail"
                            if exists window 1 then
                                try
                                    if exists scroll area 1 of window 1 then
                                        click scroll area 1 of window 1
                                        return "body"
                                    end if
                                end try
                            end if
                        end tell
                    end tell
                    return ""
                "#
            }
        }
        "Notes" => {
            r#"
            tell application "System Events"
                tell process "Notes"
                    if exists window 1 then
                        try
                            if exists scroll area 1 of window 1 then
                                click scroll area 1 of window 1
                                return "body"
                            end if
                        end try
                    end if
                end tell
            end tell
            return ""
        "#
        }
        "TextEdit" => {
            r#"
            tell application "System Events"
                tell process "TextEdit"
                    if exists window 1 then
                        set wName to ""
                        try
                            set wName to name of window 1 as text
                        end try
                        if wName contains "Open" or wName contains "open" or wName contains "열기" or wName contains "Save" or wName contains "save" or wName contains "저장" then
                            try
                                if exists button "Cancel" of window 1 then
                                    click button "Cancel" of window 1
                                else if exists button "취소" of window 1 then
                                    click button "취소" of window 1
                                else
                                    key code 53
                                end if
                                delay 0.12
                            end try
                        end if
                        try
                            if exists scroll area 1 of window 1 then
                                click scroll area 1 of window 1
                                return "body"
                            end if
                        end try
                    end if
                end tell
            end tell
            return ""
        "#
        }
        _ => "",
    };

    if script.is_empty() {
        return false;
    }

    if let Ok(out) = applescript::run(script) {
        if !out.trim().is_empty() {
            return true;
        }
    }

    let fallback = format!(
        r#"
        tell application "System Events"
            tell process "{}"
                if exists window 1 then
                    set {{x, y}} to position of window 1
                    set {{w, h}} to size of window 1
                    set cx to x + (w / 2)
                    set cy to y + (h / 2)
                    click at {{cx, cy}}
                end if
            end tell
        end tell
    "#,
        app
    );
    let _ = applescript::run(&fallback);
    true
}

pub fn try_close_front_dialog() -> bool {
    let script = r#"
        tell application "System Events"
            set frontApp to name of first application process whose frontmost is true
            tell process frontApp
                if (count of windows) > 0 then
                    set w to window 1
                    if exists sheet 1 of w then
                        if exists button "Cancel" of sheet 1 of w then
                            click button "Cancel" of sheet 1 of w
                            return "cancel-sheet"
                        else if exists button "취소" of sheet 1 of w then
                            click button "취소" of sheet 1 of w
                            return "cancel-sheet"
                        else if exists button "닫기" of sheet 1 of w then
                            click button "닫기" of sheet 1 of w
                            return "close-sheet"
                        end if
                    end if

                    set wName to ""
                    try
                        set wName to name of w as text
                    end try
                    set isDialogTitle to false
                    if wName is "Open" or wName is "Open…" or wName is "Save" or wName is "Save…" or wName is "Save As" or wName is "열기" or wName is "저장" then
                        set isDialogTitle to true
                    end if
                    if isDialogTitle then
                        if exists button "Cancel" of w then
                            click button "Cancel" of w
                            return "cancel-window"
                        else if exists button "취소" of w then
                            click button "취소" of w
                            return "cancel-window"
                        else if exists button "닫기" of w then
                            click button "닫기" of w
                            return "close-window"
                        else
                            key code 53
                            return "escape-window"
                        end if
                    end if
                end if
            end tell
        end tell
        return ""
    "#;
    if let Ok(out) = applescript::run(script) {
        return !out.trim().is_empty();
    }
    false
}

pub fn goal_primary_app(goal: &str) -> Option<&'static str> {
    let lower = goal.to_lowercase();
    if lower.contains("safari") || lower.contains("사파리") {
        return Some("Safari");
    }
    if lower.contains("notes") || lower.contains("노트") || lower.contains("메모") {
        return Some("Notes");
    }
    if lower.contains("mail") || lower.contains("메일") || lower.contains("gmail") {
        return Some("Mail");
    }
    if lower.contains("textedit") || lower.contains("텍스트에디트") || lower.contains("텍스트 편집")
    {
        return Some("TextEdit");
    }
    if lower.contains("calculator") || lower.contains("계산기") {
        return Some("Calculator");
    }
    if lower.contains("finder") || lower.contains("파인더") {
        return Some("Finder");
    }
    if lower.contains("preview") || lower.contains("미리보기") {
        return Some("Preview");
    }
    if lower.contains("calendar") || lower.contains("캘린더") {
        return Some("Calendar");
    }
    None
}

pub fn looks_like_dialog(desc: &str) -> bool {
    let desc_lower = desc.to_lowercase();
    desc_lower.contains("cancel")
        || desc_lower.contains("취소")
        || desc_lower.contains("open dialog")
        || desc_lower.contains("open file")
        || desc_lower.contains("save dialog")
        || desc_lower.contains("save")
}

pub async fn ensure_app_focus(target_app: &str, retries: usize) -> bool {
    let effective_retries = std::env::var("STEER_FOCUS_RETRIES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or_else(|| retries.clamp(1, 2));

    if let Ok(front) = crate::tool_chaining::CrossAppBridge::get_frontmost_app() {
        if front.eq_ignore_ascii_case(target_app) {
            return true;
        }
    }

    for _ in 0..effective_retries {
        let _ = crate::tool_chaining::CrossAppBridge::switch_to_app(target_app);
        tokio::time::sleep(tokio::time::Duration::from_millis(220)).await;
        if let Ok(front) = crate::tool_chaining::CrossAppBridge::get_frontmost_app() {
            if front.eq_ignore_ascii_case(target_app) {
                return true;
            }
        }
    }
    false
}

pub fn compute_plan_key(goal: &str, image_b64: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(goal.as_bytes());
    hasher.update(image_b64.as_bytes());
    let out = hasher.finalize();
    format!("{:x}", out)
}

pub fn resume_hint_for_goal(
    goal: &str,
    checkpoint: &Option<String>,
    front_app: Option<&str>,
) -> Option<serde_json::Value> {
    let lower = goal.to_lowercase();
    let cp = checkpoint.as_deref().unwrap_or("");
    let front = front_app.unwrap_or("");
    if lower.contains("mail") && cp == "mail_compose_open" && front.eq_ignore_ascii_case("Mail") {
        return Some(serde_json::json!({"action":"shortcut","key":"v","modifiers":["command"]}));
    }
    if lower.contains("notes") && cp == "notes_note_created" && front.eq_ignore_ascii_case("Notes")
    {
        return Some(serde_json::json!({"action":"shortcut","key":"v","modifiers":["command"]}));
    }
    if lower.contains("textedit")
        && cp == "textedit_new_doc"
        && front.eq_ignore_ascii_case("TextEdit")
    {
        return Some(serde_json::json!({"action":"type","text":"Total hours per year: "}));
    }
    None
}
