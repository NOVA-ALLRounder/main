use super::*;

fn get_frontmost_app() -> Result<String, anyhow::Error> {
    let script = r#"
        tell application "System Events"
            name of first application process whose frontmost is true
        end tell
    "#;
    applescript::run(script)
}

async fn execute_menu_click(app: &str, menu_items: &str, description: &str) -> Result<()> {
    info!("      🔧 [WORKAROUND] Using menu click for {}", description);
    let script = format!(
        r#"
        tell application "System Events"
            tell process "{}"
                {}
            end tell
        end tell
    "#,
        app, menu_items
    );

    let task = tokio::task::spawn_blocking(move || applescript::run(&script));

    match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
        Ok(Ok(Ok(_))) => Ok(()),
        Ok(Ok(Err(e))) => Err(anyhow::anyhow!("Menu click failed: {}", e)),
        Ok(Err(_)) => Err(anyhow::anyhow!("Task Panic")),
        Err(_) => Err(anyhow::anyhow!("Menu click timed out")),
    }
}

pub(super) async fn execute_keyboard_shortcut(key: &str, modifiers: &[String]) -> Result<()> {
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    if let Ok(app_name) = get_frontmost_app() {
        let app = app_name.trim();
        let is_notes = app.eq_ignore_ascii_case("Notes") || app == "메모";
        let is_textedit = app.eq_ignore_ascii_case("TextEdit") || app == "텍스트 편집기";
        let is_calculator = app.eq_ignore_ascii_case("Calculator") || app == "계산기";

        if key == "a" && modifiers.contains(&"command".to_string()) {
            if is_notes {
                if execute_menu_click(
                    app,
                    r#"click menu item "모두 선택" of menu "편집" of menu bar 1"#,
                    "Notes Cmd+A",
                )
                .await
                .is_ok()
                {
                    return Ok(());
                }
            } else if is_textedit {
                if execute_menu_click(
                    app,
                    r#"click menu item "모두 선택" of menu "편집" of menu bar 1"#,
                    "TextEdit Cmd+A",
                )
                .await
                .is_ok()
                {
                    return Ok(());
                }
            }
        }

        if key == "c" && modifiers.contains(&"command".to_string()) {
            if is_notes {
                if execute_menu_click(
                    app,
                    r#"click menu item "복사" of menu "편집" of menu bar 1"#,
                    "Notes Cmd+C",
                )
                .await
                .is_ok()
                {
                    return Ok(());
                }
            } else if is_textedit {
                if execute_menu_click(
                    app,
                    r#"click menu item "복사" of menu "편집" of menu bar 1"#,
                    "TextEdit Cmd+C",
                )
                .await
                .is_ok()
                {
                    return Ok(());
                }
            } else if is_calculator {
                if execute_menu_click(
                    app,
                    r#"click menu item "Copy" of menu "Edit" of menu bar 1"#,
                    "Calculator Cmd+C (En)",
                )
                .await
                .is_ok()
                {
                    return Ok(());
                }
                if execute_menu_click(
                    app,
                    r#"click menu item "복사" of menu "편집" of menu bar 1"#,
                    "Calculator Cmd+C (Ko)",
                )
                .await
                .is_ok()
                {
                    return Ok(());
                }
            }
        }

        if key == "n" && modifiers.contains(&"command".to_string()) && is_notes {
            if execute_menu_click(
                app,
                r#"click menu item "새로운 메모" of menu "파일" of menu bar 1"#,
                "Notes Cmd+N",
            )
            .await
            .is_ok()
            {
                return Ok(());
            }
        }

        if key == "v" && modifiers.contains(&"command".to_string()) && is_notes {
            if execute_menu_click(
                app,
                r#"click menu item "붙여넣기" of menu "편집" of menu bar 1"#,
                "Notes Cmd+V",
            )
            .await
            .is_ok()
            {
                return Ok(());
            }
        }
    }

    let key_str = key.to_string();
    let mods_str = modifiers
        .iter()
        .map(|m| format!("{} down", m))
        .collect::<Vec<_>>()
        .join(", ");

    let script = if key_str.eq_ignore_ascii_case("escape") || key_str.eq_ignore_ascii_case("esc") {
        "tell application \"System Events\" to key code 53".to_string()
    } else if modifiers.is_empty() {
        format!(
            "tell application \"System Events\" to keystroke \"{}\"",
            key_str
        )
    } else {
        format!(
            "tell application \"System Events\" to keystroke \"{}\" using {{{}}}",
            key_str, mods_str
        )
    };

    info!("      ⌨️ Shortcut: {} + {:?}", key_str, modifiers);
    debug!("      🔍 [DEBUG] AppleScript Command: {}", script);
    let task = tokio::task::spawn_blocking(move || applescript::run(&script));
    match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
        Ok(Ok(Ok(_))) => Ok(()),
        Ok(Ok(Err(e))) => Err(anyhow::anyhow!("Shortcut Failed: {}", e)),
        Ok(Err(_)) => Err(anyhow::anyhow!("Task Panic")),
        Err(_) => Err(anyhow::anyhow!("Shortcut Timed Out")),
    }
}
