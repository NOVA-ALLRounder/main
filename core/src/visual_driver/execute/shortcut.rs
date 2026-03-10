use super::*;
use crate::platform::{app_matches_role, current_platform, AppRole, PlatformKind};

fn get_frontmost_app() -> Result<String, anyhow::Error> {
    current_platform()
        .frontmost_app_name()?
        .ok_or_else(|| anyhow::anyhow!("frontmost application unavailable"))
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

    let platform = current_platform().kind();
    if platform == PlatformKind::MacOS {
        if let Ok(app_name) = get_frontmost_app() {
            let app = app_name.trim();
            let is_notes = app_matches_role(platform, AppRole::NotesApp, app);
            let is_textedit = app_matches_role(platform, AppRole::TextEditor, app);
            let is_calculator = app_matches_role(platform, AppRole::Calculator, app);

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
    }

    info!("      ⌨️ Shortcut: {} + {:?}", key, modifiers);
    let key_owned = key.to_string();
    let modifiers_owned = modifiers.to_vec();
    let task = tokio::task::spawn_blocking(move || {
        current_platform().keyboard_shortcut(&key_owned, &modifiers_owned)
    });
    match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
        Ok(Ok(Ok(_))) => Ok(()),
        Ok(Ok(Err(e))) => Err(anyhow::anyhow!("Shortcut Failed: {}", e)),
        Ok(Err(_)) => Err(anyhow::anyhow!("Task Panic")),
        Err(_) => Err(anyhow::anyhow!("Shortcut Timed Out")),
    }
}
